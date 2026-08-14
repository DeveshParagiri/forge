import * as Crypto from "expo-crypto";

import {
  FORGE_REMOTE_PROTOCOL_VERSION,
  type ClientMessage,
  type InteractionResponse,
  type RemoteError,
  type RemoteQueueItem,
  type RemoteSessionCreated,
  type RemoteSessionSnapshot,
  type ServerMessage,
  decodeServerMessage,
} from "./protocol";
import { eventsUrlForPairing } from "./pairing";

const MAX_RECONNECT_DELAY_MS = 10_000;
const PING_INTERVAL_MS = 20_000;
const SNAPSHOT_RETRY_MS = 1_000;
const QUEUE_PENDING_RESYNC_MS = 8_000;
const QUEUE_PENDING_UNLOCK_MS = 2_000;
const MAX_TIMER_DELAY_MS = 2_147_483_647;

export type RemoteConnectionPhase = "connecting" | "connected" | "reconnecting" | "error";

export interface ForgeRemoteSocketState {
  readonly phase: RemoteConnectionPhase;
  readonly error: string | null;
  readonly snapshot: RemoteSessionSnapshot | null;
  readonly revision: number | null;
  readonly expiresAt: string | null;
  readonly pendingCommandCount: number;
  readonly pendingInteractionIds: ReadonlyArray<string>;
  readonly modelCommandPending: boolean;
  readonly fastModeCommandPending: boolean;
  readonly usageCommandPending: boolean;
  readonly newSessionCommandPending: boolean;
  readonly pendingQueueItemIds: ReadonlyArray<string>;
}

export type RemoteRevocationReason = "stopped" | "expired" | "session_closed";

export interface ForgeRemoteSocketCallbacks {
  readonly onChange: (state: ForgeRemoteSocketState) => void;
  readonly onRevoked: (reason: RemoteRevocationReason) => void;
}

const INITIAL_STATE: ForgeRemoteSocketState = {
  phase: "connecting",
  error: null,
  snapshot: null,
  revision: null,
  expiresAt: null,
  pendingCommandCount: 0,
  pendingInteractionIds: [],
  modelCommandPending: false,
  fastModeCommandPending: false,
  usageCommandPending: false,
  newSessionCommandPending: false,
  pendingQueueItemIds: [],
};

type PendingCommand =
  | { readonly kind: "other" }
  | { readonly kind: "resync" }
  | { readonly kind: "model" }
  | { readonly kind: "fastMode" }
  | { readonly kind: "usage" }
  | {
      readonly kind: "queue";
      readonly queueItemId: string;
      readonly expectedVersion: number;
    }
  | {
      readonly kind: "newSession";
      readonly resolve: (created: RemoteSessionCreated) => void;
      readonly reject: (error: Error) => void;
    }
  | {
      readonly kind: "acceptNewSession";
      readonly resolve: () => void;
      readonly reject: (error: Error) => void;
    }
  | { readonly kind: "interaction"; readonly interactionId: string };

function isSnapshotUnavailable(error: RemoteError): boolean {
  return error.code === "snapshotUnavailable" && error.retryable === true;
}

function isConflictCode(code: string): boolean {
  return ["client_conflict", "too_many_clients", "already_connected"].includes(code);
}

function isIncompatibleCode(code: string): boolean {
  return ["sessionMismatch", "protocolMismatch", "incompatible"].includes(code);
}

/**
 * Native counterpart of the browser ForgeRemoteSocket. Pairing identity is
 * constructor-bound, session identity is pinned by the gateway handshake, and
 * all transcript state comes from authoritative snapshots/replacements.
 */
export class ForgeRemoteSocket {
  private socket: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private expiryTimer: ReturnType<typeof setTimeout> | null = null;
  private snapshotRetryTimer: ReturnType<typeof setTimeout> | null = null;
  private attempt = 0;
  private stopped = false;
  private suspended = false;
  private suppressAutomaticReconnect = false;
  private hasSnapshotForConnection = false;
  private pinnedSessionId: string | null = null;
  private readonly pendingCommands = new Map<string, PendingCommand>();
  private readonly queuePendingTimerByCommandId = new Map<
    string,
    ReturnType<typeof setTimeout>
  >();
  private state: ForgeRemoteSocketState = INITIAL_STATE;

  constructor(
    private readonly gatewayUrl: string,
    private readonly callbacks: ForgeRemoteSocketCallbacks,
    expectedSessionId?: string,
  ) {
    this.pinnedSessionId = expectedSessionId ?? null;
  }

  connect(): void {
    if (this.stopped || this.suspended || this.socket) return;
    this.suppressAutomaticReconnect = false;
    this.hasSnapshotForConnection = false;
    this.patchState({
      phase: this.attempt === 0 ? "connecting" : "reconnecting",
      error: null,
    });
    const socket = new WebSocket(eventsUrlForPairing(this.gatewayUrl));
    this.socket = socket;
    socket.addEventListener("open", () => {
      if (this.socket !== socket || this.stopped) return;
      this.sendHello();
      this.startPing();
    });
    socket.addEventListener("message", (event) => {
      if (this.socket !== socket || this.stopped) return;
      if (typeof event.data !== "string") {
        this.handleRecoverableProtocolError("Forge sent an unsupported binary update.");
        return;
      }
      try {
        this.acceptServerMessage(decodeServerMessage(event.data));
      } catch (error) {
        this.handleRecoverableProtocolError(
          error instanceof Error ? error.message : "Forge sent an invalid update.",
        );
      }
    });
    socket.addEventListener("close", (event) => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.stopPing();
      this.stopSnapshotRetry();
      this.clearPendingCommands();
      if (this.stopped || this.suspended) return;
      const ownershipLost =
        event.code === 4410 ||
        /superseded/i.test(event.reason) ||
        event.code === 4409 ||
        /already connected|one client|conflict/i.test(event.reason);
      if (ownershipLost) {
        this.suppressAutomaticReconnect = true;
        this.patchState({
          phase: "reconnecting",
          error: null,
        });
        return;
      }
      this.patchState({
        phase: "reconnecting",
        error: event.reason || "The private connection was interrupted.",
      });
      this.scheduleReconnect();
    });
    socket.addEventListener("error", () => {
      // React Native deliberately redacts transport details. Close owns retry.
    });
  }

  stop(closeSocket = true): void {
    this.stopped = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.stopPing();
    this.stopExpiry();
    this.stopSnapshotRetry();
    this.clearPendingCommands();
    if (closeSocket) this.socket?.close(1000, "Forge mobile closed");
    this.socket = null;
  }

  /**
   * Releases foreground ownership without forgetting pairing identity or the
   * last authoritative snapshot. Stale events from the released socket are
   * ignored by the per-socket identity guards installed in connect().
   */
  suspend(): void {
    if (this.stopped || this.suspended) return;
    this.suspended = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.stopPing();
    this.stopSnapshotRetry();
    this.clearPendingCommands();
    const socket = this.socket;
    this.socket = null;
    socket?.close(1000, "Forge mobile backgrounded");
    this.patchState({
      phase: this.state.snapshot ? "reconnecting" : "connecting",
      error: null,
    });
  }

  /** Claims ownership immediately when Forge becomes the visible client. */
  resume(): void {
    if (this.stopped) return;
    this.suspended = false;
    this.suppressAutomaticReconnect = false;
    this.connect();
  }

  sendPrompt(text: string): string | null {
    const trimmed = text.trim();
    return trimmed ? this.sendCommand({ type: "prompt", text: trimmed }, { kind: "other" }) : null;
  }

  cancel(): string | null {
    return this.sendCommand({ type: "cancel" }, { kind: "other" });
  }

  setModel(modelId: string, reasoningEffort: string | null): string | null {
    if (this.state.snapshot?.capabilities.setModel !== true) return null;
    if (this.state.snapshot.modelSwitchPending === true) return null;
    if (this.state.snapshot.fastMode?.pending === true) return null;
    if (
      [...this.pendingCommands.values()].some(
        (pending) => pending.kind === "model" || pending.kind === "fastMode",
      )
    ) {
      return null;
    }
    return this.sendCommand({ type: "setModel", modelId, reasoningEffort }, { kind: "model" });
  }

  setFastMode(enabled: boolean): string | null {
    const fastMode = this.state.snapshot?.fastMode;
    if (this.state.snapshot?.capabilities.fastMode !== true || fastMode?.supported !== true) {
      return null;
    }
    if (fastMode.pending === true || fastMode.enabled === enabled) return null;
    if (this.state.snapshot?.modelSwitchPending === true) return null;
    if (
      [...this.pendingCommands.values()].some(
        (pending) => pending.kind === "fastMode" || pending.kind === "model",
      )
    ) {
      return null;
    }
    return this.sendCommand({ type: "setFastMode", enabled }, { kind: "fastMode" });
  }

  refreshUsage(): string | null {
    if (this.state.snapshot?.capabilities.usage !== true) return null;
    if (this.state.snapshot.usage?.status === "loading") return null;
    if ([...this.pendingCommands.values()].some((pending) => pending.kind === "usage")) return null;
    return this.sendCommand({ type: "refreshUsage" }, { kind: "usage" });
  }

  askBtw(question: string): string | null {
    const trimmed = question.trim();
    return trimmed ? this.sendCommand({ type: "btw", question: trimmed }, { kind: "other" }) : null;
  }

  resolveInteraction(interactionId: string, response: InteractionResponse): string | null {
    if (
      [...this.pendingCommands.values()].some(
        (pending) => pending.kind === "interaction" && pending.interactionId === interactionId,
      )
    ) {
      return null;
    }
    return this.sendCommand(
      { type: "resolveInteraction", interactionId, response },
      { kind: "interaction", interactionId },
    );
  }

  editQueuedPrompt(queueItemId: string, expectedVersion: number, text: string): string | null {
    const trimmed = text.trim();
    if (!trimmed || !this.canControlQueueItem(queueItemId, expectedVersion, "edit")) return null;
    return this.sendCommand(
      { type: "editQueuedPrompt", queueItemId, expectedVersion, text: trimmed },
      { kind: "queue", queueItemId, expectedVersion },
    );
  }

  steerQueuedPrompt(queueItemId: string, expectedVersion: number): string | null {
    if (!this.canControlQueueItem(queueItemId, expectedVersion, "steer")) return null;
    return this.sendCommand(
      { type: "steerQueuedPrompt", queueItemId, expectedVersion },
      { kind: "queue", queueItemId, expectedVersion },
    );
  }

  cancelQueuedPrompt(queueItemId: string, expectedVersion: number): string | null {
    if (!this.canControlQueueItem(queueItemId, expectedVersion, "cancel")) return null;
    return this.sendCommand(
      { type: "cancelQueuedPrompt", queueItemId, expectedVersion },
      { kind: "queue", queueItemId, expectedVersion },
    );
  }

  newSession(): Promise<RemoteSessionCreated> | null {
    if (this.state.snapshot?.capabilities.newSession !== true) return null;
    if (
      [...this.pendingCommands.values()].some(
        (pending) => pending.kind === "newSession" || pending.kind === "acceptNewSession",
      )
    ) {
      return null;
    }
    let resolveCreated!: (created: RemoteSessionCreated) => void;
    let rejectCreated!: (error: Error) => void;
    const created = new Promise<RemoteSessionCreated>((resolve, reject) => {
      resolveCreated = resolve;
      rejectCreated = reject;
    });
    const commandId = this.sendCommand(
      { type: "newSession" },
      { kind: "newSession", resolve: resolveCreated, reject: rejectCreated },
    );
    return commandId ? created : null;
  }

  acceptNewSession(sessionId: string): Promise<void> | null {
    const trimmed = sessionId.trim();
    if (!trimmed) return null;
    if (
      [...this.pendingCommands.values()].some(
        (pending) => pending.kind === "newSession" || pending.kind === "acceptNewSession",
      )
    ) {
      return null;
    }
    let resolveAccepted!: () => void;
    let rejectAccepted!: (error: Error) => void;
    const accepted = new Promise<void>((resolve, reject) => {
      resolveAccepted = resolve;
      rejectAccepted = reject;
    });
    const commandId = this.sendCommand(
      { type: "acceptNewSession", sessionId: trimmed },
      { kind: "acceptNewSession", resolve: resolveAccepted, reject: rejectAccepted },
    );
    return commandId ? accepted : null;
  }

  resync(): string | null {
    return this.sendCommand({ type: "resync" }, { kind: "resync" });
  }

  private acceptServerMessage(message: ServerMessage): void {
    if (message.protocolVersion !== FORGE_REMOTE_PROTOCOL_VERSION) {
      this.terminateIncompatible(
        `This app supports protocol ${FORGE_REMOTE_PROTOCOL_VERSION}, but Forge sent protocol ${message.protocolVersion}. Reopen the pairing after updating Forge.`,
      );
      return;
    }
    switch (message.type) {
      case "connected":
        if (!this.pinSession(message.sessionId)) return;
        this.scheduleExpiry(message.expiresAt);
        this.patchState({ expiresAt: message.expiresAt });
        return;
      case "snapshot": {
        if (!this.pinSession(message.session.sessionId)) return;
        const currentRevision = this.state.revision;
        if (currentRevision !== null && message.revision < currentRevision) return;
        this.attempt = 0;
        this.hasSnapshotForConnection = true;
        this.stopSnapshotRetry();
        this.clearPendingCommands(new Set(["newSession", "acceptNewSession", "queue"]));
        this.reconcilePendingQueueCommands(message.session);
        this.patchState({
          phase: "connected",
          error: null,
          snapshot: message.session,
          revision: message.revision,
        });
        return;
      }
      case "delta": {
        const nextSession = message.event.session;
        if (!this.pinSession(nextSession.sessionId)) return;
        if (!this.state.snapshot || this.state.revision === null) {
          this.requestAuthoritativeSnapshot(
            "Forge sent an update before the authoritative session snapshot.",
          );
          return;
        }
        if (message.revision <= this.state.revision) return;
        if (message.baseRevision !== this.state.revision) {
          this.requestAuthoritativeSnapshot(
            "Some session updates were missed. Refreshing authoritative state.",
          );
          return;
        }
        this.patchState({
          phase: "connected",
          error: null,
          snapshot: nextSession,
          revision: message.revision,
        });
        this.reconcilePendingQueueCommands(nextSession);
        return;
      }
      case "resyncRequired":
        this.requestAuthoritativeSnapshot(message.reason);
        return;
      case "revoked":
        this.callbacks.onRevoked(message.reason);
        this.stop();
        return;
      case "commandResult": {
        const pending = this.pendingCommands.get(message.commandId);
        if (!pending) return;
        const keepUntilAuthoritativeQueue =
          pending?.kind === "queue" && message.outcome.status === "ok";
        if (!keepUntilAuthoritativeQueue) this.removePendingCommand(message.commandId);
        if (pending?.kind === "newSession") {
          pending.reject(
            message.outcome.status === "error"
              ? new Error(message.outcome.error.message)
              : new Error("Forge returned an invalid new-session response."),
          );
        } else if (pending?.kind === "acceptNewSession") {
          if (message.outcome.status === "ok") pending.resolve();
          else pending.reject(new Error(message.outcome.error.message));
        }
        if (message.outcome.status === "error") {
          if (isSnapshotUnavailable(message.outcome.error)) this.scheduleSnapshotRetry();
          if (isIncompatibleCode(message.outcome.error.code)) {
            this.terminateIncompatible(message.outcome.error.message);
            return;
          }
          this.patchState({ error: message.outcome.error.message });
        }
        return;
      }
      case "sessionCreated": {
        const pending = this.pendingCommands.get(message.commandId);
        if (pending?.kind !== "newSession") return;
        this.removePendingCommand(message.commandId);
        pending.resolve({
          sessionId: message.sessionId,
          pairingUrl: message.pairingUrl,
          expiresAt: message.expiresAt,
        });
        return;
      }
      case "error":
        if (isSnapshotUnavailable(message.error)) {
          this.patchState({ error: message.error.message });
          this.scheduleSnapshotRetry();
          return;
        }
        if (isIncompatibleCode(message.error.code)) {
          this.terminateIncompatible(message.error.message);
          return;
        }
        if (isConflictCode(message.error.code)) {
          this.suppressAutomaticReconnect = true;
          this.patchState({ phase: "reconnecting", error: null });
          this.socket?.close(4409, "Forge pairing already connected");
          return;
        }
        this.patchState({ error: message.error.message });
        return;
      case "pong":
        return;
    }
  }

  private pinSession(sessionId: string): boolean {
    if (this.pinnedSessionId === null) {
      this.pinnedSessionId = sessionId;
      return true;
    }
    if (this.pinnedSessionId === sessionId) return true;
    this.terminateIncompatible(
      "Forge sent state for a different session. Stop this remote immediately.",
    );
    return false;
  }

  private terminateIncompatible(message: string): void {
    this.patchState({ phase: "error", error: message });
    this.stop();
  }

  private handleRecoverableProtocolError(message: string): void {
    this.requestAuthoritativeSnapshot(message);
  }

  private requestAuthoritativeSnapshot(message: string): void {
    this.patchState({ phase: "reconnecting", error: message });
    if (this.hasSnapshotForConnection) {
      this.resync();
    } else {
      this.sendHello();
    }
  }

  private sendHello(): boolean {
    return this.sendRaw({ type: "hello", protocolVersion: FORGE_REMOTE_PROTOCOL_VERSION });
  }

  private sendCommand(
    command: Extract<ClientMessage, { type: "command" }>["command"],
    pending: PendingCommand | null,
  ): string | null {
    const id = Crypto.randomUUID();
    const sent = this.sendRaw({
      type: "command",
      protocolVersion: FORGE_REMOTE_PROTOCOL_VERSION,
      commandId: id,
      command,
    });
    if (!sent) {
      this.patchState({ error: "The phone is not connected to this Forge session." });
      return null;
    }
    if (pending) {
      this.pendingCommands.set(id, pending);
      if (pending.kind === "queue") this.armQueuePendingTimeout(id);
      this.publishPendingCommands();
    }
    return id;
  }

  private removePendingCommand(id: string): void {
    this.clearQueuePendingTimer(id);
    if (!this.pendingCommands.delete(id)) return;
    this.publishPendingCommands();
  }

  private reconcilePendingQueueCommands(snapshot: RemoteSessionSnapshot): void {
    let changed = false;
    for (const [commandId, pending] of this.pendingCommands) {
      if (pending.kind !== "queue") continue;
      const current = snapshot.queue?.find((item) => item.id === pending.queueItemId);
      if (current?.source === "shared" && current.version === pending.expectedVersion) continue;
      this.clearQueuePendingTimer(commandId);
      this.pendingCommands.delete(commandId);
      changed = true;
    }
    if (changed) this.publishPendingCommands();
  }

  private clearPendingCommands(preserveKinds: ReadonlySet<PendingCommand["kind"]> = new Set()): void {
    if (this.pendingCommands.size === 0) return;
    for (const [commandId, pending] of this.pendingCommands) {
      if (preserveKinds.has(pending.kind)) continue;
      if (pending.kind === "newSession" || pending.kind === "acceptNewSession") {
        pending.reject(new Error("The connection changed before the new session was ready."));
      }
      this.clearQueuePendingTimer(commandId);
      this.pendingCommands.delete(commandId);
    }
    this.publishPendingCommands();
  }

  private publishPendingCommands(): void {
    const pending = [...this.pendingCommands.values()];
    this.patchState({
      pendingCommandCount: pending.filter((entry) => entry.kind !== "resync").length,
      pendingInteractionIds: pending.flatMap((entry) =>
        entry.kind === "interaction" ? [entry.interactionId] : [],
      ),
      modelCommandPending: pending.some((entry) => entry.kind === "model"),
      fastModeCommandPending: pending.some((entry) => entry.kind === "fastMode"),
      usageCommandPending: pending.some((entry) => entry.kind === "usage"),
      newSessionCommandPending: pending.some((entry) => entry.kind === "newSession"),
      pendingQueueItemIds: pending.flatMap((entry) =>
        entry.kind === "queue" ? [entry.queueItemId] : [],
      ),
    });
  }

  private armQueuePendingTimeout(commandId: string): void {
    this.clearQueuePendingTimer(commandId);
    const timer = setTimeout(() => {
      const pending = this.pendingCommands.get(commandId);
      if (pending?.kind !== "queue") return;
      this.resync();
      const unlock = setTimeout(() => {
        this.queuePendingTimerByCommandId.delete(commandId);
        if (this.pendingCommands.get(commandId)?.kind === "queue") {
          this.pendingCommands.delete(commandId);
          this.publishPendingCommands();
        }
      }, QUEUE_PENDING_UNLOCK_MS);
      this.queuePendingTimerByCommandId.set(commandId, unlock);
    }, QUEUE_PENDING_RESYNC_MS);
    this.queuePendingTimerByCommandId.set(commandId, timer);
  }

  private clearQueuePendingTimer(commandId: string): void {
    const timer = this.queuePendingTimerByCommandId.get(commandId);
    if (timer) clearTimeout(timer);
    this.queuePendingTimerByCommandId.delete(commandId);
  }

  private canControlQueueItem(
    queueItemId: string,
    expectedVersion: number,
    action: keyof RemoteQueueItem["actions"],
  ): boolean {
    if (this.state.snapshot?.capabilities.queueControl !== true) return false;
    if (
      [...this.pendingCommands.values()].some(
        (pending) => pending.kind === "queue" && pending.queueItemId === queueItemId,
      )
    ) {
      return false;
    }
    const item = this.state.snapshot.queue?.find((candidate) => candidate.id === queueItemId);
    return (
      item?.source === "shared" && item.version === expectedVersion && item.actions[action] === true
    );
  }

  private sendRaw(message: ClientMessage): boolean {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return false;
    this.socket.send(JSON.stringify(message));
    return true;
  }

  private patchState(patch: Partial<ForgeRemoteSocketState>): void {
    this.state = { ...this.state, ...patch };
    this.callbacks.onChange(this.state);
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer || this.stopped || this.suspended || this.suppressAutomaticReconnect)
      return;
    this.attempt += 1;
    const base = Math.min(MAX_RECONNECT_DELAY_MS, 400 * 2 ** Math.min(this.attempt, 5));
    this.reconnectTimer = setTimeout(
      () => {
        this.reconnectTimer = null;
        this.connect();
      },
      base + Math.floor(Math.random() * 250),
    );
  }

  private startPing(): void {
    this.stopPing();
    this.pingTimer = setInterval(() => {
      this.sendCommand({ type: "ping" }, null);
    }, PING_INTERVAL_MS);
  }

  private stopPing(): void {
    if (this.pingTimer) clearInterval(this.pingTimer);
    this.pingTimer = null;
  }

  private scheduleSnapshotRetry(): void {
    if (this.snapshotRetryTimer || this.stopped || this.suspended) return;
    this.snapshotRetryTimer = setTimeout(() => {
      this.snapshotRetryTimer = null;
      if (this.hasSnapshotForConnection) this.resync();
      else this.sendHello();
    }, SNAPSHOT_RETRY_MS);
  }

  private stopSnapshotRetry(): void {
    if (this.snapshotRetryTimer) clearTimeout(this.snapshotRetryTimer);
    this.snapshotRetryTimer = null;
  }

  private scheduleExpiry(expiresAt: string): void {
    this.stopExpiry();
    const deadline = Date.parse(expiresAt);
    if (!Number.isFinite(deadline)) return;
    this.expiryTimer = setTimeout(
      () => {
        this.callbacks.onRevoked("expired");
        this.stop();
      },
      Math.min(MAX_TIMER_DELAY_MS, Math.max(0, deadline - Date.now())),
    );
  }

  private stopExpiry(): void {
    if (this.expiryTimer) clearTimeout(this.expiryTimer);
    this.expiryTimer = null;
  }
}

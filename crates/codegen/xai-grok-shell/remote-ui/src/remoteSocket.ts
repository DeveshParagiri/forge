import {
  FORGE_REMOTE_PROTOCOL_VERSION,
  type ClientMessage,
  type InteractionResponse,
  ProtocolDecodeError,
  type RemotePromptImage,
  type RemoteQueueItem,
  type RemoteSessionCreated,
  type RemoteSessionSnapshot,
  type ServerMessage,
  commandId,
  decodeServerMessage,
} from "./protocol";
import type { PendingCommand, RemoteClientAction } from "./reducer";

const MAX_RECONNECT_DELAY_MS = 10_000;
const PING_INTERVAL_MS = 20_000;
const RELINQUISH_TIMEOUT_MS = 500;
const QUEUE_PENDING_RESYNC_MS = 8_000;
const QUEUE_PENDING_UNLOCK_MS = 2_000;
const SESSION_COMMAND_TIMEOUT_MS = 45_000;

interface VisibilityTarget {
  readonly hidden: boolean;
  addEventListener(type: "visibilitychange", listener: () => void): void;
  removeEventListener(type: "visibilitychange", listener: () => void): void;
}

export interface RemoteSocketVisibilityController {
  dispose(): void;
  activate(): void;
  whenSettled(): Promise<void>;
}

export interface ForgeRemoteCommands {
  sendPrompt(text: string, images?: ReadonlyArray<RemotePromptImage>): string | null;
  cancel(): string | null;
  setModel(modelId: string, reasoningEffort: string | null): string | null;
  setFastMode(enabled: boolean): string | null;
  askBtw(question: string): string | null;
  refreshUsage(): string | null;
  resolveInteraction(interactionId: string, response: InteractionResponse): string | null;
  editQueuedPrompt(queueItemId: string, expectedVersion: number, text: string): string | null;
  steerQueuedPrompt(queueItemId: string, expectedVersion: number): string | null;
  cancelQueuedPrompt(queueItemId: string, expectedVersion: number): string | null;
  newSession(): Promise<RemoteSessionCreated> | null;
  acceptNewSession(sessionId: string): Promise<void> | null;
  resync(): string | null;
}

type PendingSessionCommand =
  | {
      kind: "newSession";
      resolve: (created: RemoteSessionCreated) => void;
      reject: (error: Error) => void;
      timeout: number;
    }
  | {
      kind: "acceptNewSession";
      resolve: () => void;
      reject: (error: Error) => void;
      timeout: number;
    };

export function eventsUrlForPairing(pairingBaseUrl: string): string {
  const base = new URL(pairingBaseUrl);
  if (!base.pathname.endsWith("/")) base.pathname += "/";
  const url = new URL("events", base);
  url.protocol = base.protocol === "https:" ? "wss:" : "ws:";
  url.search = "";
  url.hash = "";
  return url.toString();
}

export class ForgeRemoteSocket implements ForgeRemoteCommands {
  private socket: WebSocket | null = null;
  private reconnectTimer: number | null = null;
  private pingTimer: number | null = null;
  private expiryTimer: number | null = null;
  private snapshotRetryTimer: number | null = null;
  private stopped = false;
  private attempt = 0;
  private hasSnapshotForConnection = false;
  private latestSession: RemoteSessionSnapshot | null = null;
  private readonly pendingSessionCommands = new Map<string, PendingSessionCommand>();
  private readonly pendingQueueCommands = new Map<
    string,
    { queueItemId: string; expectedVersion: number }
  >();
  private readonly queuePendingTimerByCommandId = new Map<string, number>();
  private readonly pendingModelCommands = new Set<string>();
  private readonly pendingFastModeCommands = new Set<string>();

  constructor(
    private readonly dispatch: (action: RemoteClientAction) => void,
    private readonly pairingBaseUrl = window.location.href,
  ) {}

  connect(): void {
    if (this.socket) return;
    this.stopped = false;
    this.dispatch({
      type: "socketConnecting",
      reconnecting: this.attempt > 0,
      attempt: this.attempt,
    });
    this.hasSnapshotForConnection = false;
    const socket = new WebSocket(eventsUrlForPairing(this.pairingBaseUrl));
    this.socket = socket;
    socket.addEventListener("open", () => {
      if (this.socket !== socket || this.stopped) return;
      this.dispatch({ type: "socketOpen" });
      this.sendRaw({ type: "hello", protocolVersion: FORGE_REMOTE_PROTOCOL_VERSION });
      this.startPing();
    });
    socket.addEventListener("message", (event) => {
      if (this.socket !== socket || this.stopped) return;
      if (typeof event.data !== "string") {
        this.dispatch({ type: "decodeError", message: "Forge sent an unsupported binary update." });
        return;
      }
      try {
        const message = decodeServerMessage(event.data);
        this.observeServerMessage(message);
        if (message.type === "snapshot") {
          this.attempt = 0;
          this.hasSnapshotForConnection = true;
          this.stopSnapshotRetry();
        }
        this.dispatch({ type: "serverMessage", message });
        if (message.type === "connected") this.scheduleExpiry(message.expiresAt);
        if (
          (message.type === "error" &&
            message.error.code === "snapshotUnavailable" &&
            message.error.retryable) ||
          (message.type === "commandResult" &&
            message.outcome.status === "error" &&
            message.outcome.error.code === "snapshotUnavailable" &&
            message.outcome.error.retryable)
        ) {
          this.scheduleSnapshotRetry();
        }
        if (message.type === "revoked") this.stop();
      } catch (error) {
        const message =
          error instanceof ProtocolDecodeError ? error.message : "Forge sent an invalid update.";
        this.dispatch({ type: "decodeError", message });
      }
    });
    socket.addEventListener("close", (event) => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.stopPing();
      this.rejectPendingSessionCommands(
        new Error("The connection changed before the new session was ready."),
      );
      this.clearTransientCommandState();
      this.dispatch({
        type: "socketClosed",
        code: event.code,
        reason: event.reason,
        wasClean: event.wasClean,
      });
      if (event.code === 4410) this.stopped = true;
      if (!this.stopped && event.code !== 4409) this.scheduleReconnect();
    });
    socket.addEventListener("error", () => {
      // The close event owns retry behavior; browsers intentionally redact WebSocket error detail.
    });
  }

  stop(closeSocket = true): void {
    this.stopped = true;
    if (this.reconnectTimer !== null) window.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.stopPing();
    this.stopExpiry();
    this.stopSnapshotRetry();
    this.rejectPendingSessionCommands(new Error("The remote page closed before the new session was ready."));
    this.clearTransientCommandState();
    if (closeSocket) this.socket?.close(1000, "remote page closed");
    this.socket = null;
  }

  relinquish(reason = "remote page hidden"): Promise<void> {
    this.stopped = true;
    if (this.reconnectTimer !== null) window.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.stopPing();
    this.stopExpiry();
    this.stopSnapshotRetry();
    this.rejectPendingSessionCommands(
      new Error("The connection changed before the new session was ready."),
    );
    this.clearTransientCommandState();

    const socket = this.socket;
    if (!socket) return Promise.resolve();

    return new Promise((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        window.clearTimeout(timeout);
        if (this.socket === socket) this.socket = null;
        resolve();
      };
      const timeout = window.setTimeout(finish, RELINQUISH_TIMEOUT_MS);
      socket.addEventListener("close", finish, { once: true });
      try {
        socket.close(1000, reason);
      } catch {
        finish();
      }
    });
  }

  sendPrompt(text: string, images?: ReadonlyArray<RemotePromptImage>): string | null {
    const trimmed = text.trim();
    const attachments = images?.filter((image) => image.data.length > 0) ?? [];
    if (!trimmed && attachments.length === 0) return null;
    return this.sendCommand(
      {
        type: "prompt",
        text: trimmed,
        ...(attachments.length > 0 ? { images: [...attachments] } : {}),
      },
      { type: "prompt", label: "Sending message" },
    );
  }

  cancel(): string | null {
    return this.sendCommand(
      { type: "cancel" },
      { type: "cancel", label: "Stopping current turn" },
    );
  }

  setModel(modelId: string, reasoningEffort: string | null): string | null {
    if (
      this.latestSession?.modelSwitchPending === true ||
      this.latestSession?.fastMode?.pending === true ||
      this.pendingModelCommands.size > 0 ||
      this.pendingFastModeCommands.size > 0
    ) {
      return null;
    }
    const id = this.sendCommand(
      { type: "setModel", modelId, reasoningEffort },
      { type: "setModel", label: "Changing model" },
    );
    if (id) this.pendingModelCommands.add(id);
    return id;
  }

  setFastMode(enabled: boolean): string | null {
    const fastMode = this.latestSession?.fastMode;
    if (
      this.latestSession?.capabilities.fastMode !== true ||
      fastMode?.supported !== true ||
      fastMode.pending === true ||
      fastMode.enabled === enabled ||
      this.latestSession.modelSwitchPending === true ||
      this.pendingFastModeCommands.size > 0 ||
      this.pendingModelCommands.size > 0
    ) {
      return null;
    }
    const id = this.sendCommand(
      { type: "setFastMode", enabled },
      { type: "setFastMode", label: enabled ? "Enabling fast mode" : "Disabling fast mode" },
    );
    if (id) this.pendingFastModeCommands.add(id);
    return id;
  }

  askBtw(question: string): string | null {
    const trimmed = question.trim();
    if (!trimmed) return null;
    return this.sendCommand(
      { type: "btw", question: trimmed },
      { type: "btw", label: "Asking side question" },
    );
  }

  refreshUsage(): string | null {
    return this.sendCommand(
      { type: "refreshUsage" },
      { type: "refreshUsage", label: "Refreshing usage" },
    );
  }

  resolveInteraction(interactionId: string, response: InteractionResponse): string | null {
    return this.sendCommand(
      { type: "resolveInteraction", interactionId, response },
      { type: "resolveInteraction", label: "Answering request" },
    );
  }

  editQueuedPrompt(queueItemId: string, expectedVersion: number, text: string): string | null {
    const trimmed = text.trim();
    if (!trimmed || !this.canControlQueueItem(queueItemId, expectedVersion, "edit")) return null;
    return this.sendQueueCommand(
      { type: "editQueuedPrompt", queueItemId, expectedVersion, text: trimmed },
      queueItemId,
      expectedVersion,
      "Editing queued message",
    );
  }

  steerQueuedPrompt(queueItemId: string, expectedVersion: number): string | null {
    if (!this.canControlQueueItem(queueItemId, expectedVersion, "steer")) return null;
    return this.sendQueueCommand(
      { type: "steerQueuedPrompt", queueItemId, expectedVersion },
      queueItemId,
      expectedVersion,
      "Steering with queued message",
    );
  }

  cancelQueuedPrompt(queueItemId: string, expectedVersion: number): string | null {
    if (!this.canControlQueueItem(queueItemId, expectedVersion, "cancel")) return null;
    return this.sendQueueCommand(
      { type: "cancelQueuedPrompt", queueItemId, expectedVersion },
      queueItemId,
      expectedVersion,
      "Cancelling queued message",
    );
  }

  newSession(): Promise<RemoteSessionCreated> | null {
    if (
      this.latestSession?.capabilities.newSession !== true ||
      this.pendingSessionCommands.size > 0
    ) {
      return null;
    }
    let resolveCreated!: (created: RemoteSessionCreated) => void;
    let rejectCreated!: (error: Error) => void;
    const created = new Promise<RemoteSessionCreated>((resolve, reject) => {
      resolveCreated = resolve;
      rejectCreated = reject;
    });
    const id = this.sendCommand(
      { type: "newSession" },
      { type: "newSession", label: "Creating session" },
    );
    if (!id) return null;
    this.trackPendingSessionCommand(id, {
      kind: "newSession",
      resolve: resolveCreated,
      reject: rejectCreated,
    });
    return created;
  }

  acceptNewSession(sessionId: string): Promise<void> | null {
    const trimmed = sessionId.trim();
    if (!trimmed || this.pendingSessionCommands.size > 0) return null;
    let resolveAccepted!: () => void;
    let rejectAccepted!: (error: Error) => void;
    const accepted = new Promise<void>((resolve, reject) => {
      resolveAccepted = resolve;
      rejectAccepted = reject;
    });
    const id = this.sendCommand(
      { type: "acceptNewSession", sessionId: trimmed },
      { type: "acceptNewSession", label: "Opening session" },
    );
    if (!id) return null;
    this.trackPendingSessionCommand(id, {
      kind: "acceptNewSession",
      resolve: resolveAccepted,
      reject: rejectAccepted,
    });
    return accepted;
  }

  resync(): string | null {
    return this.sendCommand({ type: "resync" }, null);
  }

  private sendQueueCommand(
    command: Extract<ClientMessage, { type: "command" }>["command"],
    queueItemId: string,
    expectedVersion: number,
    label: string,
  ): string | null {
    const id = this.sendCommand(command, {
      type: "queue",
      label,
      queueItemId,
      expectedVersion,
    });
    if (id) {
      this.pendingQueueCommands.set(id, { queueItemId, expectedVersion });
      this.armQueuePendingTimeout(id);
    }
    return id;
  }

  private canControlQueueItem(
    queueItemId: string,
    expectedVersion: number,
    action: keyof RemoteQueueItem["actions"],
  ): boolean {
    if (
      !queueItemId.trim() ||
      !Number.isSafeInteger(expectedVersion) ||
      expectedVersion < 0 ||
      this.latestSession?.capabilities.queueControl !== true ||
      [...this.pendingQueueCommands.values()].some(
        (pending) => pending.queueItemId === queueItemId,
      )
    ) {
      return false;
    }
    const item = this.latestSession.queue?.find((candidate) => candidate.id === queueItemId);
    return (
      item?.source === "shared" &&
      item.version === expectedVersion &&
      item.actions[action] === true
    );
  }

  private observeServerMessage(message: ServerMessage): void {
    if (message.protocolVersion !== FORGE_REMOTE_PROTOCOL_VERSION) return;
    if (message.type === "snapshot") {
      this.latestSession = message.session;
      this.pendingModelCommands.clear();
      this.pendingFastModeCommands.clear();
      this.reconcilePendingQueueCommands(message.session);
      return;
    }
    if (message.type === "delta" && message.event.kind === "stateReplaced") {
      this.latestSession = message.event.session;
      this.pendingModelCommands.clear();
      this.pendingFastModeCommands.clear();
      this.reconcilePendingQueueCommands(message.event.session);
      return;
    }
    if (message.type === "sessionCreated") {
      const pending = this.pendingSessionCommands.get(message.commandId);
      if (pending?.kind !== "newSession") return;
      this.pendingSessionCommands.delete(message.commandId);
      window.clearTimeout(pending.timeout);
      pending.resolve({
        sessionId: message.sessionId,
        pairingUrl: message.pairingUrl,
        expiresAt: message.expiresAt,
      });
      return;
    }
    if (message.type !== "commandResult") return;

    this.pendingModelCommands.delete(message.commandId);
    this.pendingFastModeCommands.delete(message.commandId);
    if (message.outcome.status === "error") {
      this.clearQueuePendingTimer(message.commandId);
      this.pendingQueueCommands.delete(message.commandId);
    }

    const pending = this.pendingSessionCommands.get(message.commandId);
    if (!pending) return;
    this.pendingSessionCommands.delete(message.commandId);
    window.clearTimeout(pending.timeout);
    if (pending.kind === "newSession") {
      pending.reject(
        message.outcome.status === "error"
          ? new Error(message.outcome.error.message)
          : new Error("Forge returned an invalid new-session response."),
      );
      return;
    }
    if (message.outcome.status === "ok") pending.resolve();
    else pending.reject(new Error(message.outcome.error.message));
  }

  private reconcilePendingQueueCommands(session: RemoteSessionSnapshot): void {
    for (const [commandId, pending] of this.pendingQueueCommands) {
      const current = session.queue?.find((item) => item.id === pending.queueItemId);
      if (current?.source === "shared" && current.version === pending.expectedVersion) continue;
      this.clearQueuePendingTimer(commandId);
      this.pendingQueueCommands.delete(commandId);
    }
  }

  private armQueuePendingTimeout(commandId: string): void {
    this.clearQueuePendingTimer(commandId);
    const timer = window.setTimeout(() => {
      const pending = this.pendingQueueCommands.get(commandId);
      if (!pending) return;
      this.resync();
      const unlock = window.setTimeout(() => {
        this.queuePendingTimerByCommandId.delete(commandId);
        if (!this.pendingQueueCommands.delete(commandId)) return;
        this.dispatch({ type: "commandSettled", commandId });
      }, QUEUE_PENDING_UNLOCK_MS);
      this.queuePendingTimerByCommandId.set(commandId, unlock);
    }, QUEUE_PENDING_RESYNC_MS);
    this.queuePendingTimerByCommandId.set(commandId, timer);
  }

  private clearQueuePendingTimer(commandId: string): void {
    const timer = this.queuePendingTimerByCommandId.get(commandId);
    if (timer !== undefined) window.clearTimeout(timer);
    this.queuePendingTimerByCommandId.delete(commandId);
  }

  private rejectPendingSessionCommands(error: Error): void {
    for (const pending of this.pendingSessionCommands.values()) {
      window.clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pendingSessionCommands.clear();
  }

  private trackPendingSessionCommand(
    commandId: string,
    pending: Omit<PendingSessionCommand, "timeout">,
  ): void {
    const timeout = window.setTimeout(() => {
      const current = this.pendingSessionCommands.get(commandId);
      if (!current) return;
      this.pendingSessionCommands.delete(commandId);
      this.dispatch({ type: "commandSettled", commandId });
      current.reject(new Error("Forge did not finish the new-session request in time."));
    }, SESSION_COMMAND_TIMEOUT_MS);
    this.pendingSessionCommands.set(commandId, { ...pending, timeout } as PendingSessionCommand);
  }

  private clearTransientCommandState(): void {
    for (const timer of this.queuePendingTimerByCommandId.values()) window.clearTimeout(timer);
    this.queuePendingTimerByCommandId.clear();
    this.pendingQueueCommands.clear();
    this.pendingModelCommands.clear();
    this.pendingFastModeCommands.clear();
  }

  private sendCommand(
    command: Extract<ClientMessage, { type: "command" }>["command"],
    pending: PendingCommand | null,
  ): string | null {
    const id = commandId();
    const sent = this.sendRaw({
      type: "command",
      protocolVersion: FORGE_REMOTE_PROTOCOL_VERSION,
      commandId: id,
      command,
    });
    if (!sent) {
      this.dispatch({ type: "decodeError", message: "The phone is not connected to Forge." });
      return null;
    }
    if (pending) this.dispatch({ type: "commandQueued", commandId: id, command: pending });
    return id;
  }

  private sendRaw(message: ClientMessage): boolean {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return false;
    this.socket.send(JSON.stringify(message));
    return true;
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer !== null || this.stopped) return;
    this.attempt += 1;
    const baseDelay = Math.min(MAX_RECONNECT_DELAY_MS, 400 * 2 ** Math.min(this.attempt, 5));
    const delay = baseDelay + Math.floor(Math.random() * 250);
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private startPing(): void {
    this.stopPing();
    this.pingTimer = window.setInterval(() => {
      this.sendCommand({ type: "ping" }, null);
    }, PING_INTERVAL_MS);
  }

  private stopPing(): void {
    if (this.pingTimer !== null) window.clearInterval(this.pingTimer);
    this.pingTimer = null;
  }

  private scheduleExpiry(expiresAt: string): void {
    this.stopExpiry();
    const expiresAtMs = Date.parse(expiresAt);
    if (!Number.isFinite(expiresAtMs)) return;
    this.expiryTimer = window.setTimeout(() => {
      this.expiryTimer = null;
      this.dispatch({ type: "localExpired" });
      this.stop();
    }, Math.max(0, expiresAtMs - Date.now()));
  }

  private stopExpiry(): void {
    if (this.expiryTimer !== null) window.clearTimeout(this.expiryTimer);
    this.expiryTimer = null;
  }

  private scheduleSnapshotRetry(): void {
    if (this.snapshotRetryTimer !== null || this.stopped) return;
    this.snapshotRetryTimer = window.setTimeout(() => {
      this.snapshotRetryTimer = null;
      if (this.hasSnapshotForConnection) {
        this.resync();
      } else {
        this.sendRaw({ type: "hello", protocolVersion: FORGE_REMOTE_PROTOCOL_VERSION });
      }
    }, 1_000);
  }

  private stopSnapshotRetry(): void {
    if (this.snapshotRetryTimer !== null) window.clearTimeout(this.snapshotRetryTimer);
    this.snapshotRetryTimer = null;
  }

}

export function bindRemoteSocketVisibility(
  socket: Pick<ForgeRemoteSocket, "connect" | "relinquish" | "stop">,
  visibilityTarget: VisibilityTarget = document,
): RemoteSocketVisibilityController {
  let disposed = false;
  let transition = Promise.resolve();

  const enqueue = (visible: boolean) => {
    transition = transition.then(async () => {
      if (disposed) return;
      if (visible) {
        socket.connect();
      } else {
        await socket.relinquish();
      }
    });
  };
  const onVisibilityChange = () => enqueue(!visibilityTarget.hidden);

  visibilityTarget.addEventListener("visibilitychange", onVisibilityChange);
  enqueue(!visibilityTarget.hidden);

  return {
    activate: () => enqueue(true),
    dispose: () => {
      if (disposed) return;
      disposed = true;
      visibilityTarget.removeEventListener("visibilitychange", onVisibilityChange);
      socket.stop();
    },
    whenSettled: () => transition,
  };
}

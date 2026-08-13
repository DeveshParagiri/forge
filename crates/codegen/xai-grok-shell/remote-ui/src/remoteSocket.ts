import {
  FORGE_REMOTE_PROTOCOL_VERSION,
  type ClientMessage,
  type InteractionResponse,
  ProtocolDecodeError,
  commandId,
  decodeServerMessage,
} from "./protocol";
import type { PendingCommand, RemoteClientAction } from "./reducer";

const MAX_RECONNECT_DELAY_MS = 10_000;
const PING_INTERVAL_MS = 20_000;
const RELINQUISH_TIMEOUT_MS = 500;

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
  sendPrompt(text: string): string | null;
  cancel(): string | null;
  setModel(modelId: string, reasoningEffort: string | null): string | null;
  askBtw(question: string): string | null;
  refreshUsage(): string | null;
  resolveInteraction(interactionId: string, response: InteractionResponse): string | null;
  resync(): string | null;
}

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

  sendPrompt(text: string): string | null {
    const trimmed = text.trim();
    if (!trimmed) return null;
    return this.sendCommand({ type: "prompt", text: trimmed }, "prompt", "Sending message");
  }

  cancel(): string | null {
    return this.sendCommand({ type: "cancel" }, "cancel", "Stopping current turn");
  }

  setModel(modelId: string, reasoningEffort: string | null): string | null {
    return this.sendCommand(
      { type: "setModel", modelId, reasoningEffort },
      "setModel",
      "Changing model",
    );
  }

  askBtw(question: string): string | null {
    const trimmed = question.trim();
    if (!trimmed) return null;
    return this.sendCommand({ type: "btw", question: trimmed }, "btw", "Asking side question");
  }

  refreshUsage(): string | null {
    return this.sendCommand({ type: "refreshUsage" }, "refreshUsage", "Refreshing usage");
  }

  resolveInteraction(interactionId: string, response: InteractionResponse): string | null {
    return this.sendCommand(
      { type: "resolveInteraction", interactionId, response },
      "resolveInteraction",
      "Answering request",
    );
  }

  resync(): string | null {
    return this.sendCommand({ type: "resync" }, null, "Refreshing session");
  }

  private sendCommand(
    command: Extract<ClientMessage, { type: "command" }>["command"],
    pendingType: PendingCommand["type"] | null,
    label: string,
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
    if (pendingType) {
      this.dispatch({ type: "commandQueued", commandId: id, command: { type: pendingType, label } });
    }
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
      this.sendCommand({ type: "ping" }, null, "Checking connection");
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

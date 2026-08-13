import { afterEach, describe, expect, it, vi } from "vitest";
import {
  bindRemoteSocketVisibility,
  eventsUrlForPairing,
  ForgeRemoteSocket,
} from "../src/remoteSocket";
import { sessionFixture } from "./fixtures";
import {
  initialRemoteClientState,
  remoteClientReducer,
  type RemoteClientAction,
  type RemoteClientState,
} from "../src/reducer";

type Listener = (event: Record<string, unknown>) => void;

class FakeWebSocket {
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static instances: FakeWebSocket[] = [];
  static autoClose = true;

  readyState = 0;
  readonly sent: string[] = [];
  private readonly listeners = new Map<string, Listener[]>();

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this);
  }

  addEventListener(type: string, listener: Listener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  send(payload: string): void {
    this.sent.push(payload);
  }

  private pendingClose: { code: number; reason: string } | undefined;

  close(code = 1000, reason = ""): void {
    this.readyState = FakeWebSocket.CLOSING;
    this.pendingClose = { code, reason };
    if (FakeWebSocket.autoClose) this.completeClose();
  }

  completeClose(): void {
    const { code, reason } = this.pendingClose ?? { code: 1000, reason: "" };
    this.pendingClose = undefined;
    this.readyState = FakeWebSocket.CLOSED;
    this.emit("close", { code, reason, wasClean: true });
  }

  emit(type: string, event: Record<string, unknown> = {}): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  FakeWebSocket.instances = [];
  FakeWebSocket.autoClose = true;
});

class VisibilityFixture {
  hidden = false;
  private listeners = new Set<() => void>();

  addEventListener(_type: "visibilitychange", listener: () => void): void {
    this.listeners.add(listener);
  }

  removeEventListener(_type: "visibilitychange", listener: () => void): void {
    this.listeners.delete(listener);
  }

  change(hidden: boolean): void {
    this.hidden = hidden;
    for (const listener of this.listeners) listener();
  }
}

describe("ForgeRemoteSocket lifecycle", () => {
  it("derives one exact WebSocket endpoint from the selected immutable pairing", () => {
    const first = `https://forge.tail.example/forge/${"a".repeat(64)}/`;
    const second = `https://forge.tail.example/forge/${"b".repeat(64)}/`;
    expect(eventsUrlForPairing(first)).toBe(`wss://forge.tail.example/forge/${"a".repeat(64)}/events`);
    expect(eventsUrlForPairing(second)).toBe(`wss://forge.tail.example/forge/${"b".repeat(64)}/events`);
  });

  it("connects only the base URL passed for the selected pairing", () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const pairing = `https://forge.tail.example/forge/${"b".repeat(64)}/`;
    const remote = new ForgeRemoteSocket(() => {}, pairing);
    remote.connect();
    expect(FakeWebSocket.instances).toHaveLength(1);
    expect(FakeWebSocket.instances[0]?.url).toBe(`${pairing}events`.replace("https:", "wss:"));
    remote.stop();
  });

  it("restarts cleanly after an effect cleanup and ignores the stale socket", () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const pairing = `https://forge.tail.example/forge/${"b".repeat(64)}/`;
    const remote = new ForgeRemoteSocket(() => {}, pairing);
    remote.connect();
    const first = FakeWebSocket.instances[0];
    first.readyState = FakeWebSocket.OPEN;
    first.emit("open");

    remote.stop(false);
    remote.connect();
    const second = FakeWebSocket.instances[1];
    second.readyState = FakeWebSocket.OPEN;
    second.emit("open");
    first.emit("close", { code: 1006, reason: "stale", wasClean: false });

    expect(remote.sendPrompt("still connected")).not.toBeNull();
    expect(second.sent.map((payload) => JSON.parse(payload))).toContainEqual(
      expect.objectContaining({ command: { type: "prompt", text: "still connected" } }),
    );
    remote.stop();
  });

  it("disconnects while Safari is hidden and does not schedule a reconnect", async () => {
    vi.useFakeTimers();
    vi.spyOn(Math, "random").mockReturnValue(0);
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const visibility = new VisibilityFixture();
    const remote = new ForgeRemoteSocket(() => {}, `https://forge.tail.example/forge/${"c".repeat(64)}/`);
    const lifecycle = bindRemoteSocketVisibility(remote, visibility);
    await lifecycle.whenSettled();
    expect(FakeWebSocket.instances).toHaveLength(1);

    const first = FakeWebSocket.instances[0];
    first.readyState = FakeWebSocket.OPEN;
    first.emit("open");
    visibility.change(true);
    await lifecycle.whenSettled();
    vi.advanceTimersByTime(30_000);

    expect(first.readyState).toBe(FakeWebSocket.CLOSED);
    expect(FakeWebSocket.instances).toHaveLength(1);
    lifecycle.dispose();
  });

  it("reconnects only when Safari becomes visible again", async () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const visibility = new VisibilityFixture();
    const remote = new ForgeRemoteSocket(() => {}, `https://forge.tail.example/forge/${"d".repeat(64)}/`);
    const lifecycle = bindRemoteSocketVisibility(remote, visibility);
    await lifecycle.whenSettled();

    visibility.change(true);
    await lifecycle.whenSettled();
    expect(FakeWebSocket.instances).toHaveLength(1);

    visibility.change(false);
    await lifecycle.whenSettled();
    expect(FakeWebSocket.instances).toHaveLength(2);
    lifecycle.dispose();
  });

  it("waits for hidden-socket release before a rapid visible reconnect", async () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    FakeWebSocket.autoClose = false;
    const visibility = new VisibilityFixture();
    const remote = new ForgeRemoteSocket(() => {}, `https://forge.tail.example/forge/${"e".repeat(64)}/`);
    const lifecycle = bindRemoteSocketVisibility(remote, visibility);
    await lifecycle.whenSettled();
    const first = FakeWebSocket.instances[0];

    visibility.change(true);
    visibility.change(false);
    await Promise.resolve();
    await Promise.resolve();
    expect(first.readyState).toBe(FakeWebSocket.CLOSING);
    expect(FakeWebSocket.instances).toHaveLength(1);

    first.completeClose();
    await lifecycle.whenSettled();
    expect(FakeWebSocket.instances).toHaveLength(2);
    lifecycle.dispose();
  });

  it("does not automatically reclaim ownership after a 4410 superseded close", () => {
    vi.useFakeTimers();
    vi.spyOn(Math, "random").mockReturnValue(0);
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const remote = new ForgeRemoteSocket(() => {}, `https://forge.tail.example/forge/${"f".repeat(64)}/`);
    remote.connect();
    const socket = FakeWebSocket.instances[0];
    socket.readyState = FakeWebSocket.OPEN;
    socket.emit("open");
    socket.emit("close", {
      code: 4410,
      reason: "superseded by a newer active client",
      wasClean: true,
    });

    vi.advanceTimersByTime(30_000);
    expect(FakeWebSocket.instances).toHaveLength(1);

    remote.connect();
    expect(FakeWebSocket.instances).toHaveLength(2);
    remote.stop();
  });

  it("sends a trimmed BTW question as the typed side-channel command", () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const remote = new ForgeRemoteSocket(() => {});
    remote.connect();
    const socket = FakeWebSocket.instances[0];
    socket.readyState = FakeWebSocket.OPEN;
    socket.emit("open");
    expect(remote.askBtw("   did the tests finish?   ")).not.toBeNull();
    expect(socket.sent.map((payload) => JSON.parse(payload))).toContainEqual(
      expect.objectContaining({
        type: "command",
        command: { type: "btw", question: "did the tests finish?" },
      }),
    );
    remote.stop();
  });

  it("sends usage refresh as its own typed command instead of a prompt", () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const remote = new ForgeRemoteSocket(() => {});
    remote.connect();
    const socket = FakeWebSocket.instances[0];
    socket.readyState = FakeWebSocket.OPEN;
    socket.emit("open");
    expect(remote.refreshUsage()).not.toBeNull();
    const sent = socket.sent.map((payload) => JSON.parse(payload));
    expect(sent).toContainEqual(
      expect.objectContaining({
        type: "command",
        command: { type: "refreshUsage" },
      }),
    );
    expect(sent.some((message) => message.command?.type === "prompt")).toBe(false);
    remote.stop();
  });

  it("expires locally and stops reconnecting when the phone was offline at expiry", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2030-01-01T00:00:00Z"));
    vi.spyOn(Math, "random").mockReturnValue(0);
    vi.stubGlobal("WebSocket", FakeWebSocket);
    let state: RemoteClientState = initialRemoteClientState;
    const dispatch = (action: RemoteClientAction) => {
      state = remoteClientReducer(state, action);
    };
    const remote = new ForgeRemoteSocket(dispatch);

    remote.connect();
    const first = FakeWebSocket.instances[0];
    first.readyState = FakeWebSocket.OPEN;
    first.emit("open");
    first.emit("message", {
      data: JSON.stringify({
        type: "connected",
        protocolVersion: 1,
        sessionId: "session-123",
        expiresAt: "2030-01-01T00:00:01Z",
      }),
    });
    first.readyState = 3;
    first.emit("close", { code: 1006, reason: "", wasClean: false });

    vi.advanceTimersByTime(1_000);
    expect(state.phase).toBe("revoked");
    expect(state.revocationReason).toBe("expired");
    const connectionsAtExpiry = FakeWebSocket.instances.length;
    vi.advanceTimersByTime(30_000);
    expect(FakeWebSocket.instances).toHaveLength(connectionsAtExpiry);
  });

  it("retries hello when the initial authoritative snapshot is unavailable", () => {
    vi.useFakeTimers();
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const remote = new ForgeRemoteSocket(() => {});
    remote.connect();
    const socket = FakeWebSocket.instances[0];
    socket.readyState = FakeWebSocket.OPEN;
    socket.emit("open");
    socket.emit("message", {
      data: JSON.stringify({
        type: "error",
        protocolVersion: 1,
        error: {
          code: "snapshotUnavailable",
          message: "Forge is still preparing the session snapshot.",
          retryable: true,
        },
      }),
    });

    vi.advanceTimersByTime(1_000);
    const sent = socket.sent.map((payload) => JSON.parse(payload));
    expect(sent.filter((message) => message.type === "hello")).toHaveLength(2);
    expect(sent.some((message) => message.command?.type === "resync")).toBe(false);
    remote.stop();
  });

  it("retries an established resync after a retryable snapshot error", () => {
    vi.useFakeTimers();
    vi.stubGlobal("WebSocket", FakeWebSocket);
    const remote = new ForgeRemoteSocket(() => {});
    remote.connect();
    const socket = FakeWebSocket.instances[0];
    socket.readyState = FakeWebSocket.OPEN;
    socket.emit("open");
    socket.emit("message", {
      data: JSON.stringify({
        type: "snapshot",
        protocolVersion: 1,
        revision: 1,
        session: sessionFixture(),
      }),
    });
    socket.emit("message", {
      data: JSON.stringify({
        type: "commandResult",
        protocolVersion: 1,
        commandId: "resync-1",
        outcome: {
          status: "error",
          error: {
            code: "snapshotUnavailable",
            message: "Forge is still preparing the session snapshot.",
            retryable: true,
          },
        },
      }),
    });

    vi.advanceTimersByTime(1_000);
    expect(socket.sent.map((payload) => JSON.parse(payload))).toContainEqual(
      expect.objectContaining({ type: "command", command: { type: "resync" } }),
    );
    remote.stop();
  });
});

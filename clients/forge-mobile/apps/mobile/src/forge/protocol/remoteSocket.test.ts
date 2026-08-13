import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("expo-crypto", () => {
  let nextCommand = 0;
  return { randomUUID: vi.fn(() => `native-command-${nextCommand++}`) };
});

import type { RemoteSessionSnapshot } from "./protocol";
import {
  ForgeRemoteSocket,
  type ForgeRemoteSocketState,
  type RemoteRevocationReason,
} from "./remoteSocket";

type Listener = (event: Record<string, unknown>) => void;

class FakeWebSocket {
  static readonly OPEN = 1;
  static instances: FakeWebSocket[] = [];

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

  close(code = 1000, reason = ""): void {
    this.readyState = 3;
    this.emit("close", { code, reason, wasClean: true });
  }

  emit(type: string, event: Record<string, unknown> = {}): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

function session(overrides: Partial<RemoteSessionSnapshot> = {}): RemoteSessionSnapshot {
  return {
    sessionId: "session-123",
    title: "Forge Remote",
    cwd: "/workspace/forge",
    status: "idle",
    transcript: [],
    availableModels: [{ id: "gpt-5", label: "GPT-5" }],
    activeInteractions: [],
    capabilities: {
      prompt: true,
      cancel: true,
      setModel: true,
      reasoning: true,
      btw: true,
      usage: true,
      resolveInteractions: true,
    },
    ...overrides,
  };
}

function harness() {
  const states: ForgeRemoteSocketState[] = [];
  const revoked: RemoteRevocationReason[] = [];
  const remote = new ForgeRemoteSocket(
    `https://forge.example-tailnet.ts.net/forge/${"a".repeat(64)}/`,
    {
      onChange: (state) => states.push(state),
      onRevoked: (reason) => revoked.push(reason),
    },
  );
  remote.connect();
  const socket = FakeWebSocket.instances[0]!;
  socket.readyState = FakeWebSocket.OPEN;
  socket.emit("open");
  return { remote, socket, states, revoked };
}

function receive(socket: FakeWebSocket, message: unknown): void {
  socket.emit("message", { data: JSON.stringify(message) });
}

function connectAndSnapshot(socket: FakeWebSocket, revision = 4): void {
  receive(socket, {
    type: "connected",
    protocolVersion: 1,
    sessionId: "session-123",
    expiresAt: "2099-01-01T00:00:00Z",
  });
  receive(socket, {
    type: "snapshot",
    protocolVersion: 1,
    revision,
    session: session(),
  });
}

beforeEach(() => {
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  FakeWebSocket.instances = [];
});

describe("Forge native remote socket", () => {
  it("binds to the exact immutable pairing and applies only continuous deltas", () => {
    const { remote, socket, states } = harness();
    expect(socket.url).toBe(`wss://forge.example-tailnet.ts.net/forge/${"a".repeat(64)}/events`);
    connectAndSnapshot(socket, 4);
    receive(socket, {
      type: "delta",
      protocolVersion: 1,
      baseRevision: 4,
      revision: 5,
      event: { kind: "stateReplaced", session: session({ status: "running" }) },
    });
    expect(states.at(-1)?.revision).toBe(5);
    expect(states.at(-1)?.snapshot?.status).toBe("running");
    remote.stop();
  });

  it("rejects revision regression and resyncs rather than applying a gap", () => {
    const { remote, socket, states } = harness();
    connectAndSnapshot(socket, 4);
    receive(socket, {
      type: "delta",
      protocolVersion: 1,
      baseRevision: 4,
      revision: 3,
      event: { kind: "stateReplaced", session: session({ status: "error" }) },
    });
    expect(states.at(-1)?.revision).toBe(4);
    receive(socket, {
      type: "delta",
      protocolVersion: 1,
      baseRevision: 2,
      revision: 5,
      event: { kind: "stateReplaced", session: session({ status: "running" }) },
    });
    expect(states.at(-1)?.revision).toBe(4);
    expect(socket.sent.map(JSON.parse)).toContainEqual(
      expect.objectContaining({ type: "command", command: { type: "resync" } }),
    );
    remote.stop();
  });

  it("pins connected.sessionId and fails terminally on snapshot or delta mismatch", () => {
    vi.useFakeTimers();
    const { socket, states } = harness();
    receive(socket, {
      type: "connected",
      protocolVersion: 1,
      sessionId: "session-123",
      expiresAt: "2099-01-01T00:00:00Z",
    });
    receive(socket, {
      type: "snapshot",
      protocolVersion: 1,
      revision: 1,
      session: session({ sessionId: "different-session" }),
    });
    expect(states.at(-1)?.phase).toBe("error");
    expect(states.at(-1)?.error).toMatch(/different session/i);
    vi.advanceTimersByTime(60_000);
    expect(FakeWebSocket.instances).toHaveLength(1);
  });

  it("uses hello before the first snapshot and resync after an established snapshot", () => {
    vi.useFakeTimers();
    const { remote, socket } = harness();
    receive(socket, {
      type: "error",
      protocolVersion: 1,
      error: { code: "snapshotUnavailable", message: "preparing", retryable: true },
    });
    vi.advanceTimersByTime(1_000);
    expect(socket.sent.map(JSON.parse).filter((message) => message.type === "hello")).toHaveLength(
      2,
    );
    connectAndSnapshot(socket, 1);
    receive(socket, {
      type: "commandResult",
      protocolVersion: 1,
      commandId: "resync-1",
      outcome: {
        status: "error",
        error: { code: "snapshotUnavailable", message: "preparing", retryable: true },
      },
    });
    vi.advanceTimersByTime(1_000);
    expect(socket.sent.map(JSON.parse)).toContainEqual(
      expect.objectContaining({ type: "command", command: { type: "resync" } }),
    );
    remote.stop();
  });

  it("clears unacknowledged user commands on an authoritative reconnect snapshot", () => {
    const { remote, socket, states } = harness();
    connectAndSnapshot(socket, 1);
    expect(remote.sendPrompt("continue")).not.toBeNull();
    expect(states.at(-1)?.pendingCommandCount).toBe(1);
    receive(socket, {
      type: "snapshot",
      protocolVersion: 1,
      revision: 2,
      session: session({ status: "running" }),
    });
    expect(states.at(-1)?.pendingCommandCount).toBe(0);
    remote.stop();
  });

  it("locks only the pending interaction and model command until acknowledgement", () => {
    const { remote, socket, states } = harness();
    connectAndSnapshot(socket, 1);
    const interactionCommand = remote.resolveInteraction("interaction-1", {
      kind: "permission",
      optionId: "exact-option",
    });
    expect(interactionCommand).not.toBeNull();
    expect(remote.resolveInteraction("interaction-1", { kind: "cancel" })).toBeNull();
    expect(states.at(-1)?.pendingInteractionIds).toEqual(["interaction-1"]);
    expect(remote.resolveInteraction("interaction-2", { kind: "cancel" })).not.toBeNull();

    const modelCommand = remote.setModel("gpt-5", "high");
    expect(modelCommand).not.toBeNull();
    expect(remote.setModel("gpt-5", "low")).toBeNull();
    expect(states.at(-1)?.modelCommandPending).toBe(true);

    receive(socket, {
      type: "commandResult",
      protocolVersion: 1,
      commandId: interactionCommand,
      outcome: { status: "ok" },
    });
    expect(states.at(-1)?.pendingInteractionIds).toEqual(["interaction-2"]);
    receive(socket, {
      type: "commandResult",
      protocolVersion: 1,
      commandId: modelCommand,
      outcome: { status: "ok" },
    });
    expect(states.at(-1)?.modelCommandPending).toBe(false);
    remote.stop();
  });

  it("deduplicates usage refresh and relies on authoritative loading state", () => {
    const { remote, socket, states } = harness();
    connectAndSnapshot(socket, 1);
    const usageCommand = remote.refreshUsage();
    expect(usageCommand).not.toBeNull();
    expect(remote.refreshUsage()).toBeNull();
    expect(states.at(-1)?.usageCommandPending).toBe(true);
    expect(socket.sent.map(JSON.parse)).toContainEqual(
      expect.objectContaining({ command: { type: "refreshUsage" } }),
    );

    receive(socket, {
      type: "commandResult",
      protocolVersion: 1,
      commandId: usageCommand,
      outcome: { status: "ok" },
    });
    expect(states.at(-1)?.usageCommandPending).toBe(false);
    receive(socket, {
      type: "delta",
      protocolVersion: 1,
      baseRevision: 1,
      revision: 2,
      event: {
        kind: "stateReplaced",
        session: session({ usage: { status: "loading" } }),
      },
    });
    expect(remote.refreshUsage()).toBeNull();
    remote.stop();
  });

  it("gates model and usage commands on authoritative capabilities and pending state", () => {
    const { remote, socket } = harness();
    connectAndSnapshot(socket, 1);
    receive(socket, {
      type: "delta",
      protocolVersion: 1,
      baseRevision: 1,
      revision: 2,
      event: {
        kind: "stateReplaced",
        session: session({
          modelSwitchPending: true,
          capabilities: { ...session().capabilities, usage: false },
        }),
      },
    });
    expect(remote.setModel("gpt-5", null)).toBeNull();
    expect(remote.refreshUsage()).toBeNull();
    remote.stop();
  });

  it("sends pings without exposing them as user commands", () => {
    vi.useFakeTimers();
    const { remote, socket, states } = harness();
    connectAndSnapshot(socket, 1);
    vi.advanceTimersByTime(20_000);
    expect(socket.sent.map(JSON.parse)).toContainEqual(
      expect.objectContaining({ type: "command", command: { type: "ping" } }),
    );
    expect(states.at(-1)?.pendingCommandCount).toBe(0);
    remote.stop();
  });

  it("releases ownership while backgrounded, keeps its snapshot, and ignores stale socket events", () => {
    vi.useFakeTimers();
    const { remote, socket: firstSocket, states } = harness();
    connectAndSnapshot(firstSocket, 4);

    remote.suspend();
    expect(states.at(-1)).toMatchObject({
      phase: "reconnecting",
      error: null,
      revision: 4,
      snapshot: expect.objectContaining({ sessionId: "session-123" }),
    });
    vi.advanceTimersByTime(30_000);
    expect(FakeWebSocket.instances).toHaveLength(1);

    remote.resume();
    expect(FakeWebSocket.instances).toHaveLength(2);
    const secondSocket = FakeWebSocket.instances[1]!;
    secondSocket.readyState = FakeWebSocket.OPEN;
    secondSocket.emit("open");

    receive(firstSocket, {
      type: "snapshot",
      protocolVersion: 1,
      revision: 99,
      session: session({ status: "error" }),
    });
    firstSocket.emit("close", { code: 1006, reason: "stale close", wasClean: false });
    expect(states.at(-1)?.revision).toBe(4);

    connectAndSnapshot(secondSocket, 5);
    expect(states.at(-1)).toMatchObject({ phase: "connected", error: null, revision: 5 });
    remote.stop();
  });

  it("treats latest-active supersession as quiet passive ownership loss", () => {
    vi.useFakeTimers();
    const { remote, socket, states } = harness();
    connectAndSnapshot(socket, 3);

    socket.emit("close", {
      code: 4410,
      reason: "superseded by latest active client",
      wasClean: true,
    });
    expect(states.at(-1)).toMatchObject({
      phase: "reconnecting",
      error: null,
      revision: 3,
      snapshot: expect.objectContaining({ sessionId: "session-123" }),
    });
    expect(JSON.stringify(states)).not.toMatch(/failed to connect|already open|conflict/i);
    vi.advanceTimersByTime(30_000);
    expect(FakeWebSocket.instances).toHaveLength(1);

    remote.resume();
    expect(FakeWebSocket.instances).toHaveLength(2);
    remote.stop();
  });

  it("does not automatically reconnect after conflict, incompatibility, or expiry", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2030-01-01T00:00:00Z"));
    const first = harness();
    first.socket.emit("close", {
      code: 4409,
      reason: "already connected",
      wasClean: true,
    });
    expect(first.states.at(-1)).toMatchObject({ phase: "reconnecting", error: null });
    vi.advanceTimersByTime(30_000);
    expect(FakeWebSocket.instances).toHaveLength(1);

    FakeWebSocket.instances = [];
    const second = harness();
    receive(second.socket, {
      type: "connected",
      protocolVersion: 1,
      sessionId: "session-123",
      expiresAt: "2030-01-01T00:00:01Z",
    });
    vi.advanceTimersByTime(1_000);
    expect(second.revoked).toEqual(["expired"]);
    vi.advanceTimersByTime(30_000);
    expect(FakeWebSocket.instances).toHaveLength(1);
  });
});

import { describe, expect, it } from "vitest";
import { decodeServerMessage } from "../src/protocol";
import {
  initialRemoteClientState,
  remoteClientReducer,
  type RemoteClientState,
} from "../src/reducer";
import { sessionFixture, usageFixture } from "./fixtures";

function withSnapshot(revision = 4): RemoteClientState {
  const connected = remoteClientReducer(initialRemoteClientState, {
    type: "serverMessage",
    message: {
      type: "connected",
      protocolVersion: 1,
      sessionId: "session-123",
      expiresAt: "2030-01-01T00:00:00Z",
    },
  });
  return remoteClientReducer(connected, {
    type: "serverMessage",
    message: {
      type: "snapshot",
      protocolVersion: 1,
      revision,
      session: sessionFixture(),
    },
  });
}

describe("Forge Remote protocol reducer", () => {
  it("does not become live before an authoritative snapshot", () => {
    const connected = remoteClientReducer(initialRemoteClientState, {
      type: "serverMessage",
      message: {
        type: "connected",
        protocolVersion: 1,
        sessionId: "session-123",
        expiresAt: "2030-01-01T00:00:00Z",
      },
    });
    expect(connected.phase).toBe("syncing");
    expect(connected.session).toBeUndefined();
    const live = withSnapshot();
    expect(live.phase).toBe("live");
    expect(live.revision).toBe(4);
    expect(live.session?.transcript).toHaveLength(2);
  });

  it("rejects a revision gap and asks for a fresh snapshot", () => {
    const live = withSnapshot(4);
    const next = remoteClientReducer(live, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 2,
        revision: 5,
        event: { kind: "stateReplaced", session: sessionFixture({ status: "running" }) },
      },
    });
    expect(next.phase).toBe("resyncing");
    expect(next.needsResync).toBe(true);
    expect(next.revision).toBe(4);
    expect(next.session?.status).toBe("idle");
  });

  it("applies a full replacement only against the current revision", () => {
    const live = withSnapshot(4);
    const next = remoteClientReducer(live, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "stateReplaced",
          session: sessionFixture({
            status: "running",
            transcript: [
              ...sessionFixture().transcript,
              { id: "u2", kind: "user", text: "From the phone", status: "complete" },
            ],
          }),
        },
      },
    });
    expect(next.phase).toBe("live");
    expect(next.revision).toBe(5);
    expect(next.session?.status).toBe("running");
    expect(next.session?.transcript.at(-1)?.id).toBe("u2");
  });

  it("applies a bounded transcript splice without replacing unrelated session state", () => {
    const live = withSnapshot(4);
    const next = remoteClientReducer(live, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "transcriptSpliced",
          sessionId: "session-123",
          start: 1,
          deleteCount: 1,
          items: [{ id: "a1", kind: "assistant", text: "Streaming now", status: "running" }],
        },
      },
    });
    expect(next.phase).toBe("live");
    expect(next.revision).toBe(5);
    expect(next.session?.title).toBe("Remote feature");
    expect(next.session?.transcript.map((item) => item.id)).toEqual(["u1", "a1"]);
    expect(next.session?.transcript[1]).toMatchObject({ text: "Streaming now", status: "running" });
  });

  it("requests authoritative state for an out-of-bounds transcript splice", () => {
    const next = remoteClientReducer(withSnapshot(4), {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "transcriptSpliced",
          sessionId: "session-123",
          start: 3,
          deleteCount: 0,
          items: [],
        },
      },
    });
    expect(next.phase).toBe("resyncing");
    expect(next.needsResync).toBe(true);
    expect(next.revision).toBe(4);
    expect(next.lastError).toMatch(/invalid transcript update/i);
  });

  it("fails closed when a transcript splice names another session", () => {
    const next = remoteClientReducer(withSnapshot(4), {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "transcriptSpliced",
          sessionId: "different-session",
          start: 0,
          deleteCount: 0,
          items: [],
        },
      },
    });
    expect(next.phase).toBe("incompatible");
    expect(next.revision).toBe(4);
  });

  it("fails closed when a delta carries another session", () => {
    const live = withSnapshot(4);
    const next = remoteClientReducer(live, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "stateReplaced",
          session: sessionFixture({ sessionId: "different-session" }),
        },
      },
    });
    expect(next.phase).toBe("incompatible");
    expect(next.session?.sessionId).toBe("session-123");
  });

  it("fails closed when a reconnect announces another session", () => {
    const live = withSnapshot(4);
    const reconnecting = remoteClientReducer(live, {
      type: "socketClosed",
      code: 1006,
      reason: "",
      wasClean: false,
    });
    const next = remoteClientReducer(reconnecting, {
      type: "serverMessage",
      message: {
        type: "connected",
        protocolVersion: 1,
        sessionId: "different-session",
        expiresAt: "2030-01-01T00:00:00Z",
      },
    });
    expect(next.phase).toBe("incompatible");
    expect(next.sessionId).toBe("session-123");
    expect(next.session?.sessionId).toBe("session-123");
  });

  it("treats a gateway session mismatch as terminal across socket close", () => {
    const mismatch = remoteClientReducer(withSnapshot(), {
      type: "serverMessage",
      message: {
        type: "error",
        protocolVersion: 1,
        error: {
          code: "sessionMismatch",
          message: "Forge rejected state for another session.",
          retryable: false,
        },
      },
    });
    expect(mismatch.phase).toBe("incompatible");
    const closed = remoteClientReducer(mismatch, {
      type: "socketClosed",
      code: 1006,
      reason: "",
      wasClean: false,
    });
    expect(closed.phase).toBe("incompatible");
  });

  it("tracks a command without optimistically changing the transcript", () => {
    const live = withSnapshot();
    const queued = remoteClientReducer(live, {
      type: "commandQueued",
      commandId: "cmd-1",
      command: { type: "prompt", label: "Sending message" },
    });
    expect(queued.session?.transcript).toEqual(live.session?.transcript);
    expect(queued.pendingCommands["cmd-1"]).toBeDefined();
    const acknowledged = remoteClientReducer(queued, {
      type: "serverMessage",
      message: {
        type: "commandResult",
        protocolVersion: 1,
        commandId: "cmd-1",
        outcome: { status: "ok" },
      },
    });
    expect(acknowledged.pendingCommands).toEqual({});
  });

  it("keeps a successful queue command pending until authoritative queue state changes", () => {
    const live = {
      ...withSnapshot(4),
      session: sessionFixture({
        queue: [
          {
            id: "queued-1",
            text: "Ship it",
            position: 1,
            source: "shared",
            version: 7,
            actions: { edit: true, steer: true, cancel: true },
          },
        ],
      }),
    };
    const queued = remoteClientReducer(live, {
      type: "commandQueued",
      commandId: "queue-1",
      command: {
        type: "queue",
        label: "Steering with queued message",
        queueItemId: "queued-1",
        expectedVersion: 7,
      },
    });
    const acknowledged = remoteClientReducer(queued, {
      type: "serverMessage",
      message: {
        type: "commandResult",
        protocolVersion: 1,
        commandId: "queue-1",
        outcome: { status: "ok" },
      },
    });
    expect(acknowledged.pendingCommands["queue-1"]).toBeDefined();

    const reconciled = remoteClientReducer(acknowledged, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "stateReplaced",
          session: sessionFixture({ queue: [] }),
        },
      },
    });
    expect(reconciled.pendingCommands).toEqual({});
  });

  it("treats command acknowledgement as scheduling and waits for authoritative usage state", () => {
    const loadingUsage = usageFixture({ status: "loading", refreshedAt: "2030-01-01T00:00:00Z" });
    const live = withSnapshot();
    const queued = remoteClientReducer(live, {
      type: "commandQueued",
      commandId: "usage-1",
      command: { type: "refreshUsage", label: "Refreshing usage" },
    });
    const acknowledged = remoteClientReducer(queued, {
      type: "serverMessage",
      message: {
        type: "commandResult",
        protocolVersion: 1,
        commandId: "usage-1",
        outcome: { status: "ok" },
      },
    });
    expect(acknowledged.pendingCommands).toEqual({});
    expect(acknowledged.session?.usage).toEqual(live.session?.usage);

    const loading = remoteClientReducer(acknowledged, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: { kind: "stateReplaced", session: sessionFixture({ usage: loadingUsage }) },
      },
    });
    expect(loading.session?.usage?.status).toBe("loading");
    expect(loading.session?.usage?.context?.usedTokens).toBe(96_000);

    const ready = remoteClientReducer(loading, {
      type: "serverMessage",
      message: {
        type: "delta",
        protocolVersion: 1,
        baseRevision: 5,
        revision: 6,
        event: {
          kind: "stateReplaced",
          session: sessionFixture({
            usage: usageFixture({
              status: "ready",
              refreshedAt: "2030-01-01T00:00:02Z",
              context: {
                usedTokens: 100_000,
                totalTokens: 200_000,
                freeTokens: 100_000,
                usedPercent: 50,
                autoCompactPercent: 90,
              },
            }),
          }),
        },
      },
    });
    expect(ready.session?.usage?.status).toBe("ready");
    expect(ready.session?.usage?.context?.usedTokens).toBe(100_000);
  });

  it("reconciles an unacknowledged command from the reconnect snapshot", () => {
    const queued = remoteClientReducer(withSnapshot(), {
      type: "commandQueued",
      commandId: "lost-ack",
      command: { type: "prompt", label: "Sending message" },
    });
    const disconnected = remoteClientReducer(queued, {
      type: "socketClosed",
      code: 1006,
      reason: "",
      wasClean: false,
    });
    expect(disconnected.pendingCommands).toEqual({});
    const reconnected = remoteClientReducer(disconnected, {
      type: "serverMessage",
      message: {
        type: "snapshot",
        protocolVersion: 1,
        revision: 5,
        session: sessionFixture({ status: "running" }),
      },
    });
    expect(reconnected.phase).toBe("live");
    expect(reconnected.pendingCommands).toEqual({});
    expect(reconnected.session?.status).toBe("running");
  });

  it("keeps a superseded browser passive without conflict or pairing loss", () => {
    const next = remoteClientReducer(withSnapshot(), {
      type: "socketClosed",
      code: 4410,
      reason: "superseded by a newer active client",
      wasClean: true,
    });
    expect(next.phase).toBe("passive");
    expect(next.session?.sessionId).toBe("session-123");
    expect(next.lastError).toBeUndefined();
  });

  it("stops reconnecting after explicit revocation", () => {
    const revoked = remoteClientReducer(withSnapshot(), {
      type: "serverMessage",
      message: { type: "revoked", protocolVersion: 1, reason: "expired" },
    });
    expect(revoked.phase).toBe("revoked");
    const close = remoteClientReducer(revoked, {
      type: "socketClosed",
      code: 1000,
      reason: "",
      wasClean: true,
    });
    expect(close.phase).toBe("revoked");
  });

  it("decodes the gateway envelope and canonical camel-case event", () => {
    const message = decodeServerMessage(
      JSON.stringify({
        type: "delta",
        protocolVersion: 1,
        baseRevision: 9,
        revision: 10,
        event: { kind: "stateReplaced", session: sessionFixture() },
      }),
    );
    expect(message.type).toBe("delta");
    if (message.type !== "delta") throw new Error("expected delta");
    expect(message.event.kind).toBe("stateReplaced");
    if (message.event.kind !== "stateReplaced") throw new Error("expected replacement");
    expect(message.baseRevision).toBe(9);
    expect(message.event.session.capabilities.usage).toBe(true);
    expect(message.event.session.usage?.session?.costUsdTicks).toBe("1250000000");
    expect(message.event.session.availableModels[0]?.reasoningEffort?.options[2]?.id).toBe("high");
  });

  it("decodes work disclosure, fast-mode, and actionable queue metadata", () => {
    const serialized = sessionFixture({
      transcript: [
        { id: "thought", kind: "reasoning", text: "Checking", status: "complete" },
        {
          id: "worked",
          kind: "system",
          text: "Worked for 2m 5s",
          workDisclosure: {
            durationMs: 125_000,
            finalResponseItemId: "answer",
            workItemIds: ["thought"],
          },
        },
        { id: "answer", kind: "assistant", text: "Done", status: "complete" },
      ],
      queue: [
        {
          id: "queued-1",
          text: "Then test",
          position: 1,
          source: "shared",
          version: 3,
          kind: "prompt",
          actions: { edit: true, steer: true, cancel: false },
        },
      ],
      fastMode: { supported: true, enabled: true, pending: false },
    });
    const message = decodeServerMessage(JSON.stringify({
      type: "snapshot",
      protocolVersion: 1,
      revision: 1,
      session: serialized,
    }));
    expect(message.type).toBe("snapshot");
    if (message.type !== "snapshot") throw new Error("expected snapshot");
    const disclosure = message.session.transcript.find((item) => item.id === "worked");
    expect(disclosure?.kind).toBe("system");
    if (disclosure?.kind !== "system") throw new Error("expected disclosure marker");
    expect(disclosure.workDisclosure).toEqual({
      durationMs: 125_000,
      finalResponseItemId: "answer",
      workItemIds: ["thought"],
    });
    expect(message.session.fastMode).toEqual({ supported: true, enabled: true, pending: false });
    expect(message.session.capabilities).toMatchObject({
      fastMode: true,
      queueControl: true,
      newSession: true,
    });
    expect(message.session.queue?.[0]).toMatchObject({
      source: "shared",
      version: 3,
      kind: "prompt",
      actions: { edit: true, steer: true, cancel: false },
    });
  });

  it("decodes transcript-splice and child-session server messages", () => {
    const splice = decodeServerMessage(JSON.stringify({
      type: "delta",
      protocolVersion: 1,
      baseRevision: 1,
      revision: 2,
      event: {
        kind: "transcriptSpliced",
        sessionId: "session-123",
        start: 2,
        deleteCount: 0,
        items: [{ id: "a2", kind: "assistant", text: "More", status: "running" }],
      },
    }));
    expect(splice).toMatchObject({
      type: "delta",
      event: { kind: "transcriptSpliced", sessionId: "session-123", start: 2 },
    });

    const created = decodeServerMessage(JSON.stringify({
      type: "sessionCreated",
      protocolVersion: 1,
      commandId: "new-1",
      sessionId: "session-child",
      pairingUrl: `https://forge.example/forge/${"b".repeat(64)}/`,
      expiresAt: "2030-01-01T00:00:00Z",
    }));
    expect(created).toMatchObject({
      type: "sessionCreated",
      commandId: "new-1",
      sessionId: "session-child",
    });
  });

  it("keeps additive capabilities and queue metadata backward-compatible", () => {
    const serialized = JSON.parse(JSON.stringify(sessionFixture())) as {
      capabilities: Record<string, unknown>;
      queue: Array<Record<string, unknown>>;
    };
    delete serialized.capabilities.usage;
    delete serialized.capabilities.fastMode;
    delete serialized.capabilities.queueControl;
    delete serialized.capabilities.newSession;
    serialized.queue = [{ id: "old-queue", text: "Old prompt", position: 1 }];
    const message = decodeServerMessage(JSON.stringify({
      type: "snapshot",
      protocolVersion: 1,
      revision: 1,
      session: serialized,
    }));
    expect(message.type).toBe("snapshot");
    if (message.type !== "snapshot") throw new Error("expected snapshot");
    expect(message.session.capabilities.usage).toBe(false);
    expect(message.session.capabilities.fastMode).toBe(false);
    expect(message.session.capabilities.queueControl).toBe(false);
    expect(message.session.capabilities.newSession).toBe(false);
    expect(message.session.queue?.[0]).toMatchObject({
      source: "local",
      actions: { edit: false, steer: false, cancel: false },
    });
  });

  it("rejects malformed usage cost ticks before rendering", () => {
    const serialized = JSON.parse(JSON.stringify(sessionFixture())) as {
      usage: { session: { costUsdTicks: string } };
    };
    serialized.usage.session.costUsdTicks = "$0.12";
    expect(() => decodeServerMessage(JSON.stringify({
      type: "snapshot",
      protocolVersion: 1,
      revision: 1,
      session: serialized,
    }))).toThrow(/cost ticks/i);
  });

  it("rejects legacy event aliases instead of guessing at wire state", () => {
    expect(() =>
      decodeServerMessage(
        JSON.stringify({
          type: "delta",
          protocolVersion: 1,
          baseRevision: 9,
          revision: 10,
          event: { kind: "state_replaced", session: sessionFixture() },
        }),
      ),
    ).toThrow(/unknown delta event/i);
  });

  it("rejects malformed nested snapshots before rendering them", () => {
    const invalid = sessionFixture({
      activeInteractions: [
        {
          interactionId: "permission:opaque",
          kind: "permission",
          options: [{ id: "once", label: "Allow once" }],
        },
      ],
    });
    const serialized = JSON.parse(JSON.stringify(invalid)) as Record<string, unknown>;
    serialized.activeInteractions = [{ id: "raw-acp-id", kind: "permission", options: [] }];
    expect(() =>
      decodeServerMessage(
        JSON.stringify({
          type: "snapshot",
          protocolVersion: 1,
          revision: 1,
          session: serialized,
        }),
      ),
    ).toThrow(/interactionId/i);
  });

  it("fails safely on a protocol version mismatch", () => {
    const state = remoteClientReducer(initialRemoteClientState, {
      type: "serverMessage",
      message: {
        type: "connected",
        protocolVersion: 2,
        sessionId: "session-123",
        expiresAt: "2030-01-01T00:00:00Z",
      },
    });
    expect(state.phase).toBe("incompatible");
    expect(state.lastError).toMatch(/protocol 1/i);
  });
});

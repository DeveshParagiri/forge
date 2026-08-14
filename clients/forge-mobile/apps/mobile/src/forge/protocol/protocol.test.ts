import { describe, expect, it } from "vitest";

import { ProtocolDecodeError, decodeServerMessage } from "./protocol";

function message(overrides: Record<string, unknown> = {}) {
  return JSON.stringify({
    type: "snapshot",
    protocolVersion: 1,
    revision: 1,
    session: {
      sessionId: "session-1",
      status: "idle",
      transcript: [],
      availableModels: [
        {
          id: "gpt-5.6-sol",
          label: "GPT-5.6 Sol",
          reasoningEffort: {
            options: [
              { id: "low", label: "Low" },
              { id: "high", label: "High", description: "More reasoning" },
            ],
          },
        },
      ],
      activeInteractions: [],
      capabilities: {
        prompt: true,
        cancel: true,
        setModel: true,
        fastMode: true,
        reasoning: true,
        btw: true,
        usage: true,
        resolveInteractions: true,
      },
      fastMode: { supported: true, enabled: true, pending: false },
      usage: {
        status: "partial",
        refreshedAt: "2026-08-12T21:30:00Z",
        context: {
          usedTokens: 3200,
          totalTokens: 200000,
          freeTokens: 196800,
          usedPercent: 1.6,
          autoCompactPercent: 90,
        },
        session: {
          inputTokens: 2000,
          cachedReadTokens: 500,
          cacheCreationTokens: 100,
          outputTokens: 800,
          reasoningTokens: 300,
          totalTokens: 3200,
          modelCalls: 4,
          apiDurationMs: 12500,
          costUsdTicks: "123400000",
          costState: "exact",
          incomplete: true,
          models: [
            {
              modelId: "gpt-5.6-sol",
              inputTokens: 2000,
              cachedReadTokens: 500,
              cacheCreationTokens: 100,
              outputTokens: 800,
              reasoningTokens: 300,
              totalTokens: 3200,
              modelCalls: 4,
              apiDurationMs: 12500,
              costUsdTicks: "123400000",
              costState: "exact",
            },
          ],
        },
        account: {
          provider: "OpenAI Codex",
          status: "ready",
          plan: "Plus",
          allowed: true,
          windows: [
            {
              label: "Weekly limit",
              usedPercent: 60,
              windowSeconds: 604800,
              resetAt: 1786651200,
              resetLabel: "tomorrow",
            },
          ],
          credits: { balance: "0", unlimited: false },
        },
        errors: { account: "One quota window is delayed." },
      },
      ...overrides,
    },
  });
}

describe("Forge native protocol usage", () => {
  it("decodes bounded transcript splice deltas for near-real-time updates", () => {
    const decoded = decodeServerMessage(
      JSON.stringify({
        type: "delta",
        protocolVersion: 1,
        baseRevision: 4,
        revision: 5,
        event: {
          kind: "transcriptSpliced",
          sessionId: "session-1",
          start: 1,
          deleteCount: 1,
          items: [{ id: "answer", kind: "assistant", text: "Streaming now" }],
        },
      }),
    );
    expect(decoded).toEqual({
      type: "delta",
      protocolVersion: 1,
      baseRevision: 4,
      revision: 5,
      event: {
        kind: "transcriptSpliced",
        sessionId: "session-1",
        start: 1,
        deleteCount: 1,
        items: [{ id: "answer", kind: "assistant", text: "Streaming now" }],
      },
    });
  });

  it("rejects malformed transcript splice coordinates and items", () => {
    for (const event of [
      {
        kind: "transcriptSpliced",
        sessionId: "session-1",
        start: -1,
        deleteCount: 0,
        items: [],
      },
      {
        kind: "transcriptSpliced",
        sessionId: "session-1",
        start: 0,
        deleteCount: 0,
        items: [{ id: "answer", kind: "not-a-kind", text: "bad" }],
      },
    ]) {
      expect(() =>
        decodeServerMessage(
          JSON.stringify({
            type: "delta",
            protocolVersion: 1,
            baseRevision: 4,
            revision: 5,
            event,
          }),
        ),
      ).toThrow(ProtocolDecodeError);
    }
  });

  it("decodes additive typed work-disclosure metadata without requiring it on old items", () => {
    const decoded = decodeServerMessage(
      message({
        transcript: [
          { id: "old-system", kind: "system", text: "Recap" },
          {
            id: "turn-complete",
            kind: "system",
            text: "Worked for 3m41s",
            workDisclosure: {
              durationMs: 221_000,
              finalResponseItemId: "answer",
              workItemIds: ["reasoning", "tool"],
            },
          },
        ],
      }),
    );
    expect(decoded.type).toBe("snapshot");
    if (decoded.type !== "snapshot") throw new Error("Expected snapshot");
    expect(decoded.session.transcript[0]).not.toHaveProperty("workDisclosure");
    expect(decoded.session.transcript[1]).toMatchObject({
      workDisclosure: {
        durationMs: 221_000,
        finalResponseItemId: "answer",
        workItemIds: ["reasoning", "tool"],
      },
    });
  });

  it("rejects malformed work-disclosure metadata", () => {
    expect(() =>
      decodeServerMessage(
        message({
          transcript: [
            {
              id: "turn-complete",
              kind: "system",
              text: "Worked",
              workDisclosure: {
                durationMs: 120_000,
                finalResponseItemId: "answer",
                workItemIds: ["reasoning", 4],
              },
            },
          ],
        }),
      ),
    ).toThrow(ProtocolDecodeError);
  });

  it("decodes usage and target-model reasoning metadata without losing partial state", () => {
    const decoded = decodeServerMessage(message());
    expect(decoded.type).toBe("snapshot");
    if (decoded.type !== "snapshot") throw new Error("Expected snapshot");
    expect(decoded.session.availableModels[0]?.reasoningEffort?.options[1]).toEqual({
      id: "high",
      label: "High",
      description: "More reasoning",
    });
    expect(decoded.session.capabilities.fastMode).toBe(true);
    expect(decoded.session.fastMode).toEqual({ supported: true, enabled: true, pending: false });
    expect(decoded.session.usage?.status).toBe("partial");
    expect(decoded.session.usage?.session?.models?.[0]?.costUsdTicks).toBe("123400000");
    expect(decoded.session.usage?.account?.windows[0]?.resetLabel).toBe("tomorrow");
    expect(decoded.session.usage?.errors?.account).toBe("One quota window is delayed.");
  });

  it("defaults the additive usage capability to false for older Forge snapshots", () => {
    const capabilities = {
      prompt: true,
      cancel: true,
      setModel: true,
      reasoning: true,
      btw: true,
      resolveInteractions: true,
    };
    const decoded = decodeServerMessage(message({ capabilities, usage: undefined }));
    expect(decoded.type === "snapshot" && decoded.session.capabilities.usage).toBe(false);
    expect(decoded.type === "snapshot" && decoded.session.capabilities.fastMode).toBe(false);
    expect(decoded.type === "snapshot" && decoded.session.capabilities.queueControl).toBe(false);
    expect(decoded.type === "snapshot" && decoded.session.capabilities.newSession).toBe(false);
  });

  it("decodes authoritative shared queue controls and safely normalizes old queue rows", () => {
    const decoded = decodeServerMessage(
      message({
        capabilities: {
          prompt: true,
          cancel: true,
          setModel: true,
          fastMode: true,
          reasoning: true,
          btw: true,
          usage: true,
          resolveInteractions: true,
          queueControl: true,
          newSession: true,
        },
        queue: [
          { id: "old", text: "Old client row" },
          {
            id: "shared",
            text: "Ship it",
            position: 2,
            source: "shared",
            version: 7,
            kind: "prompt",
            actions: { edit: true, steer: false, cancel: true },
          },
        ],
      }),
    );
    if (decoded.type !== "snapshot") throw new Error("Expected snapshot");
    expect(decoded.session.capabilities).toMatchObject({ queueControl: true, newSession: true });
    expect(decoded.session.queue).toEqual([
      {
        id: "old",
        text: "Old client row",
        source: "local",
        actions: { edit: false, steer: false, cancel: false },
      },
      {
        id: "shared",
        text: "Ship it",
        position: 2,
        source: "shared",
        version: 7,
        kind: "prompt",
        actions: { edit: true, steer: false, cancel: true },
      },
    ]);
  });

  it("decodes the distinct session-created command success", () => {
    expect(
      decodeServerMessage(
        JSON.stringify({
          type: "sessionCreated",
          protocolVersion: 1,
          commandId: "new-session-1",
          sessionId: "session-2",
          pairingUrl: `https://forge.example/forge/${"b".repeat(64)}/`,
          expiresAt: "2026-08-14T00:00:00Z",
        }),
      ),
    ).toEqual({
      type: "sessionCreated",
      protocolVersion: 1,
      commandId: "new-session-1",
      sessionId: "session-2",
      pairingUrl: `https://forge.example/forge/${"b".repeat(64)}/`,
      expiresAt: "2026-08-14T00:00:00Z",
    });
  });

  it("rejects malformed authoritative Fast Mode state", () => {
    expect(() =>
      decodeServerMessage(message({ fastMode: { supported: true, enabled: "yes" } })),
    ).toThrow(ProtocolDecodeError);
  });

  it("rejects untrustworthy cost tick encodings", () => {
    const raw = JSON.parse(message()) as {
      session: { usage: { session: { costUsdTicks: string } } };
    };
    raw.session.usage.session.costUsdTicks = "1.25";
    expect(() => decodeServerMessage(JSON.stringify(raw))).toThrow(ProtocolDecodeError);
  });
});

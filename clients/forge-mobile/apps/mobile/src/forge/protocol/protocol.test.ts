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
        reasoning: true,
        btw: true,
        usage: true,
        resolveInteractions: true,
      },
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
  it("decodes usage and target-model reasoning metadata without losing partial state", () => {
    const decoded = decodeServerMessage(message());
    expect(decoded.type).toBe("snapshot");
    if (decoded.type !== "snapshot") throw new Error("Expected snapshot");
    expect(decoded.session.availableModels[0]?.reasoningEffort?.options[1]).toEqual({
      id: "high",
      label: "High",
      description: "More reasoning",
    });
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
  });

  it("rejects untrustworthy cost tick encodings", () => {
    const raw = JSON.parse(message()) as {
      session: { usage: { session: { costUsdTicks: string } } };
    };
    raw.session.usage.session.costUsdTicks = "1.25";
    expect(() => decodeServerMessage(JSON.stringify(raw))).toThrow(ProtocolDecodeError);
  });
});

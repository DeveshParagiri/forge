import type { RemoteSessionSnapshot, RemoteUsageSnapshot } from "../src/protocol";

export function usageFixture(overrides: Partial<RemoteUsageSnapshot> = {}): RemoteUsageSnapshot {
  return {
    status: "ready",
    refreshedAt: "2030-01-01T00:00:00Z",
    context: {
      usedTokens: 96_000,
      totalTokens: 200_000,
      freeTokens: 104_000,
      usedPercent: 48,
      autoCompactPercent: 90,
    },
    session: {
      inputTokens: 25_000,
      cachedReadTokens: 8_000,
      cacheCreationTokens: 2_000,
      outputTokens: 5_000,
      reasoningTokens: 1_200,
      totalTokens: 30_000,
      modelCalls: 9,
      apiDurationMs: 42_500,
      costUsdTicks: "1250000000",
      costState: "exact",
      incomplete: false,
      models: [
        {
          modelId: "gpt-5.6-sol",
          inputTokens: 25_000,
          cachedReadTokens: 8_000,
          cacheCreationTokens: 2_000,
          outputTokens: 5_000,
          reasoningTokens: 1_200,
          totalTokens: 30_000,
          modelCalls: 9,
          apiDurationMs: 42_500,
          costUsdTicks: "1250000000",
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
          windowSeconds: 604_800,
          resetAt: 1_893_456_000,
          resetLabel: "Jan 2, 12:00 AM",
        },
      ],
      credits: { balance: "0", unlimited: false },
    },
    ...overrides,
  };
}

export function sessionFixture(
  overrides: Partial<RemoteSessionSnapshot> = {},
): RemoteSessionSnapshot {
  return {
    sessionId: "session-123",
    title: "Remote feature",
    cwd: "/workspace/forge",
    status: "idle",
    transcript: [
      { id: "u1", kind: "user", text: "Keep the terminal active.", status: "complete" },
      {
        id: "a1",
        kind: "assistant",
        text: "The same **Forge session** is available here.",
        status: "complete",
      },
    ],
    currentModel: { id: "gpt-5.6-sol", label: "GPT-5.6 Sol" },
    fastMode: { supported: true, enabled: false },
    availableModels: [
      {
        id: "gpt-5.6-sol",
        label: "GPT-5.6 Sol",
        reasoningEffort: {
          options: [
            { id: "low", label: "Low" },
            { id: "medium", label: "Medium" },
            { id: "high", label: "High" },
          ],
        },
      },
      {
        id: "gpt-5.6-terra",
        label: "GPT-5.6 Terra",
        reasoningEffort: {
          options: [
            { id: "low", label: "Low" },
            { id: "medium", label: "Medium" },
          ],
        },
      },
    ],
    reasoningEffort: {
      current: "medium",
      options: [
        { id: "low", label: "Low" },
        { id: "medium", label: "Medium" },
      ],
    },
    activeInteractions: [],
    queue: [],
    usage: usageFixture(),
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
    ...overrides,
  };
}

import { describe, expect, it } from "vitest";

import type { RemoteSessionSnapshot } from "./protocol/protocol";
import { buildModelOptions } from "../lib/modelOptions";
import {
  assistantResponseMessageIds,
  buildServerConfig,
  presentInteraction,
  presentRemoteTranscript,
  presentRemoteSession,
  reasoningFromSelection,
  remoteApprovalResponse,
} from "./presentationAdapter";

describe("Forge assistant response actions", () => {
  it("keeps one copy target for the final assistant segment in each settled turn", () => {
    expect([
      ...assistantResponseMessageIds(
        [
          { id: "user", kind: "user", text: "Question" },
          { id: "assistant-1", kind: "assistant", text: "First segment" },
          { id: "tool", kind: "tool", title: "Tool", status: "complete" },
          { id: "assistant-2", kind: "assistant", text: "Final segment" },
          { id: "system", kind: "system", text: "Unrelated system note" },
        ],
        false,
      ),
    ]).toEqual(["remote:assistant-2"]);
  });

  it("keeps prior responses copyable while the current turn is running", () => {
    expect([
      ...assistantResponseMessageIds(
        [
          { id: "user-1", kind: "user", text: "First" },
          { id: "answer-1", kind: "assistant", text: "Done" },
          { id: "user-2", kind: "user", text: "Second" },
          { id: "answer-2", kind: "assistant", text: "Streaming", status: "running" },
        ],
        true,
      ),
    ]).toEqual(["remote:answer-1"]);
  });
});

describe("Forge transcript status presentation", () => {
  it("does not invent a live Working timer from snapshot update timestamps", () => {
    const presented = presentRemoteTranscript([], true, "2026-08-13T12:00:00.000Z");
    expect(presented.feed).toEqual([]);
  });

  it("hides an authoritative 1m59.9s marker without hiding its exact work details", () => {
    const presented = presentRemoteTranscript(
      [
        { id: "user", kind: "user", text: "Question" },
        { id: "reasoning", kind: "reasoning", text: "Reasoning" },
        { id: "assistant", kind: "assistant", text: "Answer" },
        {
          id: "worked",
          kind: "system",
          text: "Worked for 1m59.9s",
          workDisclosure: {
            durationMs: 119_900,
            finalResponseItemId: "assistant",
            workItemIds: ["reasoning"],
          },
        },
      ],
      false,
      "2026-08-13T12:00:00.000Z",
    );
    expect(presented.feed.map((entry) => String(entry.id))).toEqual([
      "remote:user",
      "remote:reasoning",
      "remote:assistant",
    ]);
    expect(presented.workDisclosures).toEqual([]);
  });

  it("shows the exact 2m boundary and reorders it before its typed final response", () => {
    const presented = presentRemoteTranscript(
      [
        { id: "user", kind: "user", text: "Question" },
        { id: "assistant", kind: "assistant", text: "Answer" },
        {
          id: "worked",
          kind: "system",
          text: "server label is not presentation truth",
          workDisclosure: {
            durationMs: 120_000,
            finalResponseItemId: "assistant",
            workItemIds: [],
          },
        },
      ],
      false,
      "2026-08-13T12:00:00.000Z",
    );
    expect(presented.feed.map((entry) => String(entry.id))).toEqual([
      "remote:user",
      "remote:worked",
      "remote:assistant",
    ]);
    expect(presented.workDisclosures[0]).toMatchObject({
      label: "Worked for 2m",
      durationMs: 120_000,
    });
    expect(presented.workDisclosures[0]?.hiddenEntryIds.size).toBe(0);
  });

  it("hides only authoritative work IDs and formats 3m41s with readable spacing", () => {
    const presented = presentRemoteTranscript(
      [
        { id: "user", kind: "user", text: "Question" },
        { id: "reasoning", kind: "reasoning", text: "Reasoning" },
        { id: "tool", kind: "tool", title: "Read file", status: "complete" },
        { id: "assistant", kind: "assistant", text: "Answer" },
        {
          id: "worked",
          kind: "system",
          text: "Worked for 3m41s",
          workDisclosure: {
            durationMs: 221_000,
            finalResponseItemId: "assistant",
            workItemIds: ["reasoning", "tool", "missing-item"],
          },
        },
        { id: "recap", kind: "system", text: "Recap — current work" },
      ],
      false,
      "2026-08-13T12:00:00.000Z",
    );
    expect(presented.feed.map((entry) => String(entry.id))).toEqual([
      "remote:user",
      "remote:reasoning",
      "activity:tool",
      "remote:worked",
      "remote:assistant",
      "remote:recap",
    ]);
    expect(presented.workDisclosures).toHaveLength(1);
    expect(presented.workDisclosures[0]).toMatchObject({
      markerMessageId: "remote:worked",
      label: "Worked for 3m 41s",
      durationMs: 221_000,
    });
    expect([...(presented.workDisclosures[0]?.hiddenEntryIds ?? [])]).toEqual([
      "remote:reasoning",
      "activity:tool",
    ]);
  });

  it("leaves matching text and unrelated Recap rows ordinary without typed metadata", () => {
    const presented = presentRemoteTranscript(
      [
        { id: "worked", kind: "system", text: "Worked for 3m41s" },
        { id: "recap", kind: "system", text: "Recap — current work" },
      ],
      false,
      "2026-08-13T12:00:00.000Z",
    );
    expect(presented.feed.map((entry) => String(entry.id))).toEqual([
      "remote:worked",
      "remote:recap",
    ]);
    expect(presented.workDisclosures).toEqual([]);
  });
});

describe("Forge interaction presentation", () => {
  it("preserves a question that forbids freeform input", () => {
    const presented = presentInteraction({
      interactionId: "question-1",
      kind: "question",
      questions: [
        {
          prompt: "Choose exactly one",
          options: [{ label: "A" }, { label: "B" }],
          allowFreeform: false,
        },
      ],
    });
    expect(presented?.userInput?.questions[0]?.allowCustomAnswer).toBe(false);
  });

  it("preserves every permission option and maps it back by exact opaque id", () => {
    const interaction = {
      interactionId: "permission-1",
      kind: "permission" as const,
      title: "Run command?",
      description: "npm test",
      allowFollowup: true,
      options: [
        { id: "opaque-allow-once", label: "Allow once", description: "Only this command" },
        { id: "opaque-reject-always", label: "Never allow", description: "Remember this denial" },
        { id: "provider-custom", label: "Use safe mode", description: "Provider-specific option" },
      ],
    };
    const presented = presentInteraction(interaction);
    expect(presented?.approval?.remoteActions).toEqual([
      expect.objectContaining({
        id: "permission-option:0",
        label: "Allow once",
        description: "Only this command",
      }),
      expect.objectContaining({
        id: "permission-option:1",
        label: "Never allow",
        description: "Remember this denial",
      }),
      expect.objectContaining({
        id: "permission-option:2",
        label: "Use safe mode",
        description: "Provider-specific option",
      }),
      expect.objectContaining({ id: "permission-cancel", label: "Cancel request" }),
    ]);
    expect(remoteApprovalResponse(interaction, "permission-option:2")).toEqual({
      kind: "permission",
      optionId: "provider-custom",
    });
    expect(remoteApprovalResponse(interaction, "permission-option:99")).toBeNull();
    expect(remoteApprovalResponse(interaction, "permission-followup", "  use read-only  ")).toEqual(
      {
        kind: "permissionFollowup",
        text: "use read-only",
      },
    );
  });

  it("maps plan approval, cancellation, and requested-change feedback exactly", () => {
    const interaction = {
      interactionId: "plan-1",
      kind: "plan" as const,
      plan: "1. Test\n2. Ship",
      allowFeedback: true,
    };
    const presented = presentInteraction(interaction);
    expect(presented?.approval?.remoteFeedback).toEqual({
      actionId: "plan-feedback",
      actionLabel: "Request changes",
      placeholder: "Changes for Forge",
    });
    expect(remoteApprovalResponse(interaction, "plan-approve")).toEqual({
      kind: "plan",
      outcome: "approved",
    });
    expect(remoteApprovalResponse(interaction, "plan-cancel")).toEqual({
      kind: "plan",
      outcome: "cancelled",
    });
    expect(remoteApprovalResponse(interaction, "plan-feedback", "  test first  ")).toEqual({
      kind: "plan",
      outcome: "cancelled",
      feedback: "test first",
    });
  });
});

describe("Forge model presentation", () => {
  it("uses current effort only for the current model and target-specific options for switches", () => {
    const snapshot: RemoteSessionSnapshot = {
      sessionId: "session-1",
      status: "idle",
      transcript: [],
      currentModel: { id: "current", label: "Current" },
      availableModels: [
        { id: "current", label: "Current" },
        {
          id: "target",
          label: "Target",
          reasoningEffort: {
            options: [
              { id: "low", label: "Low" },
              { id: "high", label: "High" },
            ],
          },
        },
        { id: "unknown-target", label: "Unknown target" },
      ],
      reasoningEffort: {
        current: "max",
        options: [
          { id: "medium", label: "Medium" },
          { id: "max", label: "Max" },
        ],
      },
      activeInteractions: [],
      capabilities: {
        prompt: true,
        cancel: true,
        setModel: true,
        fastMode: false,
        reasoning: true,
        btw: true,
        usage: true,
        resolveInteractions: true,
      },
    };
    const models = buildServerConfig(snapshot)?.providers[0]?.models ?? [];
    expect(models[0]?.capabilities?.optionDescriptors?.[0]).toMatchObject({
      currentValue: "max",
      options: [
        { id: "medium", label: "Medium" },
        { id: "max", label: "Max" },
      ],
    });
    expect(models[1]?.capabilities?.optionDescriptors?.[0]).toMatchObject({
      options: [
        { id: "low", label: "Low" },
        { id: "high", label: "High" },
      ],
    });
    expect(models[1]?.capabilities?.optionDescriptors?.[0]).not.toHaveProperty("currentValue");
    expect(models[2]?.capabilities).toBeNull();
    const target = buildModelOptions(buildServerConfig(snapshot), null).find(
      (model) => model.selection.model === "target",
    );
    expect(target).toBeDefined();
    expect(reasoningFromSelection(target!.selection)).toBeNull();
  });
});

describe("Forge local session organization presentation", () => {
  it("uses a local alias ahead of the live title and projects local pin state", () => {
    const presented = presentRemoteSession({
      pairing: {
        id: "pairing-1",
        host: "mac.ts.net",
        addedAt: "2026-08-13T10:00:00.000Z",
        metadata: {
          customTitle: "My release session",
          title: "Cached remote title",
          pinnedAt: "2026-08-13T11:00:00.000Z",
          pinOrderKey: "2026-08-13T11:00:00.000Z",
        },
      },
      connectionPhase: "connected",
      connectionError: null,
      snapshot: {
        sessionId: "session-1",
        title: "Live remote title",
        status: "idle",
        transcript: [
          { id: "user", kind: "user", text: "Question" },
          { id: "reasoning", kind: "reasoning", text: "Thinking" },
          { id: "system", kind: "system", text: "Switched model" },
          { id: "assistant", kind: "assistant", text: "Answer" },
        ],
        currentModel: null,
        availableModels: [],
        activeInteractions: [],
        capabilities: {
          prompt: true,
          cancel: true,
          setModel: false,
          fastMode: false,
          reasoning: false,
          btw: false,
          usage: false,
          resolveInteractions: false,
        },
      },
      revision: 1,
      expiresAt: null,
      pendingInteractionIds: [],
      modelCommandPending: false,
      usageCommandPending: false,
    });

    expect(presented.thread.title).toBe("My release session");
    expect(presented.thread.pinnedAt).toBe("2026-08-13T11:00:00.000Z");
    expect(presented.thread.pinOrderKey).toBe("2026-08-13T11:00:00.000Z");
    expect([...presented.assistantResponseMessageIds]).toEqual(["remote:assistant"]);
    expect(presented.workDisclosures).toEqual([]);
  });
});

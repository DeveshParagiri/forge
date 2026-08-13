import { describe, expect, it } from "vitest";

import type { RemoteSessionSnapshot } from "./protocol/protocol";
import { buildModelOptions } from "../lib/modelOptions";
import {
  buildServerConfig,
  presentInteraction,
  reasoningFromSelection,
  remoteApprovalResponse,
} from "./presentationAdapter";

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

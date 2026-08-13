import type { ModelSelection, ProviderOptionDescriptor } from "@t3tools/contracts";
import { describe, expect, it } from "vitest";

import { buildThreadSettingsMenu } from "../features/threads/thread-settings-menu";
import type { ProviderGroup } from "../lib/modelOptions";

const selected: ModelSelection = { instanceId: "forge", model: "gpt-5.6-sol" };
const providerGroups: ReadonlyArray<ProviderGroup> = [
  {
    providerKey: "forge",
    providerLabel: "Forge",
    models: [
      {
        key: "forge:gpt-5.6-sol",
        label: "GPT-5.6 Sol",
        subtitle: "Forge",
        providerKey: "forge",
        providerLabel: "Forge",
        providerDriver: "forge",
        isDefault: true,
        isLegacy: false,
        capabilities: null,
        selection: selected,
      },
    ],
  },
];
const reasoning: ReadonlyArray<ProviderOptionDescriptor> = [
  {
    id: "reasoningEffort",
    label: "Reasoning",
    type: "select",
    currentValue: "high",
    options: [
      { id: "low", label: "Low" },
      { id: "high", label: "High" },
    ],
  },
];

describe("Forge retained T3 settings integration", () => {
  it("keeps Usage reachable while only model controls are disabled", () => {
    const menu = buildThreadSettingsMenu({
      providerGroups,
      selectedModel: selected,
      optionDescriptors: reasoning,
      runtimeMode: "approval-required",
      includeRuntime: false,
      includeUsage: true,
      modelSelectionDisabled: true,
    });
    expect(menu.actions.map((action) => action.title)).toEqual(["Model", "Reasoning", "Usage"]);
    expect(menu.actions[0]?.subactions?.[0]?.attributes).toEqual({ disabled: true });
    expect(menu.actions[1]?.subactions?.[0]?.attributes).toEqual({ disabled: true });
    expect(menu.actions[2]?.attributes?.disabled).not.toBe(true);
    expect(menu.events.get("usage")).toEqual({ type: "open-usage" });
  });
});

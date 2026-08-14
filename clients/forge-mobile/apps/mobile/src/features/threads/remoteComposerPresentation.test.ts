import { describe, expect, it } from "vite-plus/test";

import {
  compactRemoteModelLabel,
  remoteComposerAttachmentActions,
  remoteComposerPresentation,
} from "./remoteComposerPresentation";

describe("remoteComposerAttachmentActions", () => {
  it("keeps Photos and Files visually distinct with an explicit dark-mode-safe tint", () => {
    expect(remoteComposerAttachmentActions("#f5f5f5")).toEqual([
      { id: "photos", title: "Photos", image: "photo", imageColor: "#f5f5f5" },
      { id: "files", title: "Files", image: "doc", imageColor: "#f5f5f5" },
    ]);
  });
});

describe("compactRemoteModelLabel", () => {
  it("shows the Forge model name without provider or GPT chrome", () => {
    expect(compactRemoteModelLabel("OpenAI · GPT-5.6 Sol")).toBe("5.6 Sol");
    expect(compactRemoteModelLabel("OpenRouter · Gemini 3.5 Flash")).toBe("Gemini 3.5 Flash");
    expect(compactRemoteModelLabel("SpaceX · Grok 4.6")).toBe("Grok 4.6");
  });

  it("turns a protocol model slug into a human display name", () => {
    expect(compactRemoteModelLabel("gpt-5.6-sol")).toBe("5.6 Sol");
  });

  it("does not erase a model family that is part of the model name", () => {
    expect(compactRemoteModelLabel("Grok 4.6")).toBe("Grok 4.6");
    expect(compactRemoteModelLabel("Claude Opus 4.6")).toBe("Claude Opus 4.6");
  });

  it("shows only the compact model and the selected reasoning label", () => {
    expect(
      remoteComposerPresentation("OpenAI · GPT-5.6 Sol", [
        {
          id: "reasoningEffort",
          label: "Reasoning",
          type: "select",
          currentValue: "xhigh",
          options: [{ id: "xhigh", label: "Extra High" }],
        },
        {
          id: "fastMode",
          label: "Fast",
          type: "boolean",
          currentValue: true,
        },
      ]),
    ).toEqual({
      accessibilityLabel: "5.6 Sol · Ultra",
      model: "5.6 Sol",
      reasoning: "Ultra",
    });
    expect(remoteComposerPresentation("OpenAI · GPT-5.6 Sol", [], "138K / 500K")).toEqual({
      accessibilityLabel: "5.6 Sol · 138K / 500K",
      model: "5.6 Sol",
      usage: "138K / 500K",
    });
  });
});

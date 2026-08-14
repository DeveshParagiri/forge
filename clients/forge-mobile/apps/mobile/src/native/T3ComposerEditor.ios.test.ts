import { readFileSync } from "node:fs";

import { describe, expect, it } from "vite-plus/test";

const source = readFileSync(
  new URL("../../modules/t3-composer-editor/ios/T3ComposerEditorView.swift", import.meta.url),
  "utf8",
);

describe("T3ComposerEditor iOS multiline sizing contract", () => {
  it("lays out soft-wrapped text independently of the current editor height", () => {
    expect(source).toContain("textView.textContainer.widthTracksTextView = true");
    expect(source).toContain("textView.textContainer.heightTracksTextView = false");
    expect(source).toContain("textView.textContainer.lineBreakMode = .byWordWrapping");
    expect(source).toContain("textView.layoutManager.ensureLayout(for: textView.textContainer)");
    expect(source).toContain("textView.layoutManager.usedRect(for: textView.textContainer)");
    expect(source).toContain("textView.sizeThatFits(");
  });

  it("re-measures after UIKit commits deferred wrapping", () => {
    const emitTextChange = source.slice(
      source.indexOf("private func emitTextChange()"),
      source.indexOf("private func emitSelection()"),
    );
    expect(emitTextChange).toContain("emitContentSizeIfNeeded()");
    expect(emitTextChange).toContain("scheduleContentSizeEmission()");
  });
});

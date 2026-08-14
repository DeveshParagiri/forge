import { readFileSync } from "node:fs";

import { describe, expect, it } from "vite-plus/test";

const source = readFileSync(
  new URL(
    "../../modules/t3-composer-editor/android/src/main/java/expo/modules/t3composereditor/T3ComposerEditorView.kt",
    import.meta.url,
  ),
  "utf8",
);

describe("T3ComposerEditor Android multiline sizing contract", () => {
  it("uses TextView's natural text layout height instead of the fixed view height", () => {
    expect(source).toContain("val textHeight = editor.layout?.height ?: editor.measuredHeight");
  });

  it("re-measures after Android commits deferred soft wrapping", () => {
    const watcher = source.slice(
      source.indexOf("override fun afterTextChanged"),
      source.indexOf("editor.addOnLayoutChangeListener"),
    );
    expect(watcher).toContain("emitContentSizeIfNeeded()");
    expect(watcher).toContain("scheduleContentSizeEmission()");
    expect(source).toContain("editor.postOnAnimation {");
    expect(source).toContain("editor.post {");
  });
});

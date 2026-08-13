import { describe, expect, it } from "vite-plus/test";

import {
  REMOTE_COMPOSER_EDITOR_MAX_HEIGHT,
  REMOTE_COMPOSER_EDITOR_MIN_HEIGHT,
  resolveRemoteComposerEditorHeight,
} from "./remoteComposerLayout";

describe("resolveRemoteComposerEditorHeight", () => {
  it("keeps an empty or one-line remote draft compact", () => {
    expect(resolveRemoteComposerEditorHeight(0)).toBe(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT);
    expect(resolveRemoteComposerEditorHeight(23.2)).toBe(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT);
  });

  it("grows with multiline content and caps at the scroll threshold", () => {
    expect(resolveRemoteComposerEditorHeight(61.2)).toBe(62);
    expect(resolveRemoteComposerEditorHeight(400)).toBe(REMOTE_COMPOSER_EDITOR_MAX_HEIGHT);
  });

  it("rejects unusable native measurements", () => {
    expect(resolveRemoteComposerEditorHeight(Number.NaN)).toBe(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT);
  });
});

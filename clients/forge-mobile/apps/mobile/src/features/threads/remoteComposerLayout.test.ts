import { describe, expect, it } from "vite-plus/test";

import {
  REMOTE_COMPOSER_EDITOR_MAX_HEIGHT,
  REMOTE_COMPOSER_EDITOR_MIN_HEIGHT,
  REMOTE_COMPOSER_COLLAPSED_EDITOR_HEIGHT,
  REMOTE_COMPOSER_COLLAPSED_CHROME,
  REMOTE_COMPOSER_COLLAPSED_HORIZONTAL_INSET,
  REMOTE_COMPOSER_FOCUSED_HORIZONTAL_INSET,
  REMOTE_COMPOSER_EXPANDED_CHROME,
  REMOTE_COMPOSER_FOCUSED_CONTENT_INSET_HORIZONTAL,
  REMOTE_COMPOSER_FOCUSED_TEXT_INSET_HORIZONTAL,
  REMOTE_COMPOSER_SURFACE_PADDING_HORIZONTAL,
  resolveComposerEditorContentInsetHorizontal,
  resolveRemoteComposerBottomInset,
  resolveRemoteComposerEditorHeight,
  resolveRemoteComposerVisualChrome,
  resolveThreadComposerChrome,
  shouldTrackRemoteComposerKeyboard,
} from "./remoteComposerLayout";

describe("resolveRemoteComposerEditorHeight", () => {
  it("keeps an empty or one-line remote draft compact", () => {
    expect(resolveRemoteComposerEditorHeight(0)).toBe(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT);
    expect(resolveRemoteComposerEditorHeight(23.2)).toBe(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT);
  });

  it("grows through several visible lines before capping the native editor", () => {
    expect(resolveRemoteComposerEditorHeight(31.2)).toBe(32);
    expect(resolveRemoteComposerEditorHeight(61.2)).toBe(62);
    expect(resolveRemoteComposerEditorHeight(109.2)).toBe(110);
    expect(resolveRemoteComposerEditorHeight(400)).toBe(REMOTE_COMPOSER_EDITOR_MAX_HEIGHT);
  });

  it("rejects unusable native measurements", () => {
    expect(resolveRemoteComposerEditorHeight(Number.NaN)).toBe(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT);
  });
});

describe("remote composer keyboard geometry", () => {
  it("adds the missing native text inset only to the focused Forge editor", () => {
    expect(REMOTE_COMPOSER_FOCUSED_CONTENT_INSET_HORIZONTAL).toBe(10);
    expect(
      REMOTE_COMPOSER_SURFACE_PADDING_HORIZONTAL + REMOTE_COMPOSER_FOCUSED_CONTENT_INSET_HORIZONTAL,
    ).toBe(REMOTE_COMPOSER_FOCUSED_TEXT_INSET_HORIZONTAL);
    expect(REMOTE_COMPOSER_FOCUSED_TEXT_INSET_HORIZONTAL).toBe(16);
    expect(resolveComposerEditorContentInsetHorizontal({ remoteOnly: true, expanded: true })).toBe(
      10,
    );
    expect(resolveComposerEditorContentInsetHorizontal({ remoteOnly: true, expanded: false })).toBe(
      0,
    );
    expect(resolveComposerEditorContentInsetHorizontal({ remoteOnly: false, expanded: true })).toBe(
      0,
    );
  });

  it("uses ChatGPT-style side insets: shorter idle pill, wider focused editor", () => {
    expect(REMOTE_COMPOSER_COLLAPSED_HORIZONTAL_INSET).toBe(32);
    expect(REMOTE_COMPOSER_FOCUSED_HORIZONTAL_INSET).toBe(13);
  });

  it("uses a 66pt closed pill and a 184pt focused reservation without changing retained T3", () => {
    expect(REMOTE_COMPOSER_COLLAPSED_EDITOR_HEIGHT).toBe(32);
    expect(REMOTE_COMPOSER_COLLAPSED_CHROME).toBe(66);
    expect(REMOTE_COMPOSER_EXPANDED_CHROME).toBe(184);
    expect(
      resolveThreadComposerChrome({
        remoteOnly: true,
        expanded: false,
        retainedCollapsedChrome: 60,
        retainedExpandedChrome: 174,
      }),
    ).toBe(REMOTE_COMPOSER_COLLAPSED_CHROME);
    expect(
      resolveThreadComposerChrome({
        remoteOnly: true,
        expanded: true,
        retainedCollapsedChrome: 60,
        retainedExpandedChrome: 174,
      }),
    ).toBe(REMOTE_COMPOSER_EXPANDED_CHROME);
    expect(
      resolveThreadComposerChrome({
        remoteOnly: false,
        expanded: true,
        retainedCollapsedChrome: 60,
        retainedExpandedChrome: 174,
      }),
    ).toBe(174);
  });

  it("measures the focused two-level composer from 104pt to its multiline cap", () => {
    expect(resolveRemoteComposerVisualChrome({ expanded: true, editorHeight: 28 })).toBe(104);
    expect(resolveRemoteComposerVisualChrome({ expanded: true, editorHeight: 80 })).toBe(152);
    expect(resolveRemoteComposerVisualChrome({ expanded: true, editorHeight: 400 })).toBe(184);
    expect(resolveRemoteComposerVisualChrome({ expanded: false, editorHeight: 400 })).toBe(66);
  });

  it("keeps the sticky keyboard track active from iOS focus before visibility catches up", () => {
    expect(
      shouldTrackRemoteComposerKeyboard({
        platform: "ios",
        keyboardVisible: false,
        composerFocused: true,
        keyboardStateSuspect: false,
      }),
    ).toBe(true);
  });

  it("preserves Android visibility ownership and quarantines suspect keyboard state", () => {
    expect(
      shouldTrackRemoteComposerKeyboard({
        platform: "android",
        keyboardVisible: false,
        composerFocused: true,
        keyboardStateSuspect: false,
      }),
    ).toBe(false);
    expect(
      shouldTrackRemoteComposerKeyboard({
        platform: "ios",
        keyboardVisible: true,
        composerFocused: true,
        keyboardStateSuspect: true,
      }),
    ).toBe(false);
  });

  it("removes home-indicator padding while the iOS editor owns the keyboard", () => {
    expect(
      resolveRemoteComposerBottomInset({
        platform: "ios",
        keyboardVisible: false,
        composerFocused: true,
        safeAreaBottom: 34,
      }),
    ).toBe(0);
    expect(
      resolveRemoteComposerBottomInset({
        platform: "ios",
        keyboardVisible: false,
        composerFocused: false,
        safeAreaBottom: 34,
      }),
    ).toBe(34);
  });
});

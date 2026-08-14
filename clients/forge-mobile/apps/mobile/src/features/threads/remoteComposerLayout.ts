export const REMOTE_COMPOSER_EDITOR_MIN_HEIGHT = 28;
export const REMOTE_COMPOSER_COLLAPSED_EDITOR_HEIGHT = 32;
/** Four comfortable body-text lines before the native editor starts scrolling. */
export const REMOTE_COMPOSER_EDITOR_MAX_HEIGHT = 112;
export const REMOTE_COMPOSER_FOCUSED_EDITOR_MIN_HEIGHT = 32;
export const REMOTE_COMPOSER_CONTROL_ROW_HEIGHT = 44;
export const REMOTE_COMPOSER_SURFACE_PADDING_HORIZONTAL = 6;
export const REMOTE_COMPOSER_COLLAPSED_HORIZONTAL_INSET = 32;
export const REMOTE_COMPOSER_FOCUSED_HORIZONTAL_INSET = 13;
export const REMOTE_COMPOSER_FOCUSED_TEXT_INSET_HORIZONTAL = 16;
/** Native inset after subtracting the card's own horizontal padding. */
export const REMOTE_COMPOSER_FOCUSED_CONTENT_INSET_HORIZONTAL =
  REMOTE_COMPOSER_FOCUSED_TEXT_INSET_HORIZONTAL - REMOTE_COMPOSER_SURFACE_PADDING_HORIZONTAL;

/** Compact one-line pill, including its six-point outer top and bottom gaps. */
export const REMOTE_COMPOSER_COLLAPSED_CHROME = 66;

/**
 * Maximum focused Forge composer reservation, excluding the device safe-area
 * inset: 2 outer top + 8 top + 112 editor + 4 gap + 44 controls + 4 bottom +
 * 10 outer bottom. The ten-point keyboard gap is intentional on older iOS.
 */
export const REMOTE_COMPOSER_EXPANDED_CHROME = 184;

export function resolveThreadComposerChrome(input: {
  readonly remoteOnly: boolean;
  readonly expanded: boolean;
  readonly retainedCollapsedChrome: number;
  readonly retainedExpandedChrome: number;
}): number {
  if (input.remoteOnly) {
    return input.expanded ? REMOTE_COMPOSER_EXPANDED_CHROME : REMOTE_COMPOSER_COLLAPSED_CHROME;
  }
  return input.expanded ? input.retainedExpandedChrome : input.retainedCollapsedChrome;
}

/** Actual focused surface height for the current editor content, outer gaps included. */
export function resolveRemoteComposerVisualChrome(input: {
  readonly expanded: boolean;
  readonly editorHeight: number;
}): number {
  if (!input.expanded) return REMOTE_COMPOSER_COLLAPSED_CHROME;
  const editorHeight = Math.max(
    REMOTE_COMPOSER_FOCUSED_EDITOR_MIN_HEIGHT,
    resolveRemoteComposerEditorHeight(input.editorHeight),
  );
  return 2 + 8 + editorHeight + 4 + REMOTE_COMPOSER_CONTROL_ROW_HEIGHT + 4 + 10;
}

export function resolveRemoteComposerEditorHeight(contentHeight: number): number {
  if (!Number.isFinite(contentHeight)) return REMOTE_COMPOSER_EDITOR_MIN_HEIGHT;
  return Math.min(
    REMOTE_COMPOSER_EDITOR_MAX_HEIGHT,
    Math.max(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT, Math.ceil(contentHeight)),
  );
}

export function resolveComposerEditorContentInsetHorizontal(input: {
  readonly remoteOnly: boolean;
  readonly expanded: boolean;
}): number {
  return input.remoteOnly && input.expanded ? REMOTE_COMPOSER_FOCUSED_CONTENT_INSET_HORIZONTAL : 0;
}

/**
 * Older iOS releases can lag the keyboard controller's visibility bit behind
 * focus. The native editor is authoritative while it owns focus; Android
 * retains the upstream visibility-only policy because stale IME state after
 * backgrounding is handled separately by the host.
 */
export function shouldTrackRemoteComposerKeyboard(input: {
  readonly platform: "ios" | "android";
  readonly keyboardVisible: boolean;
  readonly composerFocused: boolean;
  readonly keyboardStateSuspect: boolean;
}): boolean {
  if (input.keyboardStateSuspect) return false;
  return input.keyboardVisible || (input.platform === "ios" && input.composerFocused);
}

export function resolveRemoteComposerBottomInset(input: {
  readonly platform: "ios" | "android";
  readonly keyboardVisible: boolean;
  readonly composerFocused: boolean;
  readonly safeAreaBottom: number;
}): number {
  const keyboardOwnsBottomEdge =
    input.platform === "android" ? input.keyboardVisible : input.composerFocused;
  return keyboardOwnsBottomEdge ? 0 : Math.max(input.safeAreaBottom, 12);
}

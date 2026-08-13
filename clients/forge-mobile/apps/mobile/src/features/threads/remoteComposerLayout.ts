export const REMOTE_COMPOSER_EDITOR_MIN_HEIGHT = 28;
export const REMOTE_COMPOSER_EDITOR_MAX_HEIGHT = 96;

/** One-line remote composer chrome, excluding the device safe-area inset. */
export const REMOTE_COMPOSER_EXPANDED_CHROME = 112;

export function resolveRemoteComposerEditorHeight(contentHeight: number): number {
  if (!Number.isFinite(contentHeight)) return REMOTE_COMPOSER_EDITOR_MIN_HEIGHT;
  return Math.min(
    REMOTE_COMPOSER_EDITOR_MAX_HEIGHT,
    Math.max(REMOTE_COMPOSER_EDITOR_MIN_HEIGHT, Math.ceil(contentHeight)),
  );
}

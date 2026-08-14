/*
 * Adapted from T3 Code's apps/web/src/components/ComposerPromptEditor.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import { forwardRef, useCallback, useImperativeHandle, useLayoutEffect, useRef } from "react";

const EDITOR_MIN_HEIGHT = 32;
const EDITOR_MAX_HEIGHT = 112;

export interface ComposerPromptEditorHandle {
  focus(): void;
  focusAtEnd(): void;
}

export const ComposerPromptEditor = forwardRef<
  ComposerPromptEditorHandle,
  {
    value: string;
    onChange(value: string): void;
    onSubmit(): void;
    disabled: boolean;
    placeholder: string;
    label: string;
  }
>(function ComposerPromptEditor({ value, onChange, onSubmit, disabled, placeholder, label }, ref) {
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const resize = useCallback(() => {
    const input = inputRef.current;
    if (!input) return;
    input.style.height = `${EDITOR_MIN_HEIGHT}px`;
    const height = Math.min(EDITOR_MAX_HEIGHT, Math.max(EDITOR_MIN_HEIGHT, input.scrollHeight));
    input.style.height = `${height}px`;
    input.style.overflowY = input.scrollHeight > EDITOR_MAX_HEIGHT ? "auto" : "hidden";
  }, []);

  useLayoutEffect(resize, [resize, value]);

  useImperativeHandle(ref, () => ({
    focus: () => inputRef.current?.focus(),
    focusAtEnd: () => {
      const input = inputRef.current;
      if (!input) return;
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    },
  }));
  return (
    <textarea
      ref={inputRef}
      className="composer-editor"
      aria-label={label}
      value={value}
      rows={1}
      disabled={disabled}
      placeholder={placeholder}
      enterKeyHint="enter"
      autoCapitalize="sentences"
      autoComplete="off"
      spellCheck="true"
      onChange={(event) => {
        onChange(event.target.value);
        window.requestAnimationFrame(resize);
      }}
      onFocus={(event) => {
        const input = event.currentTarget;
        window.setTimeout(() => input.scrollIntoView({ block: "nearest" }), 80);
      }}
      onKeyDown={(event) => {
        const isDesktopSubmit = event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing;
        if (!isDesktopSubmit || window.matchMedia("(pointer: coarse)").matches) return;
        event.preventDefault();
        onSubmit();
      }}
    />
  );
});

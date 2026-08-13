/*
 * Adapted from T3 Code's apps/web/src/components/ComposerPromptEditor.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import { forwardRef, useImperativeHandle, useRef } from "react";

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
  useImperativeHandle(ref, () => ({
    focus: () => inputRef.current?.focus(),
    focusAtEnd: () => {
      const input = inputRef.current;
      if (!input) return;
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    },
  }));
  const rows = Math.min(6, Math.max(1, value.split("\n").length));
  return (
    <textarea
      ref={inputRef}
      className="composer-editor"
      aria-label={label}
      value={value}
      rows={rows}
      disabled={disabled}
      placeholder={placeholder}
      enterKeyHint="enter"
      autoCapitalize="sentences"
      autoComplete="off"
      spellCheck="true"
      onChange={(event) => onChange(event.target.value)}
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

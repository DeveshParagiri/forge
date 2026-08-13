export type ForgeComposerCommand =
  | { readonly type: "prompt"; readonly text: string }
  | { readonly type: "btw"; readonly question: string }
  | { readonly type: "usage" }
  | { readonly type: "invalidBtw" };

/** Mirrors the TUI slash boundary: `/btw` must be followed by whitespace. */
export function parseForgeComposerCommand(input: string): ForgeComposerCommand | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  if (trimmed === "/usage") return { type: "usage" };
  if (/^\/btw$/.test(trimmed)) return { type: "invalidBtw" };
  const match = trimmed.match(/^\/btw\s+(.+)$/s);
  if (match) {
    const question = match[1]?.trim() ?? "";
    return question ? { type: "btw", question } : { type: "invalidBtw" };
  }
  return { type: "prompt", text: trimmed };
}

export function isValidBtwCommand(input: string): boolean {
  return parseForgeComposerCommand(input)?.type === "btw";
}

export function forgeComposerAction(
  input: string,
  runningAndCancellable: boolean,
): "send" | "stop" {
  const command = parseForgeComposerCommand(input);
  if (command?.type === "btw" || command?.type === "usage") return "send";
  return runningAndCancellable ? "stop" : "send";
}

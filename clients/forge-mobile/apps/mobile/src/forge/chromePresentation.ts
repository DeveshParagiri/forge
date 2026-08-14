export const FORGE_HEADER_BACKGROUND = "#000000";

export interface ForgeHomeHeaderPresentation {
  readonly addSessionAccessibilityLabel: "Add Forge session";
  readonly addSessionLabel: "+";
  readonly backgroundColor: typeof FORGE_HEADER_BACKGROUND;
  readonly showConnectionDot: false;
}

export function forgeHomeHeaderPresentation(): ForgeHomeHeaderPresentation {
  return {
    addSessionAccessibilityLabel: "Add Forge session",
    addSessionLabel: "+",
    backgroundColor: FORGE_HEADER_BACKGROUND,
    showConnectionDot: false,
  };
}

export function forgeHeaderPairTextColor(
  scheme: "light" | "dark" | "unspecified" | null | undefined,
): string {
  return scheme === "dark" ? "#FFFFFF" : "#111111";
}

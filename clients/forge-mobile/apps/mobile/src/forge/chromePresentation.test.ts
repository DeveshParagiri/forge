import { describe, expect, it } from "vitest";

import { forgeHeaderPairTextColor } from "./chromePresentation";

describe("Forge native header chrome", () => {
  it("uses a visible Pair token in both system schemes", () => {
    expect(forgeHeaderPairTextColor("dark")).toBe("#FFFFFF");
    expect(forgeHeaderPairTextColor("light")).toBe("#111111");
    expect(forgeHeaderPairTextColor(null)).toBe("#111111");
  });
});

import { describe, expect, it } from "vitest";

import { forgeHomeHeaderPresentation } from "./chromePresentation";

describe("Forge native header chrome", () => {
  it("matches the logo black, removes the Home status dot, and labels add-session as plus", () => {
    expect(forgeHomeHeaderPresentation()).toEqual({
      addSessionAccessibilityLabel: "Add Forge session",
      addSessionLabel: "+",
      backgroundColor: "#000000",
      showConnectionDot: false,
    });
  });
});

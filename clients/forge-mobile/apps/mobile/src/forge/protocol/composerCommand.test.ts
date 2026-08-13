import { describe, expect, it } from "vitest";

import { forgeComposerAction, parseForgeComposerCommand } from "./composerCommand";

describe("Forge mobile composer commands", () => {
  it("keeps the trailing action as Send for a valid running BTW question", () => {
    expect(forgeComposerAction(" /btw did the tests finish? ", true)).toBe("send");
    expect(parseForgeComposerCommand(" /btw did the tests finish? ")).toEqual({
      type: "btw",
      question: "did the tests finish?",
    });
  });

  it("uses the same trailing action as Stop for ordinary or empty running drafts", () => {
    expect(forgeComposerAction("ship it", true)).toBe("stop");
    expect(forgeComposerAction("", true)).toBe("stop");
    expect(forgeComposerAction("/btw", true)).toBe("stop");
  });

  it("keeps Send when a running session does not advertise cancellation", () => {
    expect(forgeComposerAction("continue", false)).toBe("send");
    expect(forgeComposerAction("", false)).toBe("send");
  });

  it("intercepts only exact whitespace-tolerant /usage and keeps it usable while running", () => {
    expect(parseForgeComposerCommand("  /usage\n")).toEqual({ type: "usage" });
    expect(forgeComposerAction(" /usage ", true)).toBe("send");
    expect(parseForgeComposerCommand("/usage show")).toEqual({
      type: "prompt",
      text: "/usage show",
    });
    expect(parseForgeComposerCommand("/Usage")).toEqual({
      type: "prompt",
      text: "/Usage",
    });
  });

  it("treats uppercase BTW as a normal prompt because slash commands are case-sensitive", () => {
    expect(parseForgeComposerCommand("/BTW leave this unchanged")).toEqual({
      type: "prompt",
      text: "/BTW leave this unchanged",
    });
  });
});

import { describe, expect, it, vi } from "vitest";

import { isForgeAppForeground, syncSelectedForegroundOwnership } from "./foregroundOwnership";

describe("Forge native foreground ownership", () => {
  it("treats only an active app as the WebSocket owner", () => {
    expect(isForgeAppForeground("active")).toBe(true);
    expect(isForgeAppForeground("inactive")).toBe(false);
    expect(isForgeAppForeground("background")).toBe(false);
    expect(isForgeAppForeground("unknown")).toBe(false);
    expect(isForgeAppForeground("extension")).toBe(false);
  });

  it("claims only the selected session and releases every other saved pairing", () => {
    const first = { resume: vi.fn(), suspend: vi.fn() };
    const second = { resume: vi.fn(), suspend: vi.fn() };
    const sockets = [
      ["first", first],
      ["second", second],
    ] as const;

    syncSelectedForegroundOwnership(sockets, "second", "active");
    expect(first.suspend).toHaveBeenCalledOnce();
    expect(first.resume).not.toHaveBeenCalled();
    expect(second.resume).toHaveBeenCalledOnce();
    expect(second.suspend).not.toHaveBeenCalled();

    syncSelectedForegroundOwnership(sockets, null, "active");
    expect(first.suspend).toHaveBeenCalledTimes(2);
    expect(second.suspend).toHaveBeenCalledOnce();
  });

  it("keeps the selected session released while the app is offscreen", () => {
    const selected = { resume: vi.fn(), suspend: vi.fn() };
    syncSelectedForegroundOwnership([["selected", selected]], "selected", "background");
    expect(selected.suspend).toHaveBeenCalledOnce();
    expect(selected.resume).not.toHaveBeenCalled();
  });
});

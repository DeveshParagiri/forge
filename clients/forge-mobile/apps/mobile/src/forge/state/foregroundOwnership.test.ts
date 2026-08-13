import { describe, expect, it, vi } from "vitest";

import { isForgeAppForeground, syncForegroundOwnership } from "./foregroundOwnership";

describe("Forge native foreground ownership", () => {
  it("treats only an active app as the WebSocket owner", () => {
    expect(isForgeAppForeground("active")).toBe(true);
    expect(isForgeAppForeground("inactive")).toBe(false);
    expect(isForgeAppForeground("background")).toBe(false);
    expect(isForgeAppForeground("unknown")).toBe(false);
    expect(isForgeAppForeground("extension")).toBe(false);
  });

  it("releases every pairing offscreen and claims each one in the foreground", () => {
    const sockets = [
      { resume: vi.fn(), suspend: vi.fn() },
      { resume: vi.fn(), suspend: vi.fn() },
    ];

    syncForegroundOwnership(sockets, "inactive");
    syncForegroundOwnership(sockets, "background");
    for (const socket of sockets) {
      expect(socket.suspend).toHaveBeenCalledTimes(2);
      expect(socket.resume).not.toHaveBeenCalled();
    }

    syncForegroundOwnership(sockets, "active");
    for (const socket of sockets) expect(socket.resume).toHaveBeenCalledOnce();
  });
});

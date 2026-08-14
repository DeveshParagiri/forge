import { describe, expect, it } from "vitest";

import {
  FORGE_SESSION_HEADER_STATUS_GEOMETRY,
  forgeConnectionStatusDot,
} from "./connectionStatusDot";
import type { RemoteConnectionPhase } from "./protocol/remoteSocket";

describe("Forge connection status dot", () => {
  it("stays compact beside the title", () => {
    expect(FORGE_SESSION_HEADER_STATUS_GEOMETRY).toEqual({
      diameter: 8,
      radius: 4,
      titleGap: 6,
    });
  });

  it("is green only when connected and red otherwise, with no status wording", () => {
    const expected: Record<RemoteConnectionPhase, "#34C759" | "#FF3B30"> = {
      connected: "#34C759",
      connecting: "#FF3B30",
      reconnecting: "#FF3B30",
      error: "#FF3B30",
    };

    for (const [phase, color] of Object.entries(expected)) {
      expect(forgeConnectionStatusDot(phase as RemoteConnectionPhase)).toEqual({
        color,
        message: null,
      });
    }
  });
});

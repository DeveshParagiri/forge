import { describe, expect, it, vi } from "vitest";

import { openForgeHomeThread } from "./homeThreadNavigation";

describe("Forge Home thread selection", () => {
  it("reclaims native ownership before opening the saved thread", () => {
    const events: string[] = [];
    const reconnect = vi.fn((pairingId: string) => events.push(`reconnect:${pairingId}`));
    const navigate = vi.fn((pairingId: string) => events.push(`navigate:${pairingId}`));

    openForgeHomeThread("pairing-1", reconnect, navigate);

    expect(events).toEqual(["reconnect:pairing-1", "navigate:pairing-1"]);
    expect(reconnect).toHaveBeenCalledOnce();
    expect(navigate).toHaveBeenCalledOnce();
  });
});

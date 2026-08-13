import { describe, expect, it, vi } from "vitest";

import { drainQueuedPairing, openOrQueuePairing } from "./pendingNavigation";

describe("Forge cold-start pairing navigation", () => {
  it("queues a registered initial URL until NavigationContainer becomes ready", () => {
    const pending = { pairingId: null as string | null };
    const navigate = vi.fn();
    openOrQueuePairing(pending, "pairing-cold-start", () => false, navigate);
    expect(navigate).not.toHaveBeenCalled();
    expect(pending.pairingId).toBe("pairing-cold-start");
    drainQueuedPairing(pending, navigate);
    expect(navigate).toHaveBeenCalledWith("pairing-cold-start");
    expect(pending.pairingId).toBeNull();
    drainQueuedPairing(pending, navigate);
    expect(navigate).toHaveBeenCalledTimes(1);
  });

  it("opens live pairings immediately once navigation is ready", () => {
    const pending = { pairingId: "stale" as string | null };
    const navigate = vi.fn();
    openOrQueuePairing(pending, "pairing-live", () => true, navigate);
    expect(navigate).toHaveBeenCalledWith("pairing-live");
    expect(pending.pairingId).toBeNull();
  });
});

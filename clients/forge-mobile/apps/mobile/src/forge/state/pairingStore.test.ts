import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => {
  const values = new Map<string, string>();
  return {
    clear: () => values.clear(),
    get: (key: string) => values.get(key) ?? null,
    getItemAsync: vi.fn((key: string) => Promise.resolve(values.get(key) ?? null)),
    setItemAsync: vi.fn((key: string, value: string) => {
      values.set(key, value);
      return Promise.resolve();
    }),
    deleteItemAsync: vi.fn((key: string) => {
      values.delete(key);
      return Promise.resolve();
    }),
    randomUUID: vi.fn(() => "pairing-1"),
  };
});

vi.mock("expo-secure-store", () => ({
  WHEN_UNLOCKED_THIS_DEVICE_ONLY: "when-unlocked-this-device-only",
  deleteItemAsync: mocks.deleteItemAsync,
  getItemAsync: mocks.getItemAsync,
  setItemAsync: mocks.setItemAsync,
}));

vi.mock("expo-crypto", () => ({
  randomUUID: mocks.randomUUID,
}));

import {
  bindStoredPairingSessionIdentity,
  finalizeStoredPairing,
  loadStoredPairings,
  normalizeSessionAlias,
  reconcileStoredPairingSessionIdentity,
  registerStoredPairing,
  registerStoredPairingWithStatus,
  updateStoredPairingMetadata,
} from "./pairingStore";
import { comparePairingSummaries } from "./sessionOrganization";

const token = "0123456789abcdef".repeat(4);
const pairingUrl = `https://forge-mac.example-tailnet.ts.net/forge/${token}/`;
const refreshedPairingUrl = `https://forge-mac.example-tailnet.ts.net/forge/${"fedcba9876543210".repeat(4)}/`;
const otherHostPairingUrl = `https://other-mac.example-tailnet.ts.net/forge/${"abcdef0123456789".repeat(4)}/`;

describe("Forge pairing organization storage", () => {
  beforeEach(() => {
    mocks.clear();
    vi.clearAllMocks();
  });

  it("keeps local alias, pin, and archive metadata in secure storage", async () => {
    const pairing = await registerStoredPairing(pairingUrl);
    await updateStoredPairingMetadata(pairing.id, {
      customTitle: "Ship Forge",
      pinnedAt: "2026-08-13T12:00:00.000Z",
      pinOrderKey: "2026-08-13T12:00:00.000Z",
      archivedAt: "2026-08-13T13:00:00.000Z",
    });

    expect(await loadStoredPairings()).toEqual([
      expect.objectContaining({
        id: pairing.id,
        gatewayUrl: pairingUrl,
        metadata: expect.objectContaining({
          customTitle: "Ship Forge",
          pinnedAt: "2026-08-13T12:00:00.000Z",
          pinOrderKey: "2026-08-13T12:00:00.000Z",
          archivedAt: "2026-08-13T13:00:00.000Z",
        }),
      }),
    ]);
  });

  it("recovers an archived pairing by re-scanning the exact URL", async () => {
    const pairing = await registerStoredPairing(pairingUrl);
    await updateStoredPairingMetadata(pairing.id, {
      customTitle: "Keep my name",
      archivedAt: "2026-08-13T13:00:00.000Z",
    });

    const restored = await registerStoredPairing(pairingUrl);

    expect(restored.id).toBe(pairing.id);
    expect(restored.metadata.customTitle).toBe("Keep my name");
    expect(restored.metadata.archivedAt).toBeUndefined();
    expect((await loadStoredPairings())[0]?.metadata.archivedAt).toBeUndefined();
  });

  it("reuses the stable session record when /rc issues a new bearer URL", async () => {
    mocks.randomUUID.mockReturnValueOnce("pairing-existing").mockReturnValueOnce("pairing-scanned");
    const existing = await registerStoredPairing(pairingUrl);
    await bindStoredPairingSessionIdentity(existing.id, "session-main");
    await updateStoredPairingMetadata(existing.id, {
      customTitle: "Main Forge Harness Development",
      pinnedAt: "2026-08-13T12:00:00.000Z",
    });

    const scanned = await registerStoredPairingWithStatus(refreshedPairingUrl);
    expect(scanned).toMatchObject({ created: true, record: { id: "pairing-scanned" } });

    const reconciled = await reconcileStoredPairingSessionIdentity(
      scanned.record.id,
      "session-main",
    );

    expect(reconciled).toMatchObject({
      record: {
        id: "pairing-existing",
        gatewayUrl: refreshedPairingUrl,
        sessionId: "session-main",
        metadata: {
          customTitle: "Main Forge Harness Development",
          pinnedAt: "2026-08-13T12:00:00.000Z",
        },
      },
      removedPairingIds: ["pairing-scanned"],
    });
    expect(await loadStoredPairings()).toEqual([reconciled.record]);
  });

  it("reports exact bearer rescans as an existing registration", async () => {
    const first = await registerStoredPairingWithStatus(pairingUrl);
    const second = await registerStoredPairingWithStatus(pairingUrl);

    expect(first.created).toBe(true);
    expect(second).toEqual({ created: false, record: first.record });
  });

  it("does not merge equal session labels from different remote hosts", async () => {
    mocks.randomUUID.mockReturnValueOnce("pairing-first").mockReturnValueOnce("pairing-other");
    const first = await registerStoredPairing(pairingUrl);
    await bindStoredPairingSessionIdentity(first.id, "host-local-session");
    const other = await registerStoredPairing(otherHostPairingUrl);

    const reconciled = await reconcileStoredPairingSessionIdentity(other.id, "host-local-session");

    expect(reconciled.record.id).toBe("pairing-other");
    expect(reconciled.removedPairingIds).toBeUndefined();
    expect(await loadStoredPairings()).toHaveLength(2);
  });

  it("persists an expected child session identity only for a fresh pairing URL", async () => {
    const child = await registerStoredPairing(pairingUrl, "session-child");
    expect(child.sessionId).toBe("session-child");
    expect(child.provisionalUntil).toBeDefined();
    expect((await loadStoredPairings())[0]?.sessionId).toBe("session-child");
    await finalizeStoredPairing(child.id);
    expect((await loadStoredPairings())[0]?.provisionalUntil).toBeUndefined();
    await expect(registerStoredPairing(pairingUrl, "session-other")).rejects.toThrow(
      /fresh child pairing/i,
    );
  });

  it("clears local organization fields without removing unrelated remote cache metadata", async () => {
    const pairing = await registerStoredPairing(pairingUrl);
    await updateStoredPairingMetadata(pairing.id, {
      customTitle: "Temporary alias",
      title: "Remote title",
      pinnedAt: "2026-08-13T12:00:00.000Z",
      pinOrderKey: "2026-08-13T12:00:00.000Z",
    });
    await updateStoredPairingMetadata(pairing.id, {
      customTitle: undefined,
      pinnedAt: undefined,
      pinOrderKey: undefined,
    });

    expect((await loadStoredPairings())[0]?.metadata).toEqual({ title: "Remote title" });
  });

  it("serializes overlapping metadata writes so remote cache updates cannot erase local state", async () => {
    const pairing = await registerStoredPairing(pairingUrl);

    await Promise.all([
      updateStoredPairingMetadata(pairing.id, { customTitle: "Pinned name" }),
      updateStoredPairingMetadata(pairing.id, { title: "Remote name" }),
      updateStoredPairingMetadata(pairing.id, { pinnedAt: "2026-08-13T12:00:00.000Z" }),
    ]);

    expect((await loadStoredPairings())[0]?.metadata).toMatchObject({
      customTitle: "Pinned name",
      title: "Remote name",
      pinnedAt: "2026-08-13T12:00:00.000Z",
    });
  });

  it("orders pinned sessions deterministically before newest unpinned sessions", () => {
    const records = [
      { id: "new", host: "new.ts.net", addedAt: "2026-08-13T14:00:00.000Z", metadata: {} },
      {
        id: "pinned-second",
        host: "second.ts.net",
        addedAt: "2026-08-13T11:00:00.000Z",
        metadata: {
          pinnedAt: "2026-08-13T11:00:00.000Z",
          pinOrderKey: "2026-08-13T11:00:00.000Z",
        },
      },
      {
        id: "pinned-first",
        host: "first.ts.net",
        addedAt: "2026-08-13T10:00:00.000Z",
        metadata: {
          pinnedAt: "2026-08-13T10:00:00.000Z",
          pinOrderKey: "2026-08-13T10:00:00.000Z",
        },
      },
      { id: "old", host: "old.ts.net", addedAt: "2026-08-13T09:00:00.000Z", metadata: {} },
    ];

    expect([...records].sort(comparePairingSummaries).map((record) => record.id)).toEqual([
      "pinned-first",
      "pinned-second",
      "new",
      "old",
    ]);
  });

  it("bounds and normalizes user-owned aliases before persistence", () => {
    expect(normalizeSessionAlias("  Release room  ")).toBe("Release room");
    expect(normalizeSessionAlias("   ")).toBeUndefined();
    expect(normalizeSessionAlias("x".repeat(121))).toBeUndefined();
  });
});

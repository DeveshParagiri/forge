import { describe, expect, it } from "vitest";
import {
  pairingStorageKey,
  readPairings,
  registerPairing,
  removePairing,
  stateInvalidatesPairing,
  updatePairingFromState,
  writePairings,
} from "../src/pairingRegistry";
import type { RemoteClientState } from "../src/reducer";
import { sessionFixture } from "./fixtures";

const ORIGIN = "https://forge.tail.example";
const TOKEN_A = "a".repeat(64);
const TOKEN_B = "b".repeat(64);

function storageFixture() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  };
}

describe("same-origin pairing registry", () => {
  it("registers multiple immutable capability URLs without exposing an arbitrary target", () => {
    const storage = storageFixture();
    const first = registerPairing(`${ORIGIN}/forge/${TOKEN_A}/`, {
      storage,
      expectedOrigin: ORIGIN,
      now: new Date("2030-01-01T00:00:00Z"),
      makeId: () => "pairing-a",
    });
    const second = registerPairing(`${ORIGIN}/forge/${TOKEN_B}/`, {
      storage,
      expectedOrigin: ORIGIN,
      now: new Date("2030-01-01T00:01:00Z"),
      makeId: () => "pairing-b",
    });
    expect(first?.pairing.baseUrl).toBe(`${ORIGIN}/forge/${TOKEN_A}/`);
    expect(second?.pairings.map((pairing) => pairing.id)).toEqual(["pairing-b", "pairing-a"]);
    expect(readPairings(storage, ORIGIN, Date.parse("2030-01-01T00:02:00Z"))).toHaveLength(2);
  });

  it("rejects cross-origin, query-bearing aliases, and non-pairing paths", () => {
    const storage = storageFixture();
    expect(registerPairing(`https://evil.example/forge/${TOKEN_A}/`, { storage, expectedOrigin: ORIGIN })).toBeNull();
    expect(registerPairing(`${ORIGIN}/sessions/arbitrary`, { storage, expectedOrigin: ORIGIN })).toBeNull();
    expect(registerPairing(`${ORIGIN}/forge/${TOKEN_A}/?session=another`, { storage, expectedOrigin: ORIGIN })?.pairing.baseUrl).toBe(`${ORIGIN}/forge/${TOKEN_A}/`);
  });

  it("persists only sanitized snapshot metadata and removes only the revoked pairing", () => {
    const storage = storageFixture();
    const pairings = [
      { id: "pairing-a", baseUrl: `${ORIGIN}/forge/${TOKEN_A}/`, addedAt: "2030-01-01T00:00:00Z" },
      { id: "pairing-b", baseUrl: `${ORIGIN}/forge/${TOKEN_B}/`, addedAt: "2030-01-01T00:00:00Z" },
    ];
    const state: RemoteClientState = {
      phase: "live",
      sessionId: "session-a",
      expiresAt: "2030-01-02T00:00:00Z",
      revision: 2,
      session: sessionFixture({ title: "Working session", cwd: "/workspace/forge", status: "running" }),
      pendingCommands: {},
      needsResync: false,
      reconnectAttempt: 0,
    };
    const updated = updatePairingFromState(pairings, "pairing-a", state, new Date("2030-01-01T01:00:00Z"));
    writePairings(updated, storage);
    expect(storage.getItem(pairingStorageKey())).toContain("Working session");
    expect(storage.getItem(pairingStorageKey())).not.toContain("Keep the terminal active");
    expect(removePairing(updated, "pairing-a").map((pairing) => pairing.id)).toEqual(["pairing-b"]);
  });

  it("prunes locally expired pairings without deleting live siblings", () => {
    const storage = storageFixture();
    writePairings([
      { id: "expired", baseUrl: `${ORIGIN}/forge/${TOKEN_A}/`, addedAt: "2029-01-01T00:00:00Z", expiresAt: "2030-01-01T00:00:00Z" },
      { id: "live", baseUrl: `${ORIGIN}/forge/${TOKEN_B}/`, addedAt: "2029-01-01T00:00:00Z", expiresAt: "2031-01-01T00:00:00Z" },
    ], storage);
    expect(readPairings(storage, ORIGIN, Date.parse("2030-06-01T00:00:00Z")).map((pairing) => pairing.id)).toEqual(["live"]);
  });

  it("removes revoked capabilities but retains an ordinary closed session as Settled history", () => {
    const baseState: RemoteClientState = {
      phase: "live",
      sessionId: "session-a",
      revision: 3,
      session: sessionFixture(),
      pendingCommands: {},
      needsResync: false,
      reconnectAttempt: 0,
    };
    expect(stateInvalidatesPairing({ ...baseState, phase: "revoked", revocationReason: "stopped" })).toBe(true);
    expect(stateInvalidatesPairing({
      ...baseState,
      phase: "closed",
      session: sessionFixture({ status: "closed" }),
    })).toBe(false);
  });
});

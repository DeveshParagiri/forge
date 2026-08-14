import { describe, expect, it } from "vitest";

import { shouldAttachStoredPairing } from "./sessionOrganization";

describe("Forge session attachment organization", () => {
  it("keeps archived pairings stored but detached from ownership sockets", () => {
    expect(
      shouldAttachStoredPairing({
        id: "archived",
        host: "mac.ts.net",
        addedAt: "2026-08-13T10:00:00.000Z",
        metadata: { archivedAt: "2026-08-13T11:00:00.000Z" },
      }),
    ).toBe(false);
    expect(
      shouldAttachStoredPairing({
        id: "active",
        host: "mac.ts.net",
        addedAt: "2026-08-13T10:00:00.000Z",
        metadata: {},
      }),
    ).toBe(true);
  });
});

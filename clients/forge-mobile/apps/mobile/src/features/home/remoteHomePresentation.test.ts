import { describe, expect, it } from "vitest";

import { remoteHomeEmptyState } from "./remoteHomePresentation";

describe("Forge Remote Home empty presentation", () => {
  it("stays neutral without connection/loading prose or a spinner state", () => {
    const presentations = [remoteHomeEmptyState(false), remoteHomeEmptyState(true)];

    expect(presentations).toEqual([
      {
        title: "No paired sessions",
        detail: "Pair a Forge session to see it here.",
        loading: false,
      },
      {
        title: "No sessions yet",
        detail: "Your paired Forge sessions will appear here.",
        loading: false,
      },
    ]);
    expect(presentations.map(({ title, detail }) => `${title} ${detail}`).join(" ")).not.toMatch(
      /connect|loading|retry|offline|unavailable|failed/i,
    );
  });
});

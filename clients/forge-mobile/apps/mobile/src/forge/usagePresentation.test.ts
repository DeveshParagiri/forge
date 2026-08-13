import { describe, expect, it } from "vitest";

import {
  formatUsageCost,
  formatUsageDuration,
  formatUsageReset,
  formatUsageTokens,
  usageStateNotice,
} from "./usagePresentation";

describe("Forge native usage presentation", () => {
  it("formats authoritative cost ticks without floating-point loss", () => {
    expect(formatUsageCost({ costState: "exact", costUsdTicks: "12345678901" })).toBe("$1.2345");
    expect(formatUsageCost({ costState: "partial", costUsdTicks: "12345678901" })).toBe(
      "Unavailable",
    );
    expect(formatUsageCost({ costState: "unavailable" })).toBe("Unavailable");
  });

  it("keeps compact token and duration labels readable on a phone", () => {
    expect(formatUsageTokens(1_250_000)).toBe("1.3M");
    expect(formatUsageTokens(12_500)).toBe("12.5K");
    expect(formatUsageDuration(125_000)).toBe("2m 5s");
  });

  it("prefers the server reset label and falls back to the epoch timestamp", () => {
    expect(formatUsageReset(1_786_651_200, "tomorrow")).toBe("tomorrow");
    expect(formatUsageReset(1_786_651_200, undefined)).toMatch(/Aug/);
  });

  it("distinguishes loading, partial, uncached error, and cached error states", () => {
    expect(usageStateNotice({ status: "loading" }, false)?.text).toBe("Refreshing usage…");
    expect(usageStateNotice({ status: "partial" }, false)?.tone).toBe("warning");
    expect(usageStateNotice({ status: "error" }, false)?.text).toMatch(/no cached data/i);
    expect(
      usageStateNotice(
        {
          status: "error",
          context: {
            usedTokens: 1,
            totalTokens: 2,
            freeTokens: 1,
            usedPercent: 50,
            autoCompactPercent: 90,
          },
        },
        false,
      )?.text,
    ).toMatch(/cached usage/i);
  });
});

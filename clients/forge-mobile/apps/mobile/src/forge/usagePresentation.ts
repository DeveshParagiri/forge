import type { RemoteUsageSession, RemoteUsageSnapshot } from "./protocol/protocol";

const integerFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
const percentFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const updatedAtFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
});
const resetAtFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
});

export function formatUsageTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(value >= 10_000_000 ? 0 : 1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(value >= 100_000 ? 0 : 1)}K`;
  return integerFormatter.format(value);
}

export function formatUsagePercent(value: number): string {
  return `${percentFormatter.format(value)}%`;
}

export function formatUsageDuration(milliseconds: number): string {
  if (milliseconds < 1_000) return `${integerFormatter.format(milliseconds)}ms`;
  const seconds = milliseconds / 1_000;
  if (seconds < 60) return `${seconds.toFixed(seconds >= 10 ? 0 : 1)}s`;
  return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}

export function formatUsageCost(
  session: Pick<RemoteUsageSession, "costState" | "costUsdTicks">,
): string {
  if (session.costState !== "exact" || session.costUsdTicks === undefined) return "Unavailable";
  try {
    const ticks = BigInt(session.costUsdTicks);
    const whole = ticks / 10_000_000_000n;
    const fraction = (ticks % 10_000_000_000n).toString().padStart(10, "0").slice(0, 4);
    return `$${whole.toString()}.${fraction}`;
  } catch {
    return "Unavailable";
  }
}

export function formatUsageUpdatedAt(refreshedAt: string | undefined): string {
  if (!refreshedAt) return "Not refreshed yet";
  const parsed = Date.parse(refreshedAt);
  if (!Number.isFinite(parsed)) return "Last refresh unavailable";
  return `Updated ${updatedAtFormatter.format(parsed)}`;
}

export function formatUsageReset(
  resetAt: number | undefined,
  resetLabel: string | undefined,
): string | undefined {
  if (resetLabel) return resetLabel;
  if (resetAt === undefined) return undefined;
  const date = new Date(resetAt * 1_000);
  if (!Number.isFinite(date.getTime())) return undefined;
  return resetAtFormatter.format(date);
}

export function compactUsageLimitLabel(usage: RemoteUsageSnapshot | undefined): string | undefined {
  const used = usage?.context?.usedTokens;
  const total = usage?.context?.totalTokens;
  if (
    typeof used === "number" &&
    Number.isFinite(used) &&
    typeof total === "number" &&
    Number.isFinite(total) &&
    total > 0
  ) {
    return `${formatUsageTokens(used)} / ${formatUsageTokens(total)}`;
  }
  const window = usage?.account?.windows[0];
  if (window && Number.isFinite(window.usedPercent)) {
    return `${formatUsagePercent(window.usedPercent)} used`;
  }
  return undefined;
}

export function hasUsageData(usage: RemoteUsageSnapshot | undefined): boolean {
  return Boolean(usage?.context || usage?.session || usage?.account);
}

export function usageStateNotice(
  usage: RemoteUsageSnapshot | undefined,
  refreshing: boolean,
): { readonly tone: "neutral" | "warning" | "error"; readonly text: string } | null {
  if (refreshing || usage?.status === "loading") {
    return {
      tone: "neutral",
      text: hasUsageData(usage) ? "Refreshing cached usage…" : "Refreshing usage…",
    };
  }
  if (usage?.status === "partial") {
    return { tone: "warning", text: "Some usage sources are incomplete." };
  }
  if (usage?.status === "error") {
    return {
      tone: "error",
      text: hasUsageData(usage)
        ? "Refresh failed. Showing the last cached usage."
        : "Usage could not be refreshed, and no cached data is available.",
    };
  }
  if (!usage || usage.status === "idle") {
    return { tone: "neutral", text: "Refresh to load usage from this Forge session." };
  }
  return null;
}

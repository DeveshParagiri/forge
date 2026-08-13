/*
 * Adapted from T3 Code's apps/mobile/src/features/usage/UsageRouteScreen.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import AlertTriangle from "lucide-react/dist/esm/icons/triangle-alert.js";
import ChartBar from "lucide-react/dist/esm/icons/chart-bar.js";
import LoaderCircle from "lucide-react/dist/esm/icons/loader-circle.js";
import RefreshCw from "lucide-react/dist/esm/icons/refresh-cw.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { memo, useEffect, useRef, useState } from "react";
import type {
  RemoteUsageAccount,
  RemoteUsageContext,
  RemoteUsageSession,
  RemoteUsageSnapshot,
} from "../protocol";

type UsageTab = "context" | "session" | "account";

const TABS: ReadonlyArray<{ id: UsageTab; label: string }> = [
  { id: "context", label: "Context" },
  { id: "session", label: "This session" },
  { id: "account", label: "Account limits" },
];

const integerFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
const percentFormatter = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const updatedAtFormatter = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const resetAtFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
});

function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(value >= 10_000_000 ? 0 : 1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(value >= 100_000 ? 0 : 1)}K`;
  return integerFormatter.format(value);
}

function formatDuration(milliseconds: number): string {
  if (milliseconds < 1_000) return `${integerFormatter.format(milliseconds)}ms`;
  const seconds = milliseconds / 1_000;
  if (seconds < 60) return `${seconds.toFixed(seconds >= 10 ? 0 : 1)}s`;
  return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}

function formatReset(resetAt?: number, resetLabel?: string): string | undefined {
  if (resetLabel) return resetLabel;
  if (resetAt === undefined) return undefined;
  const milliseconds = resetAt * 1_000;
  const date = new Date(milliseconds);
  if (!Number.isFinite(date.getTime())) return undefined;
  return resetAtFormatter.format(date);
}

function formatCost(session: Pick<RemoteUsageSession, "costState" | "costUsdTicks">): string {
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

function clampPercent(value: number): number {
  return Math.min(100, Math.max(0, value));
}

function UpdatedAt({ refreshedAt }: { refreshedAt?: string }) {
  if (!refreshedAt) return <span>Not refreshed yet</span>;
  const parsed = Date.parse(refreshedAt);
  if (!Number.isFinite(parsed)) return <span>Last refresh unavailable</span>;
  return <span>Updated {updatedAtFormatter.format(parsed)}</span>;
}

function UsageError({ message }: { message: string }) {
  return (
    <div className="usage-section-error" role="status">
      <AlertTriangle aria-hidden="true" />
      <span>{message}</span>
    </div>
  );
}

function Metric({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className="usage-metric">
      <span>{label}</span>
      <strong>{value}</strong>
      {detail ? <small>{detail}</small> : null}
    </div>
  );
}

function Progress({ value, label }: { value: number; label: string }) {
  const clamped = clampPercent(value);
  return (
    <div
      className="usage-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={clamped}
    >
      <i style={{ width: `${clamped}%` }} />
    </div>
  );
}

function ContextPanel({ context, error }: { context?: RemoteUsageContext; error?: string }) {
  if (!context) return <UsageError message={error || "Context usage is not available yet."} />;
  return (
    <section className="usage-panel" aria-label="Context usage">
      {error ? <UsageError message={error} /> : null}
      <div className="usage-hero-card">
        <span>Context used</span>
        <strong>{percentFormatter.format(context.usedPercent)}%</strong>
        <Progress value={context.usedPercent} label="Context used" />
        <small>Automatic compaction begins near {percentFormatter.format(context.autoCompactPercent)}%.</small>
      </div>
      <div className="usage-metric-grid">
        <Metric label="Used" value={formatTokens(context.usedTokens)} />
        <Metric label="Available" value={formatTokens(context.freeTokens)} />
        <Metric label="Window" value={formatTokens(context.totalTokens)} />
      </div>
    </section>
  );
}

function SessionPanel({ session, error }: { session?: RemoteUsageSession; error?: string }) {
  if (!session) return <UsageError message={error || "Session usage is not available yet."} />;
  const costDetail = session.costState === "partial"
    ? "Some calls did not report trustworthy cost."
    : session.costState === "unavailable"
      ? "This provider did not report trustworthy cost."
      : "Reported cost for this session.";
  return (
    <section className="usage-panel" aria-label="Session usage">
      {session.incomplete || session.costState === "partial" ? (
        <UsageError message={error || "Usage is partial and may under-count this session."} />
      ) : error ? <UsageError message={error} /> : null}
      <div className="usage-headline-row">
        <div>
          <span>Processed tokens</span>
          <strong>{formatTokens(session.totalTokens)}</strong>
          <small>{integerFormatter.format(session.modelCalls)} model call{session.modelCalls === 1 ? "" : "s"}</small>
        </div>
        <div>
          <span>Cost</span>
          <strong>{formatCost(session)}</strong>
          <small>{costDetail}</small>
        </div>
      </div>
      <div className="usage-metric-grid">
        <Metric label="Input" value={formatTokens(session.inputTokens)} detail={`${formatTokens(session.cachedReadTokens)} cached`} />
        <Metric label="Output" value={formatTokens(session.outputTokens)} detail={`${formatTokens(session.reasoningTokens)} reasoning`} />
        <Metric label="Cache writes" value={formatTokens(session.cacheCreationTokens)} />
        <Metric label="API time" value={formatDuration(session.apiDurationMs)} />
      </div>
      {session.models?.length ? (
        <div className="usage-model-list" aria-label="Usage by model">
          <h3>By model</h3>
          {session.models.map((model) => (
            <div className="usage-model-row" key={model.modelId}>
              <span><strong>{model.modelId}</strong><small>{formatTokens(model.totalTokens)} tokens · {integerFormatter.format(model.modelCalls)} calls</small></span>
              <strong>{formatCost(model)}</strong>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function AccountPanel({ account, error }: { account?: RemoteUsageAccount; error?: string }) {
  if (!account) return <UsageError message={error || "Account limits are not available yet."} />;
  if (account.status !== "ready") {
    return <UsageError message={error || account.message || "Account limits are unavailable for this provider."} />;
  }
  return (
    <section className="usage-panel" aria-label="Account limits">
      {error ? <UsageError message={error} /> : null}
      <div className="usage-account-heading">
        <span><small>Provider</small><strong>{account.provider}</strong></span>
        {account.plan ? <span><small>Plan</small><strong>{account.plan}</strong></span> : null}
        {account.allowed === false ? <em>Limit reached</em> : null}
      </div>
      {account.windows.length ? (
        <div className="usage-window-list">
          {account.windows.map((window, index) => {
            const reset = formatReset(window.resetAt, window.resetLabel);
            return (
              <div className="usage-window" key={`${window.label}:${index}`}>
                <span><strong>{window.label}</strong><b>{percentFormatter.format(window.usedPercent)}% used</b></span>
                <Progress value={window.usedPercent} label={`${window.label} used`} />
                {reset ? <small>Resets {reset}</small> : null}
              </div>
            );
          })}
        </div>
      ) : <UsageError message={account.message || "This provider did not report quota windows."} />}
      {account.credits ? (
        <div className="usage-credits">
          <span>Credits</span>
          <strong>{account.credits.unlimited ? "Unlimited" : account.credits.balance}</strong>
        </div>
      ) : null}
    </section>
  );
}

export const UsageSheet = memo(function UsageSheet({
  usage,
  refreshing,
  onRefresh,
  onClose,
}: {
  usage?: RemoteUsageSnapshot;
  refreshing: boolean;
  onRefresh(): void;
  onClose(): void;
}) {
  const [tab, setTab] = useState<UsageTab>("context");
  const sheetRef = useRef<HTMLElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const isRefreshing = refreshing || usage?.status === "loading";

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    closeRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== "Tab" || !sheetRef.current) return;
      const focusable = Array.from(
        sheetRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable.at(-1);
      if (event.shiftKey && (document.activeElement === first || !sheetRef.current.contains(document.activeElement))) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousOverflow;
      previousFocus?.focus();
    };
  }, [onClose]);

  const selectTab = (next: UsageTab) => {
    setTab(next);
    window.requestAnimationFrame(() => document.getElementById(`usage-tab-${next}`)?.focus());
  };

  const moveTab = (event: React.KeyboardEvent<HTMLButtonElement>, current: UsageTab) => {
    const currentIndex = TABS.findIndex((item) => item.id === current);
    let nextIndex: number | undefined;
    if (event.key === "ArrowRight") nextIndex = (currentIndex + 1) % TABS.length;
    if (event.key === "ArrowLeft") nextIndex = (currentIndex - 1 + TABS.length) % TABS.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = TABS.length - 1;
    if (nextIndex === undefined) return;
    event.preventDefault();
    const next = TABS[nextIndex];
    if (next) selectTab(next.id);
  };

  return (
    <div className="usage-sheet-backdrop" onPointerDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section
        ref={sheetRef}
        className="usage-sheet"
        role="dialog"
        aria-modal="true"
        aria-busy={isRefreshing}
        aria-labelledby="usage-sheet-title"
      >
        <header className="usage-sheet-header">
          <span className="usage-sheet-mark" aria-hidden="true"><ChartBar /></span>
          <span>
            <h2 id="usage-sheet-title">Usage</h2>
            <UpdatedAt refreshedAt={usage?.refreshedAt} />
          </span>
          <button
            type="button"
            className="usage-refresh"
            aria-label="Refresh usage"
            disabled={isRefreshing}
            onClick={onRefresh}
          >
            <RefreshCw className={isRefreshing ? "status-spin" : undefined} aria-hidden="true" />
          </button>
          <button ref={closeRef} type="button" className="usage-close" aria-label="Close usage" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </header>
        <div className="usage-tabs" role="tablist" aria-label="Usage details">
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              id={`usage-tab-${item.id}`}
              aria-controls={`usage-panel-${item.id}`}
              aria-selected={tab === item.id}
              tabIndex={tab === item.id ? 0 : -1}
              onKeyDown={(event) => moveTab(event, item.id)}
              onClick={() => selectTab(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
        {usage?.status === "partial" ? (
          <div className="usage-state-banner" data-tone="partial"><AlertTriangle aria-hidden="true" />Some usage sources are incomplete.</div>
        ) : null}
        {usage?.status === "error" ? (
          <div className="usage-state-banner" data-tone="error">
            <AlertTriangle aria-hidden="true" />
            {usage.context || usage.session || usage.account
              ? "Usage could not be refreshed. Showing cached data."
              : "Usage could not be refreshed. Cached data is unavailable."}
          </div>
        ) : null}
        {isRefreshing ? <div className="usage-state-banner"><LoaderCircle className="status-spin" aria-hidden="true" />Refreshing usage…</div> : null}
        <div
          className="usage-sheet-scroll"
          role="tabpanel"
          id={`usage-panel-${tab}`}
          aria-labelledby={`usage-tab-${tab}`}
        >
          {tab === "context" ? <ContextPanel context={usage?.context} error={usage?.errors?.context} /> : null}
          {tab === "session" ? <SessionPanel session={usage?.session} error={usage?.errors?.session} /> : null}
          {tab === "account" ? <AccountPanel account={usage?.account} error={usage?.errors?.account} /> : null}
        </div>
      </section>
    </div>
  );
});

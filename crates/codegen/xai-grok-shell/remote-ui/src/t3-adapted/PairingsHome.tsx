/*
 * Adapted from T3 Code mobile's HomeHeader, HomeScreen, and thread-list-v2-items
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right.js";
import Search from "lucide-react/dist/esm/icons/search.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { memo, useMemo, useState } from "react";
import type { StoredPairing } from "../pairingRegistry";

function projectLabel(cwd?: string): string {
  if (!cwd) return "Forge session";
  const parts = cwd.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) || "Forge session";
}

function relativeTime(timestamp?: string): string {
  if (!timestamp) return "Saved";
  const elapsed = Date.now() - Date.parse(timestamp);
  if (!Number.isFinite(elapsed) || elapsed < 0) return "Saved";
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "Now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

function statusPresentation(pairing: StoredPairing): { label: string; tone: string } {
  if (pairing.attention === "approval") return { label: "Approval", tone: "approval" };
  if (pairing.attention === "input") return { label: "Input", tone: "input" };
  if (pairing.status === "running") return { label: "Working", tone: "working" };
  if (pairing.status === "error") return { label: "Failed", tone: "failed" };
  if (pairing.status === "closed") return { label: "Settled", tone: "quiet" };
  return { label: relativeTime(pairing.lastSeenAt), tone: "quiet" };
}

const PairingRow = memo(function PairingRow({
  pairing,
  onSelect,
  onRemove,
}: {
  pairing: StoredPairing;
  onSelect(pairing: StoredPairing): void;
  onRemove(pairing: StoredPairing): void;
}) {
  const status = statusPresentation(pairing);
  return (
    <article className="pairing-row">
      <button
        type="button"
        className="pairing-row-main"
        aria-label={`Open ${pairing.title || "Forge session"}`}
        onClick={() => onSelect(pairing)}
      >
        <span className="pairing-row-topline">
          <span className="pairing-project">{projectLabel(pairing.cwd)}</span>
          <span className="pairing-status" data-tone={status.tone}>{status.label}</span>
        </span>
        <strong>{pairing.title || "New Forge pairing"}</strong>
        <span className="pairing-row-meta">
          <span>{pairing.cwd || "Private tailnet session"}</span>
          {pairing.modelLabel ? <span>{pairing.modelLabel}</span> : null}
        </span>
        <ChevronRight className="pairing-row-chevron" aria-hidden="true" />
      </button>
      <button
        type="button"
        className="pairing-remove"
        aria-label={`Remove ${pairing.title || "Forge session"}`}
        onClick={() => onRemove(pairing)}
      >
        <X aria-hidden="true" />
      </button>
    </article>
  );
});

export function PairingsHome({
  pairings,
  onSelect,
  onRemove,
}: {
  pairings: StoredPairing[];
  onSelect(pairing: StoredPairing): void;
  onRemove(pairing: StoredPairing): void;
}) {
  const [query, setQuery] = useState("");
  const visiblePairings = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return pairings;
    return pairings.filter((pairing) =>
      [pairing.title, pairing.cwd, pairing.modelLabel]
        .filter(Boolean)
        .some((value) => value?.toLocaleLowerCase().includes(normalized)),
    );
  }, [pairings, query]);

  return (
    <main className="pairings-home">
      <header className="home-header">
        <div className="forge-wordmark" aria-label="Forge"><strong>Forge</strong></div>
      </header>
      <section className="pairings-list" aria-label="Forge sessions">
        {visiblePairings.length ? (
          visiblePairings.map((pairing) => (
            <PairingRow key={pairing.id} pairing={pairing} onSelect={onSelect} onRemove={onRemove} />
          ))
        ) : (
          <div className="pairings-empty">
            <h1>{pairings.length ? "No matching sessions" : "No Forge sessions yet"}</h1>
            <p>{pairings.length ? "Try another search." : "Run /rc in a Forge terminal, then scan its private QR code."}</p>
          </div>
        )}
      </section>
      <div className="home-search-dock">
        <label className="home-search">
          <Search aria-hidden="true" />
          <span className="sr-only">Search sessions</span>
          <input
            type="search"
            value={query}
            aria-label="Search sessions"
            placeholder="Search"
            onChange={(event) => setQuery(event.target.value)}
          />
          {query ? (
            <button type="button" aria-label="Clear search" onClick={() => setQuery("")}><X aria-hidden="true" /></button>
          ) : null}
        </label>
      </div>
    </main>
  );
}

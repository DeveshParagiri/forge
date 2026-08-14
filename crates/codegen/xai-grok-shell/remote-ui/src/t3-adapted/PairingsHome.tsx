/*
 * Adapted from T3 Code mobile's HomeHeader, HomeScreen, and thread-list-v2-items
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down.js";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right.js";
import Folder from "lucide-react/dist/esm/icons/folder.js";
import MoreHorizontal from "lucide-react/dist/esm/icons/ellipsis.js";
import Plus from "lucide-react/dist/esm/icons/plus.js";
import Search from "lucide-react/dist/esm/icons/search.js";
import SquarePen from "lucide-react/dist/esm/icons/square-pen.js";
import Trash2 from "lucide-react/dist/esm/icons/trash-2.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { memo, useEffect, useId, useMemo, useRef, useState } from "react";
import forgeMarkUrl from "../../../../../../clients/forge-mobile/apps/mobile/assets/forge/mark.png";
import type { StoredPairing } from "../pairingRegistry";
import {
  buildPairingHomeProjectGroups,
  type PairingHomeProjectGroup,
} from "./pairingsHomePresentation";

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

function statusPresentation(pairing: StoredPairing): { label: string; tone: string } | null {
  if (pairing.attention === "approval") return { label: "Approval", tone: "approval" };
  if (pairing.attention === "input") return { label: "Input", tone: "input" };
  if (pairing.status === "running") return { label: "Working", tone: "working" };
  if (pairing.status === "error") return { label: "Failed", tone: "failed" };
  if (pairing.status === "closed") return { label: "Settled", tone: "quiet" };
  return null;
}

const collapsedProjectKeys = new Set<string>();

function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );

  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduced(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  return reduced;
}

const PairingRow = memo(function PairingRow({
  pairing,
  onSelect,
  onRemove,
}: {
  pairing: StoredPairing;
  onSelect(pairing: StoredPairing): void;
  onRemove?(pairing: StoredPairing): void;
}) {
  const status = statusPresentation(pairing);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuId = useId();
  const menuRootRef = useRef<HTMLDivElement>(null);
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const title = pairing.title || "New Forge pairing";

  useEffect(() => {
    if (!menuOpen) return;
    const focusFrame = window.requestAnimationFrame(() => {
      menuRootRef.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    });
    const closeOutside = (event: PointerEvent) => {
      if (!menuRootRef.current?.contains(event.target as Node)) setMenuOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setMenuOpen(false);
        menuButtonRef.current?.focus();
        return;
      }
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp" && event.key !== "Home" && event.key !== "End") return;
      const items = Array.from(
        menuRootRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
      );
      if (items.length === 0) return;
      event.preventDefault();
      const current = items.indexOf(document.activeElement as HTMLElement);
      if (event.key === "Home") items[0].focus();
      else if (event.key === "End") items[items.length - 1].focus();
      else if (event.key === "ArrowDown") items[(current + 1 + items.length) % items.length].focus();
      else items[(current - 1 + items.length) % items.length].focus();
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [menuOpen]);

  return (
    <article className="pairing-row">
      <button
        type="button"
        className="pairing-row-main"
        aria-label={`Open ${title}`}
        onClick={() => onSelect(pairing)}
      >
        <strong>{title}</strong>
        <span className="pairing-thread-trailing">
          {status ? <span className="pairing-status" data-tone={status.tone}>{status.label}</span> : null}
          <time dateTime={pairing.lastSeenAt}>{relativeTime(pairing.lastSeenAt)}</time>
          <ChevronRight className="pairing-row-chevron" aria-hidden="true" />
        </span>
      </button>
      {onRemove ? (
        <div
          className="pairing-row-menu-root"
          ref={menuRootRef}
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget)) setMenuOpen(false);
          }}
        >
          <button
            ref={menuButtonRef}
            type="button"
            className="pairing-row-overflow"
            aria-label={`More options for ${title}`}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-controls={menuOpen ? menuId : undefined}
            onClick={() => setMenuOpen((open) => !open)}
          >
            <MoreHorizontal aria-hidden="true" />
          </button>
          {menuOpen ? (
            <div id={menuId} className="pairing-row-menu" role="menu" aria-label={`Options for ${title}`}>
              <button
                type="button"
                role="menuitem"
                className="pairing-row-menu-danger"
                onClick={() => {
                  setMenuOpen(false);
                  onRemove(pairing);
                }}
              >
                <Trash2 aria-hidden="true" />
                <span>Remove session</span>
              </button>
            </div>
          ) : null}
        </div>
      ) : null}
    </article>
  );
});

const ProjectGroup = memo(function ProjectGroup({
  group,
  collapsed,
  reduceMotion,
  onToggle,
  onSelect,
  onRemove,
  onCreateSession,
}: {
  group: PairingHomeProjectGroup;
  collapsed: boolean;
  reduceMotion: boolean;
  onToggle(): void;
  onSelect(pairing: StoredPairing): void;
  onRemove?(pairing: StoredPairing): void;
  onCreateSession?(group: PairingHomeProjectGroup): void;
}) {
  const groupBodyId = useId();
  return (
    <section
      className="pairing-project-group"
      data-state={collapsed ? "collapsed" : "expanded"}
      data-disclosure-motion={reduceMotion ? "reduce" : "animate"}
    >
      <header className="pairing-project-header">
        <button
          type="button"
          className="pairing-project-toggle"
          aria-label={`${collapsed ? "Expand" : "Collapse"} ${group.title}, ${group.pairings.length} ${group.pairings.length === 1 ? "session" : "sessions"}`}
          aria-expanded={!collapsed}
          aria-controls={groupBodyId}
          onClick={onToggle}
        >
          <Folder className="pairing-project-folder" aria-hidden="true" />
          <strong>{group.title}</strong>
          {collapsed ? <ChevronRight aria-hidden="true" /> : <ChevronDown aria-hidden="true" />}
          <span className="pairing-project-count" aria-hidden="true">{group.pairings.length}</span>
        </button>
        {onCreateSession && group.cwd ? (
          <button
            type="button"
            className="pairing-project-new-session"
            aria-label={`Create new session in ${group.title}`}
            onClick={() => onCreateSession(group)}
          >
            <SquarePen aria-hidden="true" />
          </button>
        ) : null}
      </header>
      <div id={groupBodyId} className="pairing-project-threads" hidden={collapsed}>
        {group.pairings.map((pairing) => (
          <PairingRow
            key={pairing.id}
            pairing={pairing}
            onSelect={onSelect}
            onRemove={onRemove}
          />
        ))}
      </div>
    </section>
  );
});

export function PairingsHome({
  pairings,
  onSelect,
  onRemove,
  onCreateSession,
  onCreateSessionInProject,
}: {
  pairings: StoredPairing[];
  onSelect(pairing: StoredPairing): void;
  onRemove?(pairing: StoredPairing): void;
  onCreateSession?(): void;
  onCreateSessionInProject?(group: PairingHomeProjectGroup): void;
}) {
  const [query, setQuery] = useState("");
  const [collapsedKeys, setCollapsedKeys] = useState(() => new Set(collapsedProjectKeys));
  const reduceMotion = usePrefersReducedMotion();
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visiblePairings = useMemo(() => {
    if (!normalizedQuery) return pairings;
    return pairings.filter((pairing) =>
      [pairing.title, pairing.cwd, pairing.modelLabel]
        .filter(Boolean)
        .some((value) => value?.toLocaleLowerCase().includes(normalizedQuery)),
    );
  }, [normalizedQuery, pairings]);
  const groups = useMemo(() => buildPairingHomeProjectGroups(visiblePairings), [visiblePairings]);

  const toggleGroup = (key: string) => {
    setCollapsedKeys((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
        collapsedProjectKeys.delete(key);
      } else {
        next.add(key);
        collapsedProjectKeys.add(key);
      }
      return next;
    });
  };

  return (
    <main className="pairings-home">
      <header className="home-header">
        <div className="forge-wordmark" aria-label="Forge">
          <img src={forgeMarkUrl} alt="" aria-hidden="true" />
          <strong>Forge</strong>
        </div>
        {onCreateSession ? (
          <button
            type="button"
            className="home-new-session"
            aria-label="Create new Forge session"
            onClick={onCreateSession}
          >
            <Plus aria-hidden="true" />
          </button>
        ) : null}
      </header>
      <section className="pairings-list" aria-label="Forge sessions">
        {groups.length ? (
          groups.map((group) => (
            <ProjectGroup
              key={group.key}
              group={group}
              collapsed={normalizedQuery ? false : collapsedKeys.has(group.key)}
              reduceMotion={reduceMotion}
              onToggle={() => toggleGroup(group.key)}
              onSelect={onSelect}
              onRemove={onRemove}
              onCreateSession={onCreateSessionInProject}
            />
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

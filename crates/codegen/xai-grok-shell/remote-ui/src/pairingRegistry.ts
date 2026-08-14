import type { RemoteClientState } from "./reducer";

const STORAGE_KEY = "forge.remote.pairings.v1";
const PAIRING_PATH = /^\/forge\/[a-f0-9]{64}\/$/i;

export interface StoredPairing {
  id: string;
  baseUrl: string;
  addedAt: string;
  lastSeenAt?: string;
  expiresAt?: string;
  sessionId?: string;
  title?: string;
  cwd?: string;
  status?: "idle" | "running" | "waiting_for_input" | "error" | "closed";
  modelLabel?: string;
  attention?: "approval" | "input";
}

interface StoredPairingEnvelope {
  version: 1;
  pairings: StoredPairing[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function optionalString(record: Record<string, unknown>, key: string): string | undefined {
  return typeof record[key] === "string" ? record[key] : undefined;
}

export function canonicalPairingUrl(input: string, expectedOrigin: string): string | null {
  try {
    const url = new URL(input);
    if (url.origin !== expectedOrigin || url.username || url.password) return null;
    if (url.protocol !== "https:" && !(url.protocol === "http:" && ["127.0.0.1", "localhost"].includes(url.hostname))) {
      return null;
    }
    if (!url.pathname.endsWith("/")) url.pathname += "/";
    if (!PAIRING_PATH.test(url.pathname)) return null;
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return null;
  }
}

function decodePairing(value: unknown, expectedOrigin: string): StoredPairing | null {
  if (!isRecord(value)) return null;
  const baseUrl = optionalString(value, "baseUrl");
  const id = optionalString(value, "id");
  const addedAt = optionalString(value, "addedAt");
  if (!baseUrl || !id || !addedAt) return null;
  const canonicalUrl = canonicalPairingUrl(baseUrl, expectedOrigin);
  if (!canonicalUrl || canonicalUrl !== baseUrl) return null;
  return {
    id,
    baseUrl,
    addedAt,
    ...(optionalString(value, "lastSeenAt") ? { lastSeenAt: optionalString(value, "lastSeenAt") } : {}),
    ...(optionalString(value, "expiresAt") ? { expiresAt: optionalString(value, "expiresAt") } : {}),
    ...(optionalString(value, "sessionId") ? { sessionId: optionalString(value, "sessionId") } : {}),
    ...(optionalString(value, "title") ? { title: optionalString(value, "title") } : {}),
    ...(optionalString(value, "cwd") ? { cwd: optionalString(value, "cwd") } : {}),
    ...(["idle", "running", "waiting_for_input", "error", "closed"].includes(String(value.status))
      ? { status: value.status as StoredPairing["status"] }
      : {}),
    ...(optionalString(value, "modelLabel") ? { modelLabel: optionalString(value, "modelLabel") } : {}),
    ...(value.attention === "approval" || value.attention === "input"
      ? { attention: value.attention }
      : {}),
  };
}

function isExpired(pairing: StoredPairing, now: number): boolean {
  if (!pairing.expiresAt) return false;
  const expiry = Date.parse(pairing.expiresAt);
  return Number.isFinite(expiry) && expiry <= now;
}

function pairingHost(baseUrl: string): string {
  try {
    return new URL(baseUrl).host.toLocaleLowerCase();
  } catch {
    return "";
  }
}

/**
 * A bearer URL is intentionally rotated whenever `/rc` is reopened. Once the
 * authoritative snapshot tells us which session that bearer belongs to, keep
 * one bearer-free row identity for that host and session.
 */
export function pairingIdForSession(baseUrl: string, sessionId: string): string {
  return `session:${encodeURIComponent(pairingHost(baseUrl))}:${encodeURIComponent(sessionId)}`;
}

function pairingActivityMs(pairing: StoredPairing): number {
  for (const value of [pairing.lastSeenAt, pairing.addedAt]) {
    if (!value) continue;
    const parsed = Date.parse(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return 0;
}

function normalizeSessionPairings(pairings: StoredPairing[]): StoredPairing[] {
  const byIdentity = new Map<string, StoredPairing>();
  const order: string[] = [];
  for (const pairing of pairings) {
    const identity = pairing.sessionId
      ? pairingIdForSession(pairing.baseUrl, pairing.sessionId)
      : pairing.id;
    const normalized = pairing.sessionId ? { ...pairing, id: identity } : pairing;
    const previous = byIdentity.get(identity);
    if (!previous) {
      byIdentity.set(identity, normalized);
      order.push(identity);
      continue;
    }
    const newer = pairingActivityMs(normalized) >= pairingActivityMs(previous) ? normalized : previous;
    const older = newer === normalized ? previous : normalized;
    byIdentity.set(identity, {
      ...older,
      ...newer,
      id: identity,
      addedAt:
        Date.parse(older.addedAt) < Date.parse(newer.addedAt) ? older.addedAt : newer.addedAt,
    });
  }
  return order.map((identity) => byIdentity.get(identity)).filter((pairing): pairing is StoredPairing => Boolean(pairing));
}

export function readPairings(
  storage: Pick<Storage, "getItem" | "setItem"> = window.localStorage,
  expectedOrigin = window.location.origin,
  now = Date.now(),
): StoredPairing[] {
  const serialized = storage.getItem(STORAGE_KEY);
  if (!serialized) return [];
  try {
    const envelope: unknown = JSON.parse(serialized);
    if (!isRecord(envelope) || envelope.version !== 1 || !Array.isArray(envelope.pairings)) {
      return [];
    }
    const decoded = envelope.pairings
      .map((value) => decodePairing(value, expectedOrigin))
      .filter((pairing): pairing is StoredPairing => Boolean(pairing));
    const live = normalizeSessionPairings(decoded.filter((pairing) => !isExpired(pairing, now)));
    if (
      live.length !== decoded.length ||
      live.some((pairing, index) => pairing.id !== decoded[index]?.id)
    ) {
      writePairings(live, storage);
    }
    return live;
  } catch {
    return [];
  }
}

export function writePairings(
  pairings: StoredPairing[],
  storage: Pick<Storage, "setItem"> = window.localStorage,
): void {
  const envelope: StoredPairingEnvelope = { version: 1, pairings };
  storage.setItem(STORAGE_KEY, JSON.stringify(envelope));
}

export function registerPairing(
  href: string,
  options: {
    storage?: Pick<Storage, "getItem" | "setItem">;
    expectedOrigin?: string;
    now?: Date;
    makeId?: () => string;
  } = {},
): { pairing: StoredPairing; pairings: StoredPairing[] } | null {
  const expectedOrigin = options.expectedOrigin ?? window.location.origin;
  const storage = options.storage ?? window.localStorage;
  const now = options.now ?? new Date();
  const baseUrl = canonicalPairingUrl(href, expectedOrigin);
  if (!baseUrl) return null;
  const pairings = readPairings(storage, expectedOrigin, now.valueOf());
  const existing = pairings.find((pairing) => pairing.baseUrl === baseUrl);
  const pairing: StoredPairing = existing ?? {
    id: (options.makeId ?? (() => crypto.randomUUID()))(),
    baseUrl,
    addedAt: now.toISOString(),
  };
  const next = [pairing, ...pairings.filter((candidate) => candidate.id !== pairing.id)];
  writePairings(next, storage);
  return { pairing, pairings: next };
}

export function updatePairingFromState(
  pairings: StoredPairing[],
  pairingId: string,
  state: RemoteClientState,
  now = new Date(),
): StoredPairing[] {
  const interaction = state.session?.activeInteractions.find(
    (candidate) => candidate.status === undefined || candidate.status === "pending",
  );
  const target = pairings.find((pairing) => pairing.id === pairingId);
  if (!target) return pairings;
  const updated: StoredPairing = {
    ...target,
    ...(state.expiresAt ? { expiresAt: state.expiresAt } : {}),
    ...(state.sessionId ? { sessionId: state.sessionId } : {}),
    ...(state.session?.title ? { title: state.session.title } : {}),
    ...(state.session?.cwd ? { cwd: state.session.cwd } : {}),
    ...(state.session?.status ? { status: state.session.status } : {}),
    ...(state.session?.currentModel?.label
      ? { modelLabel: state.session.currentModel.label }
      : {}),
    ...(interaction?.kind === "permission" || interaction?.kind === "plan"
      ? { attention: "approval" as const }
      : interaction?.kind === "question"
        ? { attention: "input" as const }
        : { attention: undefined }),
    lastSeenAt: now.toISOString(),
  };
  if (!updated.sessionId) {
    return pairings.map((pairing) => (pairing.id === pairingId ? updated : pairing));
  }

  const stableId = pairingIdForSession(updated.baseUrl, updated.sessionId);
  const host = pairingHost(updated.baseUrl);
  const duplicate = pairings.find(
    (pairing) =>
      pairing.id !== pairingId &&
      pairing.sessionId === updated.sessionId &&
      pairingHost(pairing.baseUrl) === host,
  );
  const merged: StoredPairing = {
    ...(duplicate ?? {}),
    ...updated,
    id: stableId,
    addedAt:
      duplicate && Date.parse(duplicate.addedAt) < Date.parse(updated.addedAt)
        ? duplicate.addedAt
        : updated.addedAt,
  };
  const survivors = pairings.filter(
    (pairing) =>
      pairing.id !== pairingId &&
      !(
        pairing.sessionId === updated.sessionId &&
        pairingHost(pairing.baseUrl) === host
      ),
  );
  return [merged, ...survivors];
}

export function removePairing(pairings: StoredPairing[], pairingId: string): StoredPairing[] {
  return pairings.filter((pairing) => pairing.id !== pairingId);
}

export function stateInvalidatesPairing(state: RemoteClientState): boolean {
  return state.phase === "revoked";
}

export function pairingStorageKey(): string {
  return STORAGE_KEY;
}

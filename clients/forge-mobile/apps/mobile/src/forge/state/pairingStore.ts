import * as Crypto from "expo-crypto";
import * as SecureStore from "expo-secure-store";

import { pairingDisplayHost, parsePairingInput } from "../protocol/pairing";

const INDEX_KEY = "forge.remote.pairings.v1";
const RECORD_PREFIX = "forge.remote.pairing.v1.";
export const MAX_SESSION_ALIAS_LENGTH = 120;

export interface PairingMetadata {
  /** User-owned display override. The remote title cache remains in `title`. */
  readonly customTitle?: string;
  readonly title?: string;
  readonly status?: "idle" | "running" | "waiting_for_input" | "error" | "closed";
  readonly lastSeenAt?: string;
  readonly expiresAt?: string;
  /** Local-only organization. These values are never sent to the Forge host. */
  readonly pinnedAt?: string;
  readonly pinOrderKey?: string;
  readonly archivedAt?: string;
}

export interface StoredPairing {
  readonly id: string;
  readonly gatewayUrl: string;
  /** Immutable session identity advertised when a fresh child pairing is created. */
  readonly sessionId?: string;
  /** Two-phase child handoff marker. Cleared only after server acceptance. */
  readonly provisionalUntil?: string;
  readonly addedAt: string;
  readonly metadata: PairingMetadata;
}

export interface StoredPairingRegistration {
  readonly created: boolean;
  readonly record: StoredPairing;
}

export interface StoredPairingIdentityReconciliation {
  /** The stable record that should remain visible after identity reconciliation. */
  readonly record: StoredPairing;
  /** Duplicate records removed from SecureStore, when any existed. */
  readonly removedPairingIds?: ReadonlyArray<string>;
}

export interface PairingSummary {
  readonly id: string;
  readonly host: string;
  readonly addedAt: string;
  readonly metadata: PairingMetadata;
}

function optionalBoundedString(value: unknown, maxLength: number): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 && trimmed.length <= maxLength ? trimmed : undefined;
}

export function normalizeSessionAlias(value: string | null | undefined): string | undefined {
  return optionalBoundedString(value, MAX_SESSION_ALIAS_LENGTH);
}

function parseMetadata(value: unknown): PairingMetadata {
  if (!value || typeof value !== "object") return {};
  const metadata = value as Record<string, unknown>;
  const status =
    metadata.status === "idle" ||
    metadata.status === "running" ||
    metadata.status === "waiting_for_input" ||
    metadata.status === "error" ||
    metadata.status === "closed"
      ? metadata.status
      : undefined;
  return {
    ...(normalizeSessionAlias(
      typeof metadata.customTitle === "string" ? metadata.customTitle : null,
    )
      ? { customTitle: normalizeSessionAlias(metadata.customTitle as string) }
      : {}),
    ...(optionalBoundedString(metadata.title, 1_000)
      ? { title: optionalBoundedString(metadata.title, 1_000) }
      : {}),
    ...(status ? { status } : {}),
    ...(optionalBoundedString(metadata.lastSeenAt, 100)
      ? { lastSeenAt: optionalBoundedString(metadata.lastSeenAt, 100) }
      : {}),
    ...(optionalBoundedString(metadata.expiresAt, 100)
      ? { expiresAt: optionalBoundedString(metadata.expiresAt, 100) }
      : {}),
    ...(optionalBoundedString(metadata.pinnedAt, 100)
      ? { pinnedAt: optionalBoundedString(metadata.pinnedAt, 100) }
      : {}),
    ...(optionalBoundedString(metadata.pinOrderKey, 200)
      ? { pinOrderKey: optionalBoundedString(metadata.pinOrderKey, 200) }
      : {}),
    ...(optionalBoundedString(metadata.archivedAt, 100)
      ? { archivedAt: optionalBoundedString(metadata.archivedAt, 100) }
      : {}),
  };
}

function recordKey(id: string): string {
  return `${RECORD_PREFIX}${id}`;
}

const metadataUpdateById = new Map<string, Promise<void>>();
let identityUpdate: Promise<unknown> = Promise.resolve();

function serializeIdentityUpdate<T>(operation: () => Promise<T>): Promise<T> {
  const update = identityUpdate.catch(() => undefined).then(operation);
  identityUpdate = update;
  return update;
}

async function awaitPendingMetadataUpdates(): Promise<void> {
  await Promise.all(
    [...metadataUpdateById.values()].map((update) => update.catch(() => undefined)),
  );
}

function parseIndex(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((entry): entry is string => typeof entry === "string" && entry.length > 0)
      : [];
  } catch {
    return [];
  }
}

function parseRecord(raw: string | null): StoredPairing | null {
  if (!raw) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;
    const value = parsed as Partial<StoredPairing>;
    if (
      typeof value.id !== "string" ||
      typeof value.gatewayUrl !== "string" ||
      typeof value.addedAt !== "string"
    ) {
      return null;
    }
    return {
      id: value.id,
      gatewayUrl: parsePairingInput(value.gatewayUrl),
      ...(optionalBoundedString(value.sessionId, 256)
        ? { sessionId: optionalBoundedString(value.sessionId, 256) }
        : {}),
      ...(optionalBoundedString(value.provisionalUntil, 100)
        ? { provisionalUntil: optionalBoundedString(value.provisionalUntil, 100) }
        : {}),
      addedAt: value.addedAt,
      metadata: parseMetadata(value.metadata),
    };
  } catch {
    return null;
  }
}

async function saveIndex(ids: ReadonlyArray<string>): Promise<void> {
  await SecureStore.setItemAsync(INDEX_KEY, JSON.stringify(ids), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
}

async function saveRecord(record: StoredPairing): Promise<void> {
  await SecureStore.setItemAsync(recordKey(record.id), JSON.stringify(record), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
}

export async function loadStoredPairings(): Promise<StoredPairing[]> {
  const ids = parseIndex(await SecureStore.getItemAsync(INDEX_KEY));
  const records = await Promise.all(
    ids.map(async (id) => parseRecord(await SecureStore.getItemAsync(recordKey(id)))),
  );
  return records.filter((record): record is StoredPairing => record !== null);
}

export async function registerStoredPairingWithStatus(
  input: string,
  expectedSessionId?: string,
): Promise<StoredPairingRegistration> {
  const gatewayUrl = parsePairingInput(input);
  const sessionId = optionalBoundedString(expectedSessionId, 256);
  if (expectedSessionId !== undefined && sessionId === undefined) {
    throw new Error("The new Forge session returned an invalid session identity.");
  }
  const current = await loadStoredPairings();
  const existing = current.find((record) => record.gatewayUrl === gatewayUrl);
  if (existing) {
    if (sessionId) throw new Error("Forge did not return a fresh child pairing.");
    if (existing.metadata.archivedAt === undefined) {
      return { created: false, record: existing };
    }
    const restored: StoredPairing = {
      ...existing,
      metadata: { ...existing.metadata, archivedAt: undefined },
    };
    await saveRecord(restored);
    return { created: false, record: restored };
  }

  const record: StoredPairing = {
    id: Crypto.randomUUID(),
    gatewayUrl,
    ...(sessionId ? { sessionId } : {}),
    ...(sessionId ? { provisionalUntil: new Date(Date.now() + 35_000).toISOString() } : {}),
    addedAt: new Date().toISOString(),
    metadata: {},
  };
  await saveRecord(record);
  await saveIndex([...current.map((entry) => entry.id), record.id]);
  return { created: true, record };
}

export async function registerStoredPairing(
  input: string,
  expectedSessionId?: string,
): Promise<StoredPairing> {
  return (await registerStoredPairingWithStatus(input, expectedSessionId)).record;
}

/**
 * Pins the immutable session identity learned from a gateway handshake. This
 * makes a later /rc scan able to recognize the same session even when the
 * terminal issued a new bearer capability URL.
 */
export function bindStoredPairingSessionIdentity(
  id: string,
  sessionIdInput: string,
): Promise<StoredPairing> {
  return serializeIdentityUpdate(async () => {
    await awaitPendingMetadataUpdates();
    const sessionId = optionalBoundedString(sessionIdInput, 256);
    if (!sessionId) throw new Error("Forge returned an invalid session identity.");
    const record = parseRecord(await SecureStore.getItemAsync(recordKey(id)));
    if (!record) throw new Error("The Forge pairing no longer exists.");
    if (record.sessionId && record.sessionId !== sessionId) {
      throw new Error("The Forge pairing opened a different session.");
    }
    if (record.sessionId === sessionId) return record;
    const bound = { ...record, sessionId };
    await saveRecord(bound);
    return bound;
  });
}

/**
 * Reconciles a newly scanned bearer URL against durable session identity. A
 * fresh URL for an already-known session updates the existing stable record,
 * preserving its alias, pin/archive state, and Home/thread identity.
 */
export function reconcileStoredPairingSessionIdentity(
  incomingId: string,
  sessionIdInput: string,
): Promise<StoredPairingIdentityReconciliation> {
  return serializeIdentityUpdate(async () => {
    await awaitPendingMetadataUpdates();
    const sessionId = optionalBoundedString(sessionIdInput, 256);
    if (!sessionId) throw new Error("Forge returned an invalid session identity.");
    const records = await loadStoredPairings();
    const incoming = records.find((record) => record.id === incomingId);
    if (!incoming) throw new Error("The scanned Forge pairing no longer exists.");
    if (incoming.sessionId && incoming.sessionId !== sessionId) {
      throw new Error("The scanned Forge pairing opened a different session.");
    }

    const incomingHost = pairingDisplayHost(incoming.gatewayUrl);
    const matches = records.filter(
      (record) =>
        record.id !== incomingId &&
        record.sessionId === sessionId &&
        pairingDisplayHost(record.gatewayUrl) === incomingHost,
    );
    const existing = matches[0];
    if (!existing) {
      const bound = incoming.sessionId === sessionId ? incoming : { ...incoming, sessionId };
      if (bound !== incoming) await saveRecord(bound);
      return { record: bound };
    }

    const rebound: StoredPairing = {
      ...existing,
      gatewayUrl: incoming.gatewayUrl,
      sessionId,
      metadata: { ...existing.metadata, archivedAt: undefined },
    };
    const removedPairingIds = [incomingId, ...matches.slice(1).map((record) => record.id)];
    await saveRecord(rebound);
    await Promise.all(removedPairingIds.map((id) => SecureStore.deleteItemAsync(recordKey(id))));
    await saveIndex(
      records.map((record) => record.id).filter((id) => !removedPairingIds.includes(id)),
    );
    return { record: rebound, removedPairingIds };
  });
}

/**
 * Backfills one already-open pairing and removes legacy duplicates that have
 * since proven to be the exact same immutable remote session. The currently
 * open pairing remains stable so its active navigation route cannot disappear.
 */
export function deduplicateStoredPairingSessionIdentity(
  preferredId: string,
  sessionIdInput: string,
): Promise<StoredPairingIdentityReconciliation> {
  return serializeIdentityUpdate(async () => {
    await awaitPendingMetadataUpdates();
    const sessionId = optionalBoundedString(sessionIdInput, 256);
    if (!sessionId) throw new Error("Forge returned an invalid session identity.");
    const records = await loadStoredPairings();
    const preferred = records.find((record) => record.id === preferredId);
    if (!preferred) throw new Error("The Forge pairing no longer exists.");
    if (preferred.sessionId && preferred.sessionId !== sessionId) {
      throw new Error("The Forge pairing opened a different session.");
    }

    const preferredHost = pairingDisplayHost(preferred.gatewayUrl);
    const duplicates = records.filter(
      (record) =>
        record.id !== preferredId &&
        record.sessionId === sessionId &&
        pairingDisplayHost(record.gatewayUrl) === preferredHost,
    );
    const bound = preferred.sessionId === sessionId ? preferred : { ...preferred, sessionId };
    await saveRecord(bound);
    if (duplicates.length === 0) return { record: bound };

    const removedPairingIds = duplicates.map((record) => record.id);
    await Promise.all(removedPairingIds.map((id) => SecureStore.deleteItemAsync(recordKey(id))));
    await saveIndex(
      records.map((record) => record.id).filter((id) => !removedPairingIds.includes(id)),
    );
    return { record: bound, removedPairingIds };
  });
}

export async function finalizeStoredPairing(id: string): Promise<void> {
  const record = parseRecord(await SecureStore.getItemAsync(recordKey(id)));
  if (!record) throw new Error("The provisional Forge pairing no longer exists.");
  const { provisionalUntil: _provisionalUntil, ...accepted } = record;
  await saveRecord(accepted);
}

export async function updateStoredPairingMetadata(
  id: string,
  metadata: PairingMetadata,
): Promise<void> {
  const previous = metadataUpdateById.get(id) ?? Promise.resolve();
  const update = previous
    .catch(() => undefined)
    .then(async () => {
      const record = parseRecord(await SecureStore.getItemAsync(recordKey(id)));
      if (!record) return;
      await saveRecord({ ...record, metadata: { ...record.metadata, ...metadata } });
    });
  metadataUpdateById.set(id, update);
  try {
    await update;
  } finally {
    if (metadataUpdateById.get(id) === update) metadataUpdateById.delete(id);
  }
}

export async function removeStoredPairing(id: string): Promise<void> {
  const ids = parseIndex(await SecureStore.getItemAsync(INDEX_KEY));
  await SecureStore.deleteItemAsync(recordKey(id));
  await saveIndex(ids.filter((candidate) => candidate !== id));
}

export function summarizePairing(record: StoredPairing): PairingSummary {
  return {
    id: record.id,
    host: pairingDisplayHost(record.gatewayUrl),
    addedAt: record.addedAt,
    metadata: record.metadata,
  };
}

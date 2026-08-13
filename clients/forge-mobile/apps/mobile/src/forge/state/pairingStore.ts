import * as Crypto from "expo-crypto";
import * as SecureStore from "expo-secure-store";

import { pairingDisplayHost, parsePairingInput } from "../protocol/pairing";

const INDEX_KEY = "forge.remote.pairings.v1";
const RECORD_PREFIX = "forge.remote.pairing.v1.";

export interface PairingMetadata {
  readonly title?: string;
  readonly status?: "idle" | "running" | "waiting_for_input" | "error" | "closed";
  readonly lastSeenAt?: string;
  readonly expiresAt?: string;
}

export interface StoredPairing {
  readonly id: string;
  readonly gatewayUrl: string;
  readonly addedAt: string;
  readonly metadata: PairingMetadata;
}

export interface PairingSummary {
  readonly id: string;
  readonly host: string;
  readonly addedAt: string;
  readonly metadata: PairingMetadata;
}

function recordKey(id: string): string {
  return `${RECORD_PREFIX}${id}`;
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
      addedAt: value.addedAt,
      metadata: value.metadata ?? {},
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

export async function registerStoredPairing(input: string): Promise<StoredPairing> {
  const gatewayUrl = parsePairingInput(input);
  const current = await loadStoredPairings();
  const existing = current.find((record) => record.gatewayUrl === gatewayUrl);
  if (existing) return existing;

  const record: StoredPairing = {
    id: Crypto.randomUUID(),
    gatewayUrl,
    addedAt: new Date().toISOString(),
    metadata: {},
  };
  await saveRecord(record);
  await saveIndex([...current.map((entry) => entry.id), record.id]);
  return record;
}

export async function updateStoredPairingMetadata(
  id: string,
  metadata: PairingMetadata,
): Promise<void> {
  const record = parseRecord(await SecureStore.getItemAsync(recordKey(id)));
  if (!record) return;
  await saveRecord({ ...record, metadata: { ...record.metadata, ...metadata } });
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

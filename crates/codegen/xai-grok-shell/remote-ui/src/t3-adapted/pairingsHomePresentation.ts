import type { StoredPairing } from "../pairingRegistry";

export interface PairingHomeProjectGroup {
  readonly key: string;
  readonly host: string;
  readonly cwd: string;
  readonly title: string;
  readonly pairings: ReadonlyArray<StoredPairing>;
  readonly representative: StoredPairing;
}

function pairingActivityMs(pairing: StoredPairing): number {
  for (const candidate of [pairing.lastSeenAt, pairing.addedAt]) {
    if (!candidate) continue;
    const parsed = Date.parse(candidate);
    if (Number.isFinite(parsed)) return parsed;
  }
  return 0;
}

export function normalizePairingProjectCwd(cwd?: string): string {
  const normalized = (cwd ?? "")
    .trim()
    .replaceAll("\\", "/")
    .replace(/\/{2,}/g, "/");
  if (normalized === "/") return normalized;
  return normalized.replace(/\/+$/, "");
}

export function pairingProjectHost(baseUrl: string): string {
  try {
    return new URL(baseUrl).host.trim().toLocaleLowerCase();
  } catch {
    return "unknown-host";
  }
}

export function pairingProjectTitle(cwd: string): string {
  const parts = cwd.split("/").filter(Boolean);
  return parts.at(-1) || "Forge sessions";
}

/** Bearer-free identity for one host directory. */
export function pairingHomeProjectKey(pairing: StoredPairing): string {
  const host = pairingProjectHost(pairing.baseUrl);
  const cwd = normalizePairingProjectCwd(pairing.cwd).toLocaleLowerCase();
  return JSON.stringify([host, cwd]);
}

export function buildPairingHomeProjectGroups(
  pairings: ReadonlyArray<StoredPairing>,
): PairingHomeProjectGroup[] {
  const buckets = new Map<
    string,
    { host: string; cwd: string; pairings: StoredPairing[]; insertionIndex: number }
  >();

  for (const [insertionIndex, pairing] of pairings.entries()) {
    const key = pairingHomeProjectKey(pairing);
    const existing = buckets.get(key);
    if (existing) {
      existing.pairings.push(pairing);
      continue;
    }
    buckets.set(key, {
      host: pairingProjectHost(pairing.baseUrl),
      cwd: normalizePairingProjectCwd(pairing.cwd),
      pairings: [pairing],
      insertionIndex,
    });
  }

  return [...buckets.entries()]
    .map(([key, bucket]) => {
      const groupedPairings = bucket.pairings
        .map((pairing, insertionIndex) => ({ pairing, insertionIndex }))
        .sort(
          (left, right) =>
            pairingActivityMs(right.pairing) - pairingActivityMs(left.pairing) ||
            left.insertionIndex - right.insertionIndex,
        )
        .map(({ pairing }) => pairing);
      const representative = groupedPairings[0];
      if (!representative) throw new Error("A pairing project group cannot be empty");
      const representativeCwd = normalizePairingProjectCwd(representative.cwd) || bucket.cwd;
      return {
        key,
        host: bucket.host,
        cwd: representativeCwd,
        title: pairingProjectTitle(representativeCwd),
        pairings: groupedPairings,
        representative,
        insertionIndex: bucket.insertionIndex,
      };
    })
    .sort(
      (left, right) =>
        pairingActivityMs(right.representative) - pairingActivityMs(left.representative) ||
        left.insertionIndex - right.insertionIndex,
    )
    .map(({ insertionIndex: _insertionIndex, ...group }) => group);
}

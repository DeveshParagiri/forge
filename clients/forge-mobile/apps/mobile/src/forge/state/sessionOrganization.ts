import type { PairingSummary } from "./pairingStore";

export function comparePairingSummaries(left: PairingSummary, right: PairingSummary): number {
  const leftKey = left.metadata.pinOrderKey;
  const rightKey = right.metadata.pinOrderKey;
  if (leftKey !== undefined && rightKey === undefined) return -1;
  if (leftKey === undefined && rightKey !== undefined) return 1;
  if (leftKey !== undefined && rightKey !== undefined && leftKey !== rightKey) {
    return leftKey.localeCompare(rightKey);
  }
  return right.addedAt.localeCompare(left.addedAt) || left.id.localeCompare(right.id);
}

export function shouldAttachStoredPairing(pairing: PairingSummary): boolean {
  return pairing.metadata.archivedAt === undefined;
}

export interface PendingNavigationTarget {
  pairingId: string | null;
}

/** Opens immediately when navigation is ready, otherwise keeps the latest cold-start target. */
export function openOrQueuePairing(
  pending: PendingNavigationTarget,
  pairingId: string,
  isReady: () => boolean,
  navigate: (pairingId: string) => void,
): void {
  if (isReady()) {
    pending.pairingId = null;
    navigate(pairingId);
    return;
  }
  pending.pairingId = pairingId;
}

export function drainQueuedPairing(
  pending: PendingNavigationTarget,
  navigate: (pairingId: string) => void,
): void {
  const pairingId = pending.pairingId;
  if (!pairingId) return;
  pending.pairingId = null;
  navigate(pairingId);
}

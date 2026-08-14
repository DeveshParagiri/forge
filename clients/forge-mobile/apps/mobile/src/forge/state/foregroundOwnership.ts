import type { AppStateStatus } from "react-native";

export interface ForegroundOwnedRemoteSocket {
  readonly resume: () => void;
  readonly suspend: () => void;
}

export function isForgeAppForeground(state: AppStateStatus): boolean {
  return state === "active";
}

/** Only the pairing whose screen is selected may own its gateway. */
export function syncSelectedForegroundOwnership(
  sockets: Iterable<readonly [string, ForegroundOwnedRemoteSocket]>,
  selectedPairingId: string | null,
  state: AppStateStatus,
): void {
  const foreground = isForgeAppForeground(state);
  for (const [pairingId, socket] of sockets) {
    if (foreground && pairingId === selectedPairingId) socket.resume();
    else socket.suspend();
  }
}

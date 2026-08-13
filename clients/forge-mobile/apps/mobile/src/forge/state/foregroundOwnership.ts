import type { AppStateStatus } from "react-native";

export interface ForegroundOwnedRemoteSocket {
  readonly resume: () => void;
  readonly suspend: () => void;
}

export function isForgeAppForeground(state: AppStateStatus): boolean {
  return state === "active";
}

export function syncForegroundOwnership(
  sockets: Iterable<ForegroundOwnedRemoteSocket>,
  state: AppStateStatus,
): void {
  const action = isForgeAppForeground(state) ? "resume" : "suspend";
  for (const socket of sockets) socket[action]();
}

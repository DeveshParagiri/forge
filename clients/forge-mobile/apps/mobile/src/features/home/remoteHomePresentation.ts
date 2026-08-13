export interface RemoteHomeEmptyState {
  readonly title: string;
  readonly detail: string;
  readonly loading: false;
}

/** Remote Home leaves connection state exclusively to its header dot. */
export function remoteHomeEmptyState(hasPairings: boolean): RemoteHomeEmptyState {
  return hasPairings
    ? {
        title: "No sessions yet",
        detail: "Your paired Forge sessions will appear here.",
        loading: false,
      }
    : {
        title: "No paired sessions",
        detail: "Pair a Forge session to see it here.",
        loading: false,
      };
}

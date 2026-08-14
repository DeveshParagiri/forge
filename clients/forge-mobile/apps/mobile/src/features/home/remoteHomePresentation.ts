import { normalizeProjectPathForComparison } from "@t3tools/shared/path";

import type { HomeGroupDisplayState } from "./homeListItems";

export const REMOTE_HOME_DISCLOSURE_DURATION_MS = 180;

export interface RemoteHomeNewSessionActionPresentation {
  readonly accessibilityLabel: string;
  readonly disabled: boolean;
  readonly systemImage: "square.and.pencil";
}

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

/** Stable, bearer-free identity for one Mac directory on Remote Home. */
export function remoteHomeProjectGroupKey(host: string, cwd: string): string {
  const normalizedHost = host.trim().toLocaleLowerCase();
  // Forge Remote targets macOS hosts, whose default filesystems compare path
  // names case-insensitively. Keep host identity in the key so two different
  // Macs with the same directory spelling never collapse into one project.
  const normalizedCwd = normalizeProjectPathForComparison(cwd).toLocaleLowerCase();
  return `remote-project:${normalizedHost}:${normalizedCwd}`;
}

export function remoteHomeNewSessionActionPresentation(input: {
  readonly projectTitle: string;
  readonly pending: boolean;
}): RemoteHomeNewSessionActionPresentation {
  return {
    accessibilityLabel: input.pending
      ? `Creating new session in ${input.projectTitle}`
      : `Create new session in ${input.projectTitle}`,
    disabled: input.pending,
    systemImage: "square.and.pencil",
  };
}

export function shouldAnimateRemoteHomeGroupAction(input: {
  readonly action: "toggle-collapsed" | "show-more" | "show-less";
  readonly reduceMotionEnabled: boolean;
  readonly remoteOnly: boolean;
}): boolean {
  return input.remoteOnly && input.action === "toggle-collapsed" && !input.reduceMotionEnabled;
}

// Remote Home is a stack screen, so component state is discarded when a
// session opens. Keep disclosure state for the app process without writing
// directory names to SecureStore or altering retained T3 preferences.
const remoteHomeGroupDisplayStates = new Map<string, HomeGroupDisplayState>();

export function readRemoteHomeGroupDisplayStates(): ReadonlyMap<string, HomeGroupDisplayState> {
  return new Map(remoteHomeGroupDisplayStates);
}

export function writeRemoteHomeGroupDisplayState(key: string, state: HomeGroupDisplayState): void {
  remoteHomeGroupDisplayStates.set(key, state);
}

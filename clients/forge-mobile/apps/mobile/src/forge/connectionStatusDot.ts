import type { RemoteConnectionPhase } from "./protocol/remoteSocket";

export interface ForgeConnectionStatusDotPresentation {
  readonly color: "#34C759" | "#FF3B30";
  readonly message: null;
}

export const FORGE_SESSION_HEADER_STATUS_GEOMETRY = {
  diameter: 8,
  radius: 4,
  titleGap: 6,
} as const;

/** Forge Remote deliberately presents connection state without status copy. */
export function forgeConnectionStatusDot(
  phase: RemoteConnectionPhase,
): ForgeConnectionStatusDotPresentation {
  return {
    color: phase === "connected" ? "#34C759" : "#FF3B30",
    message: null,
  };
}

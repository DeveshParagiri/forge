import type { RemoteConnectionPhase } from "./protocol/remoteSocket";

export interface ForgeConnectionStatusDotPresentation {
  readonly color: "#34C759" | "#FF3B30";
  readonly message: null;
}

/** Forge Remote deliberately presents connection state without status copy. */
export function forgeConnectionStatusDot(
  phase: RemoteConnectionPhase,
): ForgeConnectionStatusDotPresentation {
  return {
    color: phase === "connected" ? "#34C759" : "#FF3B30",
    message: null,
  };
}

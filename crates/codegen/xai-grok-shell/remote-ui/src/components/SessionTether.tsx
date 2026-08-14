import ChevronLeft from "lucide-react/dist/esm/icons/chevron-left.js";
import SquarePen from "lucide-react/dist/esm/icons/square-pen.js";
import type { ConnectionPhase } from "../reducer";
import type { RemoteSessionSnapshot } from "../protocol";

export function SessionTether({
  phase,
  session,
  onBack,
  onCreateSession,
}: {
  phase: ConnectionPhase;
  session?: RemoteSessionSnapshot;
  onBack?: () => void;
  onCreateSession?: () => void;
}) {
  const connectionState = phase === "live" ? "connected" : "disconnected";
  return (
    <header
      className="session-header remote-session-header"
      data-connection-phase={phase}
      data-connection-state={connectionState}
    >
      {onBack ? (
        <button type="button" className="detail-back" aria-label="Back to sessions" onClick={onBack}>
          <ChevronLeft aria-hidden="true" />
        </button>
      ) : <span className="detail-back-spacer" aria-hidden="true" />}
      <div className="session-identity">
        <span className="session-title remote-session-title">
          <span className="session-title-line">
            <strong>{session?.title || "Forge Remote"}</strong>
            <span
              className="session-connection-dot"
              data-connection-state={connectionState}
              aria-hidden="true"
            />
          </span>
        </span>
      </div>
      {onCreateSession ? (
        <button
          type="button"
          className="detail-new-session"
          aria-label="Create new session in this directory"
          onClick={onCreateSession}
        >
          <SquarePen aria-hidden="true" />
        </button>
      ) : <span className="detail-back-spacer" aria-hidden="true" />}
    </header>
  );
}

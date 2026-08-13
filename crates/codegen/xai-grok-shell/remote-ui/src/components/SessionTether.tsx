import ChevronLeft from "lucide-react/dist/esm/icons/chevron-left.js";
import type { ConnectionPhase } from "../reducer";
import type { RemoteSessionSnapshot } from "../protocol";

function projectLabel(cwd?: string): string | undefined {
  if (!cwd) return undefined;
  return cwd.split(/[\\/]/).filter(Boolean).at(-1);
}

export function SessionTether({
  phase,
  session,
  onBack,
}: {
  phase: ConnectionPhase;
  session?: RemoteSessionSnapshot;
  onBack?: () => void;
}) {
  const subtitle = [projectLabel(session?.cwd), session?.currentModel?.label]
    .filter(Boolean)
    .join(" · ");
  return (
    <header className="session-header" data-connection-phase={phase}>
      {onBack ? (
        <button type="button" className="detail-back" aria-label="Back to sessions" onClick={onBack}>
          <ChevronLeft aria-hidden="true" />
        </button>
      ) : <span className="detail-back-spacer" aria-hidden="true" />}
      <div className="session-identity">
        <span className="session-title">
          <strong>{session?.title || "Forge Remote"}</strong>
          <small>{subtitle || "Forge"}</small>
        </span>
      </div>
      <span className="detail-back-spacer" aria-hidden="true" />
    </header>
  );
}

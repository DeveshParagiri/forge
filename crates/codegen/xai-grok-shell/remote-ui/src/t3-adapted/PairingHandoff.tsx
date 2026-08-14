import Smartphone from "lucide-react/dist/esm/icons/smartphone.js";
import forgeMarkUrl from "../../../../../../clients/forge-mobile/apps/mobile/assets/forge/mark.png";

export function PairingHandoff({
  nativeClaimed,
  onOpenNative,
  onContinueBrowser,
}: {
  nativeClaimed: boolean;
  onOpenNative(): void;
  onContinueBrowser(): void;
}) {
  return (
    <main className="pairing-handoff">
      <div className="forge-wordmark" aria-label="Forge">
        <img src={forgeMarkUrl} alt="" aria-hidden="true" />
        <strong>Forge</strong>
      </div>
      <div className="handoff-mark" aria-hidden="true"><Smartphone /></div>
      <h1>{nativeClaimed ? "Opened in Forge" : "Opening Forge"}</h1>
      <p>{nativeClaimed ? "This private session was handed to the app." : "Use the Forge app, or continue with the same session in this browser."}</p>
      <div className="handoff-actions">
        <button type="button" className="handoff-primary" onClick={onOpenNative}>Open in Forge app</button>
        <button type="button" className="handoff-secondary" onClick={onContinueBrowser}>Continue in browser</button>
      </div>
      <small>Private tailnet connection</small>
    </main>
  );
}

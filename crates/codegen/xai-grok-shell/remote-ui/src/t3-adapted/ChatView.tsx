/*
 * Adapted from T3 Code's apps/web/src/components/ChatView.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import AlertTriangle from "lucide-react/dist/esm/icons/triangle-alert.js";
import ClipboardList from "lucide-react/dist/esm/icons/clipboard-list.js";
import ListOrdered from "lucide-react/dist/esm/icons/list-ordered.js";
import RefreshCw from "lucide-react/dist/esm/icons/refresh-cw.js";
import { useCallback, useState } from "react";
import type { RemoteSessionSnapshot } from "../protocol";
import type { RemoteClientState } from "../reducer";
import type { ForgeRemoteCommands } from "../remoteSocket";
import { InteractionCards } from "../components/InteractionCards";
import { SessionTether } from "../components/SessionTether";
import { ChatComposer } from "./ChatComposer";
import { MessagesTimeline } from "./MessagesTimeline";
import { UsageSheet } from "./UsageSheet";

const USAGE_STALE_AFTER_MS = 60_000;

export function usageNeedsRefresh(session: RemoteSessionSnapshot, now = Date.now()): boolean {
  const usage = session.usage;
  if (!usage || usage.status === "idle" || usage.status === "error") return true;
  if (usage.status === "loading") return false;
  if (!usage.refreshedAt) return true;
  const refreshedAt = Date.parse(usage.refreshedAt);
  return !Number.isFinite(refreshedAt) || now - refreshedAt >= USAGE_STALE_AFTER_MS;
}

function SessionWorkState({ session }: { session: RemoteSessionSnapshot }) {
  const queue = session.queue || [];
  const showPlan = Boolean(session.planMode?.active && session.planMode.plan);
  if (!queue.length && !showPlan) return null;
  return (
    <div className="session-work-state">
      {queue.length ? (
        <details className="queue-strip">
          <summary><ListOrdered aria-hidden="true" />{queue.length} queued message{queue.length === 1 ? "" : "s"}</summary>
          <ol>
            {queue.map((item, index) => (
              <li key={item.id}><span>{item.position ?? index + 1}</span><p>{item.text}</p></li>
            ))}
          </ol>
        </details>
      ) : null}
      {showPlan ? (
        <details className="plan-mode-strip">
          <summary><ClipboardList aria-hidden="true" />Plan mode</summary>
          <p>{session.planMode?.plan}</p>
        </details>
      ) : null}
    </div>
  );
}

function BlockingState({ state }: { state: RemoteClientState }) {
  let title = "Connecting to Forge";
  let description = "Waiting for the authoritative session snapshot before enabling controls.";
  if (state.phase === "conflict") {
    title = "This session is already open";
    description = "Forge Remote allows one browser or Forge app at a time. Close the other client, then reopen this session.";
  } else if (state.phase === "incompatible") {
    title = "Forge Remote needs an update";
    description = state.lastError || "Stop this remote in the terminal and reopen it after updating Forge.";
  } else if (state.phase === "revoked") {
    title = state.revocationReason === "expired" ? "This private link expired" : state.revocationReason === "session_closed" ? "The Forge session closed" : "Remote access was stopped";
    description = "This phone can no longer access the session. Return to the Forge terminal to create a new private link.";
  } else if (state.phase === "closed") {
    title = "The Forge session closed";
    description = "The conversation is no longer accepting remote input. Return to the terminal to continue.";
  }
  return (
    <main className="blocking-state">
      <span className="blocking-mark" aria-hidden="true">F</span>
      {state.phase === "connecting" || state.phase === "syncing" ? <RefreshCw className="status-spin" aria-hidden="true" /> : <AlertTriangle aria-hidden="true" />}
      <h1>{title}</h1>
      <p>{description}</p>
      {state.phase === "connecting" || state.phase === "syncing" ? <div className="snapshot-skeleton" aria-hidden="true"><i /><i /><i /></div> : null}
    </main>
  );
}

function LiveChatView({
  state,
  session,
  commands,
  onBack,
}: {
  state: RemoteClientState;
  session: RemoteSessionSnapshot;
  commands: ForgeRemoteCommands;
  onBack?: () => void;
}) {
  const [usageOpen, setUsageOpen] = useState(false);
  const connected = state.phase === "live";
  const pendingCommand = Object.values(state.pendingCommands)[0];
  const usagePending = Object.values(state.pendingCommands).some((command) => command.type === "refreshUsage");
  const isWorking = session.status === "running";
  const refreshUsage = useCallback(() => {
    if (!connected || !session.capabilities.usage || usagePending) return;
    commands.refreshUsage();
  }, [commands, connected, session.capabilities.usage, usagePending]);
  const openUsage = useCallback(() => {
    if (!session.capabilities.usage) return;
    setUsageOpen(true);
    if (connected && !usagePending && usageNeedsRefresh(session)) commands.refreshUsage();
  }, [commands, connected, session, usagePending]);
  const closeUsage = useCallback(() => setUsageOpen(false), []);
  return (
    <div className="remote-app">
      <SessionTether phase={state.phase} session={session} onBack={onBack} />
      {state.phase === "reconnecting" || state.phase === "resyncing" || state.phase === "passive" ? (
        <div className="connection-banner" role="status"><RefreshCw className={state.phase === "passive" ? undefined : "status-spin"} aria-hidden="true" />{state.phase === "resyncing" ? "Refreshing the complete Forge session…" : state.phase === "passive" ? "Active in the Forge app. Return to this browser to take control here." : "Reconnecting. Work continues in the terminal…"}</div>
      ) : null}
      {state.lastError ? <div className="error-banner" role="alert"><AlertTriangle aria-hidden="true" /><span>{state.lastError}</span></div> : null}
      {pendingCommand ? <div className="command-banner" role="status"><RefreshCw className="status-spin" aria-hidden="true" />{pendingCommand.label}…</div> : null}
      <main className="chat-stage">
        <MessagesTimeline
          items={session.transcript}
          isWorking={isWorking}
          workingLabel={session.taskState?.label}
          revision={state.revision}
        />
        <div className="composer-stack">
          <SessionWorkState session={session} />
          <InteractionCards interactions={session.activeInteractions} commands={commands} disabled={!connected || Boolean(pendingCommand)} />
          <ChatComposer
            session={session}
            commands={commands}
            connected={connected}
            commandPending={Boolean(pendingCommand)}
            usagePending={usagePending}
            onOpenUsage={openUsage}
          />
        </div>
      </main>
      {usageOpen ? (
        <UsageSheet
          usage={session.usage}
          refreshing={usagePending}
          onRefresh={refreshUsage}
          onClose={closeUsage}
        />
      ) : null}
    </div>
  );
}

export function ChatView({ state, commands, onBack }: { state: RemoteClientState; commands: ForgeRemoteCommands; onBack?: () => void }) {
  const session = state.session;
  const terminalPhase = ["revoked", "conflict", "incompatible"].includes(state.phase);
  if (!session || terminalPhase) {
    return (
      <div className="remote-app">
        <SessionTether phase={state.phase} session={session} onBack={onBack} />
        <BlockingState state={state} />
      </div>
    );
  }
  return <LiveChatView state={state} session={session} commands={commands} onBack={onBack} />;
}

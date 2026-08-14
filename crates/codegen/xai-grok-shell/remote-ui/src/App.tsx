import { type FormEvent, useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { beginNativeHandoff, type NativeHandoffController } from "./nativeHandoff";
import {
  canonicalPairingUrl,
  pairingStorageKey,
  pairingIdForSession,
  readPairings,
  registerPairing,
  removePairing,
  stateInvalidatesPairing,
  type StoredPairing,
  updatePairingFromState,
  writePairings,
} from "./pairingRegistry";
import {
  initialRemoteClientState,
  remoteClientReducer,
  type RemoteClientAction,
  type RemoteClientState,
} from "./reducer";
import { bindRemoteSocketVisibility, ForgeRemoteSocket } from "./remoteSocket";
import { ChatView } from "./t3-adapted/ChatView";
import { PairingHandoff } from "./t3-adapted/PairingHandoff";
import { PairingsHome } from "./t3-adapted/PairingsHome";
import type { PairingHomeProjectGroup } from "./t3-adapted/pairingsHomePresentation";
import { bindVisualViewport } from "./visualViewport";
import { useModalFocus } from "./useModalFocus";

function SessionController({
  pairing,
  onBack,
  onState,
  onRevoked,
  onSocket,
  onCreateSession,
}: {
  pairing: StoredPairing;
  onBack(): void;
  onState(state: ReturnType<typeof remoteClientReducer>): void;
  onRevoked(pairing: StoredPairing, state: RemoteClientState): void;
  onSocket(socket: ForgeRemoteSocket | null): void;
  onCreateSession(pairing: StoredPairing, socket: ForgeRemoteSocket): void;
}) {
  const [state, dispatch] = useReducer(remoteClientReducer, initialRemoteClientState);
  const socket = useMemo(() => new ForgeRemoteSocket(dispatch, pairing.baseUrl), [pairing.baseUrl]);

  useEffect(() => {
    onSocket(socket);
    const visibility = bindRemoteSocketVisibility(socket);
    return () => {
      visibility.dispose();
      onSocket(null);
    };
  }, [onSocket, socket]);

  useEffect(() => {
    if (!state.needsResync) return;
    if (socket.resync()) dispatch({ type: "resyncSent" });
  }, [socket, state.needsResync]);

  useEffect(() => {
    if (["revoked", "conflict", "incompatible", "closed"].includes(state.phase)) socket.stop();
  }, [socket, state.phase]);

  useEffect(() => onState(state), [onState, state]);

  useEffect(() => {
    if (stateInvalidatesPairing(state)) onRevoked(pairing, state);
  }, [onRevoked, pairing, state]);

  const newSessionPending = Object.values(state.pendingCommands).some(
    (command) => command.type === "newSession" || command.type === "acceptNewSession",
  );
  const canCreateSession =
    state.phase === "live" &&
    state.session?.capabilities.newSession === true &&
    !newSessionPending;

  return (
    <ChatView
      state={state}
      commands={socket}
      onBack={onBack}
      onCreateSession={canCreateSession ? () => onCreateSession(pairing, socket) : undefined}
    />
  );
}

function initialRegistry() {
  const registration = registerPairing(window.location.href);
  if (registration) {
    return {
      currentPairingId: registration.pairing.id,
      pairings: registration.pairings,
    };
  }
  return { currentPairingId: undefined, pairings: readPairings() };
}

const PAIRING_CONNECT_TIMEOUT_MS = 12_000;

interface ConnectedPairing {
  socket: ForgeRemoteSocket;
  state: RemoteClientState;
}

function terminalPairingError(state: RemoteClientState): string | null {
  if (state.phase === "revoked") return "This private Forge link is no longer valid.";
  if (state.phase === "conflict") return "This session is already open in another remote client.";
  if (state.phase === "incompatible") return "This Forge remote needs a compatible browser build.";
  if (state.phase === "closed") return "This Forge session is closed.";
  return null;
}

function connectPairing(
  baseUrl: string,
  expectedSessionId?: string,
  timeoutMs = PAIRING_CONNECT_TIMEOUT_MS,
): Promise<ConnectedPairing> {
  return new Promise((resolve, reject) => {
    let state = initialRemoteClientState;
    let settled = false;
    let timeout = 0;
    const socket = new ForgeRemoteSocket((action: RemoteClientAction) => {
      state = remoteClientReducer(state, action);
      const terminalError = terminalPairingError(state);
      if (terminalError) {
        finish(new Error(terminalError));
        return;
      }
      if (state.phase !== "live" || !state.session || !state.sessionId) return;
      if (expectedSessionId && state.sessionId !== expectedSessionId) {
        finish(new Error("The new private link opened a different Forge session."));
        return;
      }
      finish(undefined, { socket, state });
    }, baseUrl);
    const finish = (error?: Error, result?: ConnectedPairing) => {
      if (settled) return;
      settled = true;
      window.clearTimeout(timeout);
      if (error || !result) {
        socket.stop();
        reject(error ?? new Error("Forge did not return a usable session."));
      } else {
        resolve(result);
      }
    };
    timeout = window.setTimeout(
      () => finish(new Error("Forge did not connect before the browser timed out.")),
      timeoutMs,
    );
    try {
      socket.connect();
    } catch {
      finish(new Error("The browser could not open this Forge remote."));
    }
  });
}

function AddSessionSheet({
  value,
  error,
  onChange,
  onCancel,
  onSubmit,
}: {
  value: string;
  error?: string;
  onChange(value: string): void;
  onCancel(): void;
  onSubmit(event: FormEvent<HTMLFormElement>): void;
}) {
  const dialogRef = useModalFocus<HTMLElement>(true, onCancel);
  return (
    <div className="add-session-backdrop" role="presentation" onMouseDown={onCancel}>
      <section
        ref={dialogRef}
        className="add-session-sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-session-title"
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <h1 id="add-session-title">Add Forge session</h1>
          <button type="button" aria-label="Close add session" onClick={onCancel}>Close</button>
        </header>
        <form onSubmit={onSubmit}>
          <label htmlFor="forge-private-link">Private /rc link</label>
          <input
            id="forge-private-link"
            type="url"
            inputMode="url"
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            data-autofocus
            value={value}
            placeholder={`${window.location.origin}/forge/…/`}
            onChange={(event) => onChange(event.target.value)}
          />
          <p className="add-session-privacy">
            Paste the private link shown by /rc. It stays in this browser's local storage and grants access to that Forge session, so do not share it.
          </p>
          {error ? <p className="add-session-error" role="alert">{error}</p> : null}
          <button className="add-session-submit" type="submit">Add session</button>
        </form>
      </section>
    </div>
  );
}

export function App() {
  const [{ currentPairingId, pairings: storedPairings }] = useState(initialRegistry);
  const [pairings, setPairings] = useState(storedPairings);
  const [selectedPairingId, setSelectedPairingId] = useState<string | undefined>(currentPairingId);
  const [screen, setScreen] = useState<"home" | "thread">(currentPairingId ? "thread" : "home");
  const [browserStarted, setBrowserStarted] = useState(!currentPairingId);
  const [nativeClaimed, setNativeClaimed] = useState(false);
  const [addSessionOpen, setAddSessionOpen] = useState(false);
  const [addSessionUrl, setAddSessionUrl] = useState("");
  const [addSessionError, setAddSessionError] = useState<string | undefined>();
  const [newSessionBusy, setNewSessionBusy] = useState(false);
  const [newSessionMessage, setNewSessionMessage] = useState<string | undefined>();
  const handoffRef = useRef<NativeHandoffController | null>(null);
  const activeSocketRef = useRef<ForgeRemoteSocket | null>(null);
  const pairingsRef = useRef(pairings);
  const newSessionBusyRef = useRef(false);
  const didAttemptNativeRef = useRef(false);
  const selectedPairing = pairings.find((pairing) => pairing.id === selectedPairingId);

  const persistPairings = useCallback((next: StoredPairing[]) => {
    pairingsRef.current = next;
    writePairings(next);
    setPairings(next);
  }, []);

  useEffect(() => bindVisualViewport(), []);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key !== pairingStorageKey()) return;
      const next = readPairings();
      pairingsRef.current = next;
      setPairings(next);
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  useEffect(() => {
    if (!currentPairingId || browserStarted) return;
    const pairing = pairings.find((candidate) => candidate.id === currentPairingId);
    if (!pairing) return;
    const controller = beginNativeHandoff({
      bearerUrl: pairing.baseUrl,
      attemptOnStart: !didAttemptNativeRef.current,
      beforeNativeAttempt: () => activeSocketRef.current?.relinquish("opening Forge app"),
      onBrowserFallback: () => setBrowserStarted(true),
      onNativeClaimed: () => setNativeClaimed(true),
    });
    didAttemptNativeRef.current = true;
    handoffRef.current = controller;
    return () => {
      controller.dispose();
      if (handoffRef.current === controller) handoffRef.current = null;
    };
  }, [browserStarted, currentPairingId, pairings]);

  const handleSelect = useCallback((pairing: StoredPairing) => {
    setSelectedPairingId(pairing.id);
    setScreen("thread");
    setBrowserStarted(true);
  }, []);

  const handleSocket = useCallback((socket: ForgeRemoteSocket | null) => {
    activeSocketRef.current = socket;
  }, []);

  const handleRemove = useCallback((pairing: StoredPairing) => {
    persistPairings(removePairing(pairingsRef.current, pairing.id));
    if (selectedPairingId === pairing.id) {
      setSelectedPairingId(undefined);
      setScreen("home");
    }
  }, [persistPairings, selectedPairingId]);

  const handleState = useCallback((state: ReturnType<typeof remoteClientReducer>) => {
    if (!selectedPairingId) return;
    const target = pairingsRef.current.find((pairing) => pairing.id === selectedPairingId);
    const next = updatePairingFromState(pairingsRef.current, selectedPairingId, state);
    persistPairings(next);
    if (target && state.sessionId) {
      const stableId = pairingIdForSession(target.baseUrl, state.sessionId);
      if (stableId !== selectedPairingId) setSelectedPairingId(stableId);
    }
  }, [persistPairings, selectedPairingId]);

  const handleRevoked = useCallback((pairing: StoredPairing, state: RemoteClientState) => {
    const revokedIds = new Set<string>([pairing.id]);
    if (selectedPairingId) revokedIds.add(selectedPairingId);
    if (state.sessionId) revokedIds.add(pairingIdForSession(pairing.baseUrl, state.sessionId));
    persistPairings(pairingsRef.current.filter((candidate) => !revokedIds.has(candidate.id)));
    setSelectedPairingId(undefined);
    setScreen("home");
  }, [persistPairings, selectedPairingId]);

  const handleAddSession = useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const registration = registerPairing(addSessionUrl.trim());
    if (!registration) {
      setAddSessionError(
        `Paste a valid private /rc link served by ${window.location.origin}.`,
      );
      return;
    }
    persistPairings(registration.pairings);
    setAddSessionOpen(false);
    setAddSessionUrl("");
    setAddSessionError(undefined);
    handleSelect(registration.pairing);
  }, [addSessionUrl, handleSelect, persistPairings]);

  const handleCreateSession = useCallback(async (
    sourcePairing: StoredPairing,
    connectedSource?: ForgeRemoteSocket,
  ) => {
    if (newSessionBusyRef.current) return;
    newSessionBusyRef.current = true;
    setNewSessionBusy(true);
    setNewSessionMessage("Creating a new Forge session…");
    let ownedSource: ConnectedPairing | undefined;
    let childConnection: ConnectedPairing | undefined;
    let provisionalBaseUrl: string | undefined;
    let provisionalStableId: string | undefined;
    try {
      const sourceSocket = connectedSource ?? (
        ownedSource = await connectPairing(sourcePairing.baseUrl, sourcePairing.sessionId)
      ).socket;
      const creation = sourceSocket.newSession();
      if (!creation) {
        throw new Error("This Forge session cannot create a new session right now.");
      }
      const created = await creation;
      const createdBaseUrl = canonicalPairingUrl(created.pairingUrl, window.location.origin);
      if (!createdBaseUrl) {
        throw new Error("Forge returned a private link that this browser cannot safely save.");
      }
      provisionalStableId = pairingIdForSession(createdBaseUrl, created.sessionId);
      if (
        pairingsRef.current.some(
          (pairing) =>
            pairing.baseUrl === createdBaseUrl || pairing.id === provisionalStableId,
        )
      ) {
        throw new Error("Forge returned a session that is already saved in this browser.");
      }
      const registration = registerPairing(createdBaseUrl);
      if (!registration) {
        throw new Error("Forge returned a private link that this browser cannot safely save.");
      }
      provisionalBaseUrl = registration.pairing.baseUrl;
      persistPairings(registration.pairings);

      childConnection = await connectPairing(
        registration.pairing.baseUrl,
        created.sessionId,
      );
      const validated = updatePairingFromState(
        pairingsRef.current,
        registration.pairing.id,
        childConnection.state,
      );
      persistPairings(validated);
      const acceptance = sourceSocket.acceptNewSession(created.sessionId);
      if (!acceptance) {
        throw new Error("Forge could not finish opening the new session.");
      }
      await acceptance;

      childConnection.socket.stop();
      childConnection = undefined;
      ownedSource?.socket.stop();
      ownedSource = undefined;
      const child = pairingsRef.current.find((pairing) => pairing.id === provisionalStableId);
      if (!child || child.sessionId !== created.sessionId) {
        throw new Error("The validated Forge session disappeared before it could open.");
      }
      setSelectedPairingId(child.id);
      setScreen("thread");
      setBrowserStarted(true);
      setNewSessionMessage(undefined);
    } catch (error) {
      childConnection?.socket.stop();
      ownedSource?.socket.stop();
      if (provisionalBaseUrl || provisionalStableId) {
        const next = pairingsRef.current.filter(
          (pairing) =>
            pairing.baseUrl !== provisionalBaseUrl && pairing.id !== provisionalStableId,
        );
        persistPairings(next);
      }
      setNewSessionMessage(
        error instanceof Error ? error.message : "Forge could not create the new session.",
      );
    } finally {
      newSessionBusyRef.current = false;
      setNewSessionBusy(false);
    }
  }, [persistPairings]);

  const handleCreateSessionInProject = useCallback((group: PairingHomeProjectGroup) => {
    const source = group.representative;
    void handleCreateSession(source);
  }, [handleCreateSession]);

  if (currentPairingId && !browserStarted) {
    return (
      <PairingHandoff
        nativeClaimed={nativeClaimed}
        onOpenNative={() => {
          setNativeClaimed(false);
          void handoffRef.current?.retryNative();
        }}
        onContinueBrowser={() => handoffRef.current?.continueInBrowser()}
      />
    );
  }

  if (screen === "thread" && selectedPairing) {
    return (
      <>
        <SessionController
          key={selectedPairing.id}
          pairing={selectedPairing}
          onBack={() => setScreen("home")}
          onState={handleState}
          onRevoked={handleRevoked}
          onSocket={handleSocket}
          onCreateSession={(pairing, socket) => void handleCreateSession(pairing, socket)}
        />
        {newSessionMessage ? (
          <div
            className="pairing-flow-status"
            role={newSessionBusy ? "status" : "alert"}
            aria-live="polite"
          >
            {newSessionMessage}
          </div>
        ) : null}
      </>
    );
  }

  return (
    <>
      <PairingsHome
        pairings={pairings}
        onSelect={handleSelect}
        onRemove={handleRemove}
        onCreateSession={() => {
          setAddSessionError(undefined);
          setAddSessionOpen(true);
        }}
        onCreateSessionInProject={newSessionBusy ? undefined : handleCreateSessionInProject}
      />
      {newSessionMessage ? (
        <div
          className="pairing-flow-status"
          role={newSessionBusy ? "status" : "alert"}
          aria-live="polite"
        >
          {newSessionMessage}
        </div>
      ) : null}
      {addSessionOpen ? (
        <AddSessionSheet
          value={addSessionUrl}
          error={addSessionError}
          onChange={(value) => {
            setAddSessionUrl(value);
            setAddSessionError(undefined);
          }}
          onCancel={() => {
            setAddSessionOpen(false);
            setAddSessionError(undefined);
          }}
          onSubmit={handleAddSession}
        />
      ) : null}
    </>
  );
}

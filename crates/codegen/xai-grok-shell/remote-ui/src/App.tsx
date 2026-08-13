import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { beginNativeHandoff, type NativeHandoffController } from "./nativeHandoff";
import {
  pairingStorageKey,
  readPairings,
  registerPairing,
  removePairing,
  stateInvalidatesPairing,
  type StoredPairing,
  updatePairingFromState,
  writePairings,
} from "./pairingRegistry";
import { initialRemoteClientState, remoteClientReducer } from "./reducer";
import { bindRemoteSocketVisibility, ForgeRemoteSocket } from "./remoteSocket";
import { ChatView } from "./t3-adapted/ChatView";
import { PairingHandoff } from "./t3-adapted/PairingHandoff";
import { PairingsHome } from "./t3-adapted/PairingsHome";
import { bindVisualViewport } from "./visualViewport";

function SessionController({
  pairing,
  onBack,
  onState,
  onRevoked,
  onSocket,
}: {
  pairing: StoredPairing;
  onBack(): void;
  onState(state: ReturnType<typeof remoteClientReducer>): void;
  onRevoked(): void;
  onSocket(socket: ForgeRemoteSocket | null): void;
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
    if (stateInvalidatesPairing(state)) onRevoked();
  }, [onRevoked, state]);

  return <ChatView state={state} commands={socket} onBack={onBack} />;
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

export function App() {
  const [{ currentPairingId, pairings: storedPairings }] = useState(initialRegistry);
  const [pairings, setPairings] = useState(storedPairings);
  const [selectedPairingId, setSelectedPairingId] = useState<string | undefined>(currentPairingId);
  const [screen, setScreen] = useState<"home" | "thread">(currentPairingId ? "thread" : "home");
  const [browserStarted, setBrowserStarted] = useState(!currentPairingId);
  const [nativeClaimed, setNativeClaimed] = useState(false);
  const handoffRef = useRef<NativeHandoffController | null>(null);
  const activeSocketRef = useRef<ForgeRemoteSocket | null>(null);
  const didAttemptNativeRef = useRef(false);
  const selectedPairing = pairings.find((pairing) => pairing.id === selectedPairingId);

  useEffect(() => bindVisualViewport(), []);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === pairingStorageKey()) setPairings(readPairings());
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
    setPairings((current) => {
      const next = removePairing(current, pairing.id);
      writePairings(next);
      return next;
    });
    if (selectedPairingId === pairing.id) {
      setSelectedPairingId(undefined);
      setScreen("home");
    }
  }, [selectedPairingId]);

  const handleState = useCallback((state: ReturnType<typeof remoteClientReducer>) => {
    if (!selectedPairingId) return;
    setPairings((current) => {
      const next = updatePairingFromState(current, selectedPairingId, state);
      writePairings(next);
      return next;
    });
  }, [selectedPairingId]);

  const handleRevoked = useCallback(() => {
    if (!selectedPairingId) return;
    setPairings((current) => {
      const next = removePairing(current, selectedPairingId);
      writePairings(next);
      return next;
    });
    setSelectedPairingId(undefined);
    setScreen("home");
  }, [selectedPairingId]);

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
      <SessionController
        key={selectedPairing.id}
        pairing={selectedPairing}
        onBack={() => setScreen("home")}
        onState={handleState}
        onRevoked={handleRevoked}
        onSocket={handleSocket}
      />
    );
  }

  return <PairingsHome pairings={pairings} onSelect={handleSelect} onRemove={handleRemove} />;
}

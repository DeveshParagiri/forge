import {
  createContext,
  use,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { AppState, type AppStateStatus } from "react-native";

import type { InteractionResponse, RemoteSessionSnapshot } from "../protocol/protocol";
import {
  ForgeRemoteSocket,
  type ForgeRemoteSocketState,
  type RemoteConnectionPhase,
} from "../protocol/remoteSocket";
import {
  loadStoredPairings,
  registerStoredPairing,
  removeStoredPairing,
  summarizePairing,
  updateStoredPairingMetadata,
  type PairingSummary,
  type StoredPairing,
} from "./pairingStore";
import { isForgeAppForeground, syncForegroundOwnership } from "./foregroundOwnership";

export interface ForgeSessionView {
  readonly pairing: PairingSummary;
  readonly connectionPhase: RemoteConnectionPhase;
  readonly connectionError: string | null;
  readonly snapshot: RemoteSessionSnapshot | null;
  readonly revision: number | null;
  readonly expiresAt: string | null;
  readonly pendingInteractionIds: ReadonlyArray<string>;
  readonly modelCommandPending: boolean;
  readonly usageCommandPending: boolean;
}

interface ForgeSessionsContextValue {
  readonly ready: boolean;
  readonly sessions: ReadonlyArray<ForgeSessionView>;
  readonly registerPairing: (input: string) => Promise<string>;
  readonly reconnect: (pairingId: string) => void;
  readonly sendPrompt: (pairingId: string, text: string) => string | null;
  readonly cancel: (pairingId: string) => string | null;
  readonly askBtw: (pairingId: string, question: string) => string | null;
  readonly refreshUsage: (pairingId: string) => string | null;
  readonly setModel: (
    pairingId: string,
    modelId: string,
    reasoningEffort: string | null,
  ) => string | null;
  readonly resolveInteraction: (
    pairingId: string,
    interactionId: string,
    response: InteractionResponse,
  ) => string | null;
}

const ForgeSessionsContext = createContext<ForgeSessionsContextValue | null>(null);

function initialSession(record: StoredPairing): ForgeSessionView {
  return {
    pairing: summarizePairing(record),
    connectionPhase: "connecting",
    connectionError: null,
    snapshot: null,
    revision: null,
    expiresAt: record.metadata.expiresAt ?? null,
    pendingInteractionIds: [],
    modelCommandPending: false,
    usageCommandPending: false,
  };
}

export function ForgeSessionsProvider(props: { readonly children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [sessionById, setSessionById] = useState<ReadonlyMap<string, ForgeSessionView>>(
    () => new Map(),
  );
  const socketById = useRef(new Map<string, ForgeRemoteSocket>());
  const metadataTimerById = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const appState = useRef<AppStateStatus>(AppState.currentState);

  const removeRevokedPairing = useCallback((pairingId: string) => {
    socketById.current.get(pairingId)?.stop();
    socketById.current.delete(pairingId);
    const timer = metadataTimerById.current.get(pairingId);
    if (timer) clearTimeout(timer);
    metadataTimerById.current.delete(pairingId);
    setSessionById((current) => {
      const next = new Map(current);
      next.delete(pairingId);
      return next;
    });
    void removeStoredPairing(pairingId);
  }, []);

  const attachPairing = useCallback(
    (record: StoredPairing) => {
      const existingSocket = socketById.current.get(record.id);
      if (existingSocket) {
        if (isForgeAppForeground(appState.current)) existingSocket.resume();
        return;
      }
      setSessionById((current) => {
        const next = new Map(current);
        next.set(record.id, current.get(record.id) ?? initialSession(record));
        return next;
      });
      const socket = new ForgeRemoteSocket(record.gatewayUrl, {
        onChange: (state: ForgeRemoteSocketState) => {
          setSessionById((current) => {
            const previous = current.get(record.id) ?? initialSession(record);
            const next = new Map(current);
            next.set(record.id, {
              ...previous,
              connectionPhase: state.phase,
              connectionError: state.error,
              snapshot: state.snapshot,
              revision: state.revision,
              expiresAt: state.expiresAt,
              pendingInteractionIds: state.pendingInteractionIds,
              modelCommandPending: state.modelCommandPending,
              usageCommandPending: state.usageCommandPending,
              pairing: {
                ...previous.pairing,
                metadata: {
                  ...previous.pairing.metadata,
                  ...(state.snapshot?.title ? { title: state.snapshot.title } : {}),
                  ...(state.snapshot ? { status: state.snapshot.status } : {}),
                  ...(state.expiresAt ? { expiresAt: state.expiresAt } : {}),
                  ...(state.snapshot ? { lastSeenAt: new Date().toISOString() } : {}),
                },
              },
            });
            return next;
          });
          const existingTimer = metadataTimerById.current.get(record.id);
          if (existingTimer) clearTimeout(existingTimer);
          metadataTimerById.current.set(
            record.id,
            setTimeout(() => {
              metadataTimerById.current.delete(record.id);
              setSessionById((current) => {
                const session = current.get(record.id);
                if (session) {
                  void updateStoredPairingMetadata(record.id, session.pairing.metadata);
                }
                return current;
              });
            }, 1_000),
          );
        },
        onRevoked: () => removeRevokedPairing(record.id),
      });
      socketById.current.set(record.id, socket);
      syncForegroundOwnership([socket], appState.current);
    },
    [removeRevokedPairing],
  );

  useEffect(() => {
    let cancelled = false;
    const appStateSubscription = AppState.addEventListener("change", (nextState) => {
      if (nextState === appState.current) return;
      appState.current = nextState;
      syncForegroundOwnership(socketById.current.values(), nextState);
    });
    void loadStoredPairings().then((records) => {
      if (cancelled) return;
      for (const record of records) attachPairing(record);
      setReady(true);
    });
    return () => {
      cancelled = true;
      appStateSubscription.remove();
      for (const socket of socketById.current.values()) socket.stop();
      socketById.current.clear();
      for (const timer of metadataTimerById.current.values()) clearTimeout(timer);
      metadataTimerById.current.clear();
    };
  }, [attachPairing]);

  const registerPairing = useCallback(
    async (input: string) => {
      const record = await registerStoredPairing(input);
      attachPairing(record);
      return record.id;
    },
    [attachPairing],
  );

  const socket = useCallback((pairingId: string) => socketById.current.get(pairingId), []);
  const value = useMemo<ForgeSessionsContextValue>(
    () => ({
      ready,
      sessions: [...sessionById.values()].sort((left, right) =>
        right.pairing.addedAt.localeCompare(left.pairing.addedAt),
      ),
      registerPairing,
      reconnect: (pairingId) => socket(pairingId)?.resume(),
      sendPrompt: (pairingId, text) => socket(pairingId)?.sendPrompt(text) ?? null,
      cancel: (pairingId) => socket(pairingId)?.cancel() ?? null,
      askBtw: (pairingId, question) => socket(pairingId)?.askBtw(question) ?? null,
      refreshUsage: (pairingId) => socket(pairingId)?.refreshUsage() ?? null,
      setModel: (pairingId, modelId, reasoningEffort) =>
        socket(pairingId)?.setModel(modelId, reasoningEffort) ?? null,
      resolveInteraction: (pairingId, interactionId, response) =>
        socket(pairingId)?.resolveInteraction(interactionId, response) ?? null,
    }),
    [ready, registerPairing, sessionById, socket],
  );

  return (
    <ForgeSessionsContext.Provider value={value}>{props.children}</ForgeSessionsContext.Provider>
  );
}

export function useForgeSessions(): ForgeSessionsContextValue {
  const value = use(ForgeSessionsContext);
  if (!value) throw new Error("useForgeSessions must run inside ForgeSessionsProvider");
  return value;
}

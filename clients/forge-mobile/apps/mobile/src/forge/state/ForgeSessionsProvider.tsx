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

import type {
  InteractionResponse,
  RemoteSessionCreated,
  RemoteSessionSnapshot,
} from "../protocol/protocol";
import { pairingDisplayHost } from "../protocol/pairing";
import {
  ForgeRemoteSocket,
  type ForgeRemoteSocketState,
  type RemoteConnectionPhase,
} from "../protocol/remoteSocket";
import {
  bindStoredPairingSessionIdentity,
  deduplicateStoredPairingSessionIdentity,
  finalizeStoredPairing,
  loadStoredPairings,
  normalizeSessionAlias,
  reconcileStoredPairingSessionIdentity,
  registerStoredPairingWithStatus,
  removeStoredPairing,
  summarizePairing,
  updateStoredPairingMetadata,
  type PairingSummary,
  type StoredPairing,
} from "./pairingStore";
import { comparePairingSummaries, shouldAttachStoredPairing } from "./sessionOrganization";
import { syncSelectedForegroundOwnership } from "./foregroundOwnership";

export interface ForgeSessionView {
  readonly pairing: PairingSummary;
  readonly connectionPhase: RemoteConnectionPhase;
  readonly connectionError: string | null;
  readonly snapshot: RemoteSessionSnapshot | null;
  readonly revision: number | null;
  readonly expiresAt: string | null;
  readonly pendingInteractionIds: ReadonlyArray<string>;
  readonly modelCommandPending: boolean;
  readonly fastModeCommandPending: boolean;
  readonly usageCommandPending: boolean;
  readonly newSessionCommandPending: boolean;
  readonly pendingQueueItemIds: ReadonlyArray<string>;
}

interface ForgeSessionsContextValue {
  readonly ready: boolean;
  readonly sessions: ReadonlyArray<ForgeSessionView>;
  readonly registerPairing: (input: string, expectedSessionId?: string) => Promise<string>;
  readonly reconnect: (pairingId: string) => void;
  readonly releaseActiveSession: (pairingId?: string) => void;
  readonly renameSession: (pairingId: string, title: string | null) => Promise<boolean>;
  readonly pinSession: (pairingId: string) => Promise<boolean>;
  readonly unpinSession: (pairingId: string) => Promise<boolean>;
  readonly archiveSession: (pairingId: string) => Promise<boolean>;
  readonly sendPrompt: (
    pairingId: string,
    text: string,
    images?: ReadonlyArray<{
      readonly name: string;
      readonly mimeType: string;
      readonly data: string;
    }>,
  ) => string | null;
  readonly cancel: (pairingId: string) => string | null;
  readonly askBtw: (pairingId: string, question: string) => string | null;
  readonly refreshUsage: (pairingId: string) => string | null;
  readonly setModel: (
    pairingId: string,
    modelId: string,
    reasoningEffort: string | null,
  ) => string | null;
  readonly setFastMode: (pairingId: string, enabled: boolean) => string | null;
  readonly editQueuedPrompt: (
    pairingId: string,
    queueItemId: string,
    expectedVersion: number,
    text: string,
  ) => string | null;
  readonly steerQueuedPrompt: (
    pairingId: string,
    queueItemId: string,
    expectedVersion: number,
  ) => string | null;
  readonly cancelQueuedPrompt: (
    pairingId: string,
    queueItemId: string,
    expectedVersion: number,
  ) => string | null;
  readonly newSession: (pairingId: string) => Promise<RemoteSessionCreated> | null;
  readonly completeNewSession: (
    sourcePairingId: string,
    childPairingId: string,
    sessionId: string,
  ) => Promise<boolean>;
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
    fastModeCommandPending: false,
    usageCommandPending: false,
    newSessionCommandPending: false,
    pendingQueueItemIds: [],
  };
}

interface PairingRegistrationValidation {
  readonly expectedSessionId?: string;
  readonly resolve: (pairingId: string) => void;
  readonly reject: (error: Error) => void;
  readonly timeout: ReturnType<typeof setTimeout>;
}

const PAIRING_VALIDATION_TIMEOUT_MS = 10_000;

export function ForgeSessionsProvider(props: { readonly children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [sessionById, setSessionById] = useState<ReadonlyMap<string, ForgeSessionView>>(
    () => new Map(),
  );
  const socketById = useRef(new Map<string, ForgeRemoteSocket>());
  const metadataTimerById = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const recoveryTimerById = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const validationById = useRef(new Map<string, PairingRegistrationValidation>());
  const identityUpdateById = useRef(new Set<string>());
  const sessionIdByPairingId = useRef(new Map<string, string>());
  const handleSocketIdentity = useRef<
    (record: StoredPairing, state: ForgeRemoteSocketState) => void
  >(() => undefined);
  const appState = useRef<AppStateStatus>(AppState.currentState);
  const activePairingId = useRef<string | null>(null);

  const detachPairing = useCallback((pairingId: string) => {
    const validation = validationById.current.get(pairingId);
    if (validation) {
      clearTimeout(validation.timeout);
      validationById.current.delete(pairingId);
      validation.reject(new Error("The new Forge pairing was detached before validation."));
    }
    socketById.current.get(pairingId)?.stop();
    socketById.current.delete(pairingId);
    sessionIdByPairingId.current.delete(pairingId);
    const timer = metadataTimerById.current.get(pairingId);
    if (timer) clearTimeout(timer);
    metadataTimerById.current.delete(pairingId);
    const recoveryTimer = recoveryTimerById.current.get(pairingId);
    if (recoveryTimer) clearTimeout(recoveryTimer);
    recoveryTimerById.current.delete(pairingId);
    if (activePairingId.current === pairingId) activePairingId.current = null;
    setSessionById((current) => {
      const next = new Map(current);
      next.delete(pairingId);
      return next;
    });
  }, []);

  const removeRevokedPairing = useCallback(
    (pairingId: string) => {
      detachPairing(pairingId);
      void removeStoredPairing(pairingId);
    },
    [detachPairing],
  );

  const attachPairing = useCallback(
    (record: StoredPairing) => {
      const existingSocket = socketById.current.get(record.id);
      if (existingSocket) {
        return;
      }
      setSessionById((current) => {
        const next = new Map(current);
        next.set(record.id, current.get(record.id) ?? initialSession(record));
        return next;
      });
      const socket = new ForgeRemoteSocket(
        record.gatewayUrl,
        {
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
                fastModeCommandPending: state.fastModeCommandPending,
                usageCommandPending: state.usageCommandPending,
                newSessionCommandPending: state.newSessionCommandPending,
                pendingQueueItemIds: state.pendingQueueItemIds,
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
            handleSocketIdentity.current(record, state);
            const existingTimer = metadataTimerById.current.get(record.id);
            if (existingTimer) clearTimeout(existingTimer);
            metadataTimerById.current.set(
              record.id,
              setTimeout(() => {
                metadataTimerById.current.delete(record.id);
                setSessionById((current) => {
                  const session = current.get(record.id);
                  if (session) {
                    const { expiresAt, lastSeenAt, status, title } = session.pairing.metadata;
                    void updateStoredPairingMetadata(record.id, {
                      expiresAt,
                      lastSeenAt,
                      status,
                      title,
                    });
                  }
                  return current;
                });
              }, 1_000),
            );
          },
          onRevoked: () => {
            const validation = validationById.current.get(record.id);
            if (validation) {
              clearTimeout(validation.timeout);
              validationById.current.delete(record.id);
              validation.reject(new Error("The new Forge pairing was revoked before validation."));
            }
            removeRevokedPairing(record.id);
          },
        },
        record.sessionId,
      );
      socketById.current.set(record.id, socket);
    },
    [removeRevokedPairing],
  );

  const validateAttachedPairing = useCallback(
    async (
      record: StoredPairing,
      expectedSessionId: string | undefined,
      keepConnected: boolean,
    ) => {
      const validation = new Promise<string>((resolve, reject) => {
        const timeout = setTimeout(() => {
          validationById.current.delete(record.id);
          reject(new Error("The Forge pairing did not validate in time."));
        }, PAIRING_VALIDATION_TIMEOUT_MS);
        validationById.current.set(record.id, {
          ...(expectedSessionId ? { expectedSessionId } : {}),
          resolve,
          reject,
          timeout,
        });
      });
      attachPairing(record);
      socketById.current.get(record.id)?.resume();
      try {
        const pairingId = await validation;
        if (!keepConnected) socketById.current.get(pairingId)?.suspend();
        return pairingId;
      } catch (error) {
        detachPairing(record.id);
        await removeStoredPairing(record.id);
        throw error;
      }
    },
    [attachPairing, detachPairing],
  );

  handleSocketIdentity.current = (record, state) => {
    const validation = validationById.current.get(record.id);
    if (validation && state.phase === "error") {
      clearTimeout(validation.timeout);
      validationById.current.delete(record.id);
      validation.reject(new Error(state.error ?? "The Forge pairing could not connect."));
      return;
    }
    if (
      validation?.expectedSessionId &&
      state.snapshot &&
      state.snapshot.sessionId !== validation.expectedSessionId
    ) {
      clearTimeout(validation.timeout);
      validationById.current.delete(record.id);
      validation.reject(new Error("The new Forge pairing opened a different session."));
      return;
    }
    if (state.phase !== "connected" || !state.snapshot) return;

    const sessionId = state.snapshot.sessionId;
    if (!validation && sessionIdByPairingId.current.get(record.id) === sessionId) return;
    if (identityUpdateById.current.has(record.id)) return;
    identityUpdateById.current.add(record.id);

    void (async () => {
      if (validation?.expectedSessionId) {
        const recordHost = pairingDisplayHost(record.gatewayUrl);
        const collision = (await loadStoredPairings()).find(
          (candidate) =>
            candidate.id !== record.id &&
            candidate.sessionId === validation.expectedSessionId &&
            pairingDisplayHost(candidate.gatewayUrl) === recordHost,
        );
        if (collision) throw new Error("Forge did not return a fresh child session.");
        const bound = await bindStoredPairingSessionIdentity(record.id, sessionId);
        return { record: bound };
      }
      if (validation) {
        return reconcileStoredPairingSessionIdentity(record.id, sessionId);
      }
      return deduplicateStoredPairingSessionIdentity(record.id, sessionId);
    })()
      .then((identity) => {
        if (validation && validationById.current.get(record.id) !== validation) return;
        if (validation) {
          clearTimeout(validation.timeout);
          validationById.current.delete(record.id);
        }

        const pairingId = identity.record.id;
        for (const removedPairingId of identity.removedPairingIds ?? []) {
          if (removedPairingId !== record.id && removedPairingId !== pairingId) {
            detachPairing(removedPairingId);
          }
        }
        if (pairingId !== record.id) {
          detachPairing(record.id);
          if (socketById.current.has(pairingId)) detachPairing(pairingId);
          attachPairing(identity.record);
          socketById.current.get(pairingId)?.resume();
        }
        sessionIdByPairingId.current.set(pairingId, sessionId);
        validation?.resolve(pairingId);
      })
      .catch((error: unknown) => {
        if (!validation || validationById.current.get(record.id) !== validation) return;
        clearTimeout(validation.timeout);
        validationById.current.delete(record.id);
        validation.reject(
          error instanceof Error ? error : new Error("The Forge pairing could not be saved."),
        );
      })
      .finally(() => identityUpdateById.current.delete(record.id));
  };

  useEffect(() => {
    let cancelled = false;
    const appStateSubscription = AppState.addEventListener("change", (nextState) => {
      if (nextState === appState.current) return;
      appState.current = nextState;
      syncSelectedForegroundOwnership(
        socketById.current.entries(),
        activePairingId.current,
        nextState,
      );
    });
    void loadStoredPairings().then((records) => {
      if (cancelled) return;
      for (const record of records) {
        if (!shouldAttachStoredPairing(summarizePairing(record))) continue;
        if (record.provisionalUntil && record.sessionId) {
          const resumeAfter = Date.parse(record.provisionalUntil);
          const delay = Number.isFinite(resumeAfter) ? Math.max(0, resumeAfter - Date.now()) : 0;
          const timer = setTimeout(() => {
            recoveryTimerById.current.delete(record.id);
            void validateAttachedPairing(record, record.sessionId!, false)
              .then(() => finalizeStoredPairing(record.id))
              .catch(() => undefined);
          }, delay);
          recoveryTimerById.current.set(record.id, timer);
        } else {
          attachPairing(record);
        }
      }
      setReady(true);
    });
    return () => {
      cancelled = true;
      appStateSubscription.remove();
      for (const socket of socketById.current.values()) socket.stop();
      socketById.current.clear();
      for (const timer of metadataTimerById.current.values()) clearTimeout(timer);
      metadataTimerById.current.clear();
      for (const timer of recoveryTimerById.current.values()) clearTimeout(timer);
      recoveryTimerById.current.clear();
      for (const validation of validationById.current.values()) {
        clearTimeout(validation.timeout);
        validation.reject(new Error("Forge closed before the new pairing was validated."));
      }
      validationById.current.clear();
    };
  }, [attachPairing, validateAttachedPairing]);

  const registerPairing = useCallback(
    async (input: string, expectedSessionId?: string) => {
      const registration = await registerStoredPairingWithStatus(input, expectedSessionId);
      const { record } = registration;
      if (!registration.created) {
        attachPairing(record);
        return record.id;
      }
      return validateAttachedPairing(record, expectedSessionId, true);
    },
    [attachPairing, validateAttachedPairing],
  );

  const socket = useCallback((pairingId: string) => socketById.current.get(pairingId), []);
  const reconnect = useCallback((pairingId: string) => {
    activePairingId.current = pairingId;
    syncSelectedForegroundOwnership(socketById.current.entries(), pairingId, appState.current);
  }, []);
  const releaseActiveSession = useCallback((pairingId?: string) => {
    if (pairingId && activePairingId.current !== pairingId) return;
    activePairingId.current = null;
    syncSelectedForegroundOwnership(socketById.current.entries(), null, appState.current);
  }, []);
  const completeNewSession = useCallback(
    async (sourcePairingId: string, childPairingId: string, sessionId: string) => {
      const acceptance = socketById.current.get(sourcePairingId)?.acceptNewSession(sessionId);
      if (!acceptance) {
        detachPairing(childPairingId);
        await removeStoredPairing(childPairingId);
        return false;
      }
      try {
        await acceptance;
        // The route is committed only after the source socket acknowledges.
        // If clearing the provisional marker transiently fails, keep the
        // connected child: startup recovery will validate and finalize it.
        await finalizeStoredPairing(childPairingId).catch(() => undefined);
        return true;
      } catch {
        detachPairing(childPairingId);
        await removeStoredPairing(childPairingId);
        return false;
      }
    },
    [detachPairing],
  );
  const updateLocalMetadata = useCallback(
    async (pairingId: string, patch: ForgeSessionView["pairing"]["metadata"]): Promise<boolean> => {
      if (!sessionById.has(pairingId)) return false;
      try {
        await updateStoredPairingMetadata(pairingId, patch);
      } catch {
        return false;
      }
      setSessionById((current) => {
        const session = current.get(pairingId);
        if (!session) return current;
        const next = new Map(current);
        next.set(pairingId, {
          ...session,
          pairing: {
            ...session.pairing,
            metadata: { ...session.pairing.metadata, ...patch },
          },
        });
        return next;
      });
      return true;
    },
    [sessionById],
  );
  const value = useMemo<ForgeSessionsContextValue>(
    () => ({
      ready,
      sessions: [...sessionById.values()].sort((left, right) =>
        comparePairingSummaries(left.pairing, right.pairing),
      ),
      registerPairing,
      reconnect,
      releaseActiveSession,
      renameSession: (pairingId, title) =>
        updateLocalMetadata(pairingId, { customTitle: normalizeSessionAlias(title) }),
      pinSession: (pairingId) => {
        const session = sessionById.get(pairingId);
        if (!session) return Promise.resolve(false);
        if (session.pairing.metadata.pinnedAt !== undefined) return Promise.resolve(true);
        const pinnedAt = new Date().toISOString();
        return updateLocalMetadata(pairingId, { pinnedAt, pinOrderKey: pinnedAt });
      },
      unpinSession: (pairingId) =>
        updateLocalMetadata(pairingId, { pinnedAt: undefined, pinOrderKey: undefined }),
      archiveSession: async (pairingId) => {
        const archived = await updateLocalMetadata(pairingId, {
          archivedAt: new Date().toISOString(),
          pinnedAt: undefined,
          pinOrderKey: undefined,
        });
        if (archived) detachPairing(pairingId);
        return archived;
      },
      sendPrompt: (pairingId, text, images) => socket(pairingId)?.sendPrompt(text, images) ?? null,
      cancel: (pairingId) => socket(pairingId)?.cancel() ?? null,
      askBtw: (pairingId, question) => socket(pairingId)?.askBtw(question) ?? null,
      refreshUsage: (pairingId) => socket(pairingId)?.refreshUsage() ?? null,
      setModel: (pairingId, modelId, reasoningEffort) =>
        socket(pairingId)?.setModel(modelId, reasoningEffort) ?? null,
      setFastMode: (pairingId, enabled) => socket(pairingId)?.setFastMode(enabled) ?? null,
      editQueuedPrompt: (pairingId, queueItemId, expectedVersion, text) =>
        socket(pairingId)?.editQueuedPrompt(queueItemId, expectedVersion, text) ?? null,
      steerQueuedPrompt: (pairingId, queueItemId, expectedVersion) =>
        socket(pairingId)?.steerQueuedPrompt(queueItemId, expectedVersion) ?? null,
      cancelQueuedPrompt: (pairingId, queueItemId, expectedVersion) =>
        socket(pairingId)?.cancelQueuedPrompt(queueItemId, expectedVersion) ?? null,
      newSession: (pairingId) => socket(pairingId)?.newSession() ?? null,
      completeNewSession,
      resolveInteraction: (pairingId, interactionId, response) =>
        socket(pairingId)?.resolveInteraction(interactionId, response) ?? null,
    }),
    [
      detachPairing,
      completeNewSession,
      ready,
      reconnect,
      registerPairing,
      releaseActiveSession,
      sessionById,
      socket,
      updateLocalMetadata,
    ],
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

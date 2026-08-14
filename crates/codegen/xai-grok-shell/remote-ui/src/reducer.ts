import {
  FORGE_REMOTE_PROTOCOL_VERSION,
  type RemoteSessionEvent,
  type RemoteSessionSnapshot,
  type ServerMessage,
} from "./protocol";

export type ConnectionPhase =
  | "connecting"
  | "syncing"
  | "live"
  | "reconnecting"
  | "resyncing"
  | "passive"
  | "revoked"
  | "closed"
  | "conflict"
  | "incompatible";

export interface PendingCommand {
  type:
    | "prompt"
    | "cancel"
    | "setModel"
    | "setFastMode"
    | "btw"
    | "refreshUsage"
    | "resolveInteraction"
    | "queue"
    | "newSession"
    | "acceptNewSession";
  label: string;
  queueItemId?: string;
  expectedVersion?: number;
}

export interface RemoteClientState {
  phase: ConnectionPhase;
  sessionId?: string;
  expiresAt?: string;
  revision?: number;
  session?: RemoteSessionSnapshot;
  pendingCommands: Record<string, PendingCommand>;
  needsResync: boolean;
  lastError?: string;
  revocationReason?: "stopped" | "expired" | "session_closed";
  reconnectAttempt: number;
}

export type RemoteClientAction =
  | { type: "socketConnecting"; reconnecting: boolean; attempt: number }
  | { type: "socketOpen" }
  | { type: "socketClosed"; code: number; reason: string; wasClean: boolean }
  | { type: "localExpired" }
  | { type: "serverMessage"; message: ServerMessage }
  | { type: "decodeError"; message: string }
  | { type: "commandQueued"; commandId: string; command: PendingCommand }
  | { type: "commandSettled"; commandId: string }
  | { type: "resyncSent" }
  | { type: "clearError" };

export const initialRemoteClientState: RemoteClientState = {
  phase: "connecting",
  pendingCommands: {},
  needsResync: false,
  reconnectAttempt: 0,
};

export function applyRemoteEvent(
  session: RemoteSessionSnapshot,
  event: RemoteSessionEvent,
): RemoteSessionSnapshot | null {
  if (event.kind === "stateReplaced") return event.session;
  if (
    event.start < 0 ||
    event.deleteCount < 0 ||
    !Number.isSafeInteger(event.start) ||
    !Number.isSafeInteger(event.deleteCount) ||
    event.start > session.transcript.length ||
    event.deleteCount > session.transcript.length - event.start
  ) {
    return null;
  }
  return {
    ...session,
    transcript: [
      ...session.transcript.slice(0, event.start),
      ...event.items,
      ...session.transcript.slice(event.start + event.deleteCount),
    ],
  };
}

function withoutPending(
  pending: Record<string, PendingCommand>,
  commandId: string,
): Record<string, PendingCommand> {
  if (!(commandId in pending)) return pending;
  const next = { ...pending };
  delete next[commandId];
  return next;
}

function reconcilePendingQueueCommands(
  pending: Record<string, PendingCommand>,
  session: RemoteSessionSnapshot,
): Record<string, PendingCommand> {
  let changed = false;
  const next = { ...pending };
  for (const [commandId, command] of Object.entries(next)) {
    if (command.type !== "queue") continue;
    const current = session.queue?.find((item) => item.id === command.queueItemId);
    if (
      current?.source === "shared" &&
      current.version === command.expectedVersion
    ) {
      continue;
    }
    delete next[commandId];
    changed = true;
  }
  return changed ? next : pending;
}

function pendingAfterSnapshot(
  pending: Record<string, PendingCommand>,
  session: RemoteSessionSnapshot,
): Record<string, PendingCommand> {
  const preserved = Object.fromEntries(
    Object.entries(pending).filter(([, command]) =>
      ["queue", "newSession", "acceptNewSession"].includes(command.type),
    ),
  );
  return reconcilePendingQueueCommands(preserved, session);
}

function protocolCompatible(message: Extract<ServerMessage, { protocolVersion: number }>): boolean {
  return message.protocolVersion === FORGE_REMOTE_PROTOCOL_VERSION;
}

export function remoteClientReducer(
  state: RemoteClientState,
  action: RemoteClientAction,
): RemoteClientState {
  switch (action.type) {
    case "socketConnecting":
      if (["revoked", "conflict", "incompatible"].includes(state.phase)) return state;
      return {
        ...state,
        phase: action.reconnecting ? "reconnecting" : "connecting",
        reconnectAttempt: action.attempt,
        needsResync: false,
      };
    case "socketOpen":
      if (["revoked", "conflict", "incompatible"].includes(state.phase)) return state;
      return { ...state, phase: "syncing", lastError: undefined };
    case "socketClosed": {
      if (["revoked", "conflict", "incompatible"].includes(state.phase)) return state;
      if (action.code === 4410 || /superseded/i.test(action.reason)) {
        return {
          ...state,
          phase: "passive",
          pendingCommands: {},
          needsResync: false,
          lastError: undefined,
        };
      }
      const conflict =
        action.code === 4409 || /already connected|one client|conflict/i.test(action.reason);
      if (conflict) {
        return {
          ...state,
          phase: "conflict",
          pendingCommands: {},
          lastError: "This Forge session is already open in another browser or Forge app.",
        };
      }
      if (state.session?.status === "closed" || (action.wasClean && action.code === 1000)) {
        return { ...state, phase: "closed", pendingCommands: {} };
      }
      return {
        ...state,
        phase: "reconnecting",
        pendingCommands: {},
        lastError: action.reason || "The private connection was interrupted.",
      };
    }
    case "localExpired":
      return {
        ...state,
        phase: "revoked",
        revocationReason: "expired",
        pendingCommands: {},
        needsResync: false,
      };
    case "decodeError":
      return {
        ...state,
        phase: "resyncing",
        needsResync: true,
        lastError: action.message,
      };
    case "commandQueued":
      return {
        ...state,
        pendingCommands: {
          ...state.pendingCommands,
          [action.commandId]: action.command,
        },
        lastError: undefined,
      };
    case "commandSettled":
      return {
        ...state,
        pendingCommands: withoutPending(state.pendingCommands, action.commandId),
      };
    case "resyncSent":
      return { ...state, needsResync: false, phase: "resyncing" };
    case "clearError":
      return { ...state, lastError: undefined };
    case "serverMessage": {
      const message = action.message;
      if ("protocolVersion" in message && !protocolCompatible(message)) {
        return {
          ...state,
          phase: "incompatible",
          lastError: `This page supports protocol ${FORGE_REMOTE_PROTOCOL_VERSION}, but Forge sent protocol ${message.protocolVersion}. Stop and reopen the remote after updating Forge.`,
        };
      }
      switch (message.type) {
        case "connected":
          if (state.sessionId && message.sessionId !== state.sessionId) {
            return {
              ...state,
              phase: "incompatible",
              lastError: "Forge reconnected this private link to a different session. Stop this remote immediately.",
            };
          }
          return {
            ...state,
            phase: "syncing",
            sessionId: message.sessionId,
            expiresAt: message.expiresAt,
            lastError: undefined,
          };
        case "snapshot":
          if (state.sessionId && message.session.sessionId !== state.sessionId) {
            return {
              ...state,
              phase: "incompatible",
              lastError: "Forge sent state for a different session. Stop this remote immediately.",
            };
          }
          if (state.revision !== undefined && message.revision < state.revision) return state;
          return {
            ...state,
            phase: message.session.status === "closed" ? "closed" : "live",
            sessionId: message.session.sessionId,
            revision: message.revision,
            session: message.session,
            pendingCommands: pendingAfterSnapshot(state.pendingCommands, message.session),
            needsResync: false,
            reconnectAttempt: 0,
            lastError: undefined,
          };
        case "delta": {
          if (!state.session || state.revision === undefined) {
            return {
              ...state,
              phase: "resyncing",
              needsResync: true,
              lastError: "Forge sent an update before the session snapshot.",
            };
          }
          if (message.revision <= state.revision) return state;
          if (message.baseRevision !== state.revision) {
            return {
              ...state,
              phase: "resyncing",
              needsResync: true,
              lastError: "Some session updates were missed. Refreshing authoritative state.",
            };
          }
          const eventSessionId =
            message.event.kind === "stateReplaced"
              ? message.event.session.sessionId
              : message.event.sessionId;
          if (state.sessionId && eventSessionId !== state.sessionId) {
            return {
              ...state,
              phase: "incompatible",
              lastError: "Forge sent an update for a different session. Stop this remote immediately.",
            };
          }
          const session = applyRemoteEvent(state.session, message.event);
          if (!session) {
            return {
              ...state,
              phase: "resyncing",
              needsResync: true,
              lastError: "Forge sent an invalid transcript update. Refreshing authoritative state.",
            };
          }
          return {
            ...state,
            phase: session.status === "closed" ? "closed" : "live",
            revision: message.revision,
            session,
            pendingCommands: reconcilePendingQueueCommands(state.pendingCommands, session),
            needsResync: false,
            lastError: undefined,
          };
        }
        case "commandResult": {
          const pending = state.pendingCommands[message.commandId];
          const keepPendingQueue = pending?.type === "queue" && message.outcome.status === "ok";
          return {
            ...state,
            pendingCommands: keepPendingQueue
              ? state.pendingCommands
              : withoutPending(state.pendingCommands, message.commandId),
            ...(message.outcome.status === "ok"
              ? { lastError: undefined }
              : { lastError: message.outcome.error.message }),
          };
        }
        case "sessionCreated":
          return {
            ...state,
            pendingCommands: withoutPending(state.pendingCommands, message.commandId),
          };
        case "resyncRequired":
          return {
            ...state,
            phase: "resyncing",
            needsResync: true,
            lastError: message.reason,
          };
        case "pong":
          return state;
        case "revoked":
          return {
            ...state,
            phase: "revoked",
            revocationReason: message.reason,
            pendingCommands: {},
            needsResync: false,
          };
        case "error": {
          const conflict = ["client_conflict", "too_many_clients", "already_connected"].includes(
            message.error.code,
          );
          const incompatible = message.error.code === "sessionMismatch";
          return {
            ...state,
            phase: conflict ? "conflict" : incompatible ? "incompatible" : state.phase,
            lastError: message.error.message,
          };
        }
      }
    }
  }
}

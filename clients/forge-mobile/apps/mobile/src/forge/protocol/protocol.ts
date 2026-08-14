/**
 * Forge Remote protocol copied verbatim from the in-repository browser client.
 * Its provenance is tracked separately from the pinned T3 presentation source.
 */
export const FORGE_REMOTE_PROTOCOL_VERSION = 1 as const;

export type SessionStatus = "idle" | "running" | "waiting_for_input" | "error" | "closed";
export type ItemStatus = "pending" | "running" | "complete" | "failed" | "cancelled";

export interface RemoteModel {
  id: string;
  label: string;
  description?: string;
  reasoningEffort?: {
    options: RemoteReasoningOption[];
  };
}

export interface RemoteReasoningOption {
  id: string;
  label: string;
  description?: string;
}

export interface RemotePlanStep {
  id?: string;
  text: string;
  status?: ItemStatus;
}

export interface RemoteWorkDisclosure {
  durationMs: number;
  finalResponseItemId: string | null;
  workItemIds: string[];
}

export type RemoteTimelineItem =
  | {
      id: string;
      kind: "user" | "assistant" | "reasoning" | "error";
      text: string;
      status?: ItemStatus;
      createdAt?: string;
    }
  | {
      id: string;
      kind: "system";
      text: string;
      status?: ItemStatus;
      createdAt?: string;
      workDisclosure?: RemoteWorkDisclosure;
    }
  | {
      id: string;
      kind: "tool";
      title: string;
      status: ItemStatus;
      detail?: string;
      input?: string;
      output?: string;
      createdAt?: string;
    }
  | {
      id: string;
      kind: "plan";
      title?: string;
      text?: string;
      steps?: RemotePlanStep[];
      status?: ItemStatus;
      createdAt?: string;
    }
  | {
      id: string;
      kind: "btw";
      question: string;
      response?: string;
      status?: ItemStatus;
      createdAt?: string;
    }
  | {
      id: string;
      kind: "background";
      title: string;
      detail?: string;
      status: ItemStatus;
      createdAt?: string;
    };

export interface RemotePermissionOption {
  id: string;
  label: string;
  description?: string;
}

export interface RemoteQuestionOption {
  label: string;
  description?: string;
}

export interface RemoteQuestion {
  prompt: string;
  options: RemoteQuestionOption[];
  multiple?: boolean;
  allowFreeform?: boolean;
}

interface RemoteInteractionBase {
  interactionId: string;
  title?: string;
  description?: string;
  status?: "pending" | "resolved" | "cancelled" | "timed_out";
}

export type RemoteInteraction =
  | (RemoteInteractionBase & {
      kind: "permission";
      options: RemotePermissionOption[];
      allowFollowup?: boolean;
    })
  | (RemoteInteractionBase & {
      kind: "question";
      questions: RemoteQuestion[];
    })
  | (RemoteInteractionBase & {
      kind: "plan";
      plan: string;
      allowFeedback?: boolean;
    })
  | (RemoteInteractionBase & {
      kind: "unsupported";
      method?: string;
    });

export interface RemoteQueueItem {
  id: string;
  text: string;
  position?: number;
  source: "shared" | "local";
  version?: number;
  kind?: string;
  actions: {
    edit: boolean;
    steer: boolean;
    cancel: boolean;
  };
}

export interface RemoteTaskState {
  label?: string;
  progress?: number;
  backgroundCount?: number;
}

export type RemoteUsageStatus = "idle" | "loading" | "ready" | "partial" | "error";
export type RemoteUsageCostState = "exact" | "partial" | "unavailable";

export interface RemoteUsageContext {
  usedTokens: number;
  totalTokens: number;
  freeTokens: number;
  usedPercent: number;
  autoCompactPercent: number;
}

export interface RemoteUsageModel {
  modelId: string;
  inputTokens: number;
  cachedReadTokens: number;
  cacheCreationTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  totalTokens: number;
  modelCalls: number;
  apiDurationMs: number;
  costUsdTicks?: string;
  costState: RemoteUsageCostState;
}

export interface RemoteUsageSession {
  inputTokens: number;
  cachedReadTokens: number;
  cacheCreationTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  totalTokens: number;
  modelCalls: number;
  apiDurationMs: number;
  costUsdTicks?: string;
  costState: RemoteUsageCostState;
  incomplete: boolean;
  models?: RemoteUsageModel[];
}

export interface RemoteUsageWindow {
  label: string;
  usedPercent: number;
  windowSeconds?: number;
  resetAt?: number;
  resetLabel?: string;
}

export interface RemoteUsageAccount {
  provider: string;
  status: "ready" | "unavailable" | "error";
  plan?: string;
  allowed?: boolean;
  windows: RemoteUsageWindow[];
  credits?: {
    balance: string;
    unlimited: boolean;
  };
  message?: string;
}

export interface RemoteUsageSnapshot {
  status: RemoteUsageStatus;
  refreshedAt?: string;
  context?: RemoteUsageContext;
  session?: RemoteUsageSession;
  account?: RemoteUsageAccount;
  errors?: {
    context?: string;
    session?: string;
    account?: string;
  };
}

export interface RemoteCapabilities {
  prompt: boolean;
  cancel: boolean;
  setModel: boolean;
  fastMode: boolean;
  reasoning: boolean;
  btw: boolean;
  usage: boolean;
  resolveInteractions: boolean;
  queueControl: boolean;
  newSession: boolean;
}

export interface RemoteSessionCreated {
  sessionId: string;
  pairingUrl: string;
  expiresAt: string;
}

export interface RemoteFastModeState {
  supported: boolean;
  enabled: boolean;
  pending?: boolean;
}

export interface RemoteSessionSnapshot {
  sessionId: string;
  title?: string;
  cwd?: string;
  status: SessionStatus;
  transcript: RemoteTimelineItem[];
  currentModel?: RemoteModel;
  availableModels: RemoteModel[];
  modelSwitchPending?: boolean;
  fastMode?: RemoteFastModeState;
  reasoningEffort?: {
    current?: string;
    options: RemoteReasoningOption[];
  };
  planMode?: {
    active: boolean;
    plan?: string;
  };
  activeInteractions: RemoteInteraction[];
  queue?: RemoteQueueItem[];
  taskState?: RemoteTaskState;
  usage?: RemoteUsageSnapshot;
  capabilities: RemoteCapabilities;
}

export interface RemoteError {
  code: string;
  message: string;
  retryable?: boolean;
}

export type RemoteSessionEvent =
  | { kind: "stateReplaced"; session: RemoteSessionSnapshot }
  | {
      kind: "transcriptSpliced";
      sessionId: string;
      start: number;
      deleteCount: number;
      items: RemoteTimelineItem[];
    };

export type ServerMessage =
  | {
      type: "connected";
      protocolVersion: number;
      sessionId: string;
      expiresAt: string;
    }
  | {
      type: "snapshot";
      protocolVersion: number;
      revision: number;
      session: RemoteSessionSnapshot;
    }
  | {
      type: "delta";
      protocolVersion: number;
      baseRevision: number;
      revision: number;
      event: RemoteSessionEvent;
    }
  | {
      type: "commandResult";
      protocolVersion: number;
      commandId: string;
      outcome: { status: "ok" } | { status: "error"; error: RemoteError };
    }
  | ({
      type: "sessionCreated";
      protocolVersion: number;
      commandId: string;
    } & RemoteSessionCreated)
  | { type: "resyncRequired"; protocolVersion: number; reason: string }
  | { type: "pong"; protocolVersion: number; commandId: string }
  | {
      type: "revoked";
      protocolVersion: number;
      reason: "stopped" | "expired" | "session_closed";
    }
  | { type: "error"; protocolVersion: number; error: RemoteError };

export type InteractionResponse =
  | { kind: "permission"; optionId: string }
  | { kind: "permissionFollowup"; text: string }
  | { kind: "question"; answers: RemoteQuestionAnswer[] }
  | {
      kind: "plan";
      outcome: "approved" | "cancelled" | "abandoned";
      feedback?: string;
    }
  | { kind: "cancel" };

export interface RemoteQuestionAnswer {
  questionIndex: number;
  optionIndices: number[];
  freeform?: string;
}

export interface RemotePromptImage {
  name: string;
  mimeType: string;
  data: string;
}

export type ClientMessage =
  | { type: "hello"; protocolVersion: 1 }
  | {
      type: "command";
      protocolVersion: 1;
      commandId: string;
      command:
        | { type: "prompt"; text: string; images?: RemotePromptImage[] }
        | { type: "cancel" }
        | { type: "setModel"; modelId: string; reasoningEffort: string | null }
        | { type: "setFastMode"; enabled: boolean }
        | { type: "btw"; question: string }
        | { type: "refreshUsage" }
        | { type: "resolveInteraction"; interactionId: string; response: InteractionResponse }
        | {
            type: "editQueuedPrompt";
            queueItemId: string;
            expectedVersion: number;
            text: string;
          }
        | { type: "steerQueuedPrompt"; queueItemId: string; expectedVersion: number }
        | { type: "cancelQueuedPrompt"; queueItemId: string; expectedVersion: number }
        | { type: "newSession" }
        | { type: "acceptNewSession"; sessionId: string }
        | { type: "resync" }
        | { type: "ping" };
    };

export class ProtocolDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ProtocolDecodeError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function stringField(record: Record<string, unknown>, name: string): string {
  const value = record[name];
  if (typeof value !== "string") throw new ProtocolDecodeError(`Missing string field: ${name}`);
  return value;
}

function numberField(record: Record<string, unknown>, name: string): number {
  const value = record[name];
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new ProtocolDecodeError(`Invalid non-negative integer field: ${name}`);
  }
  return value;
}

function finiteNonNegativeNumberField(record: Record<string, unknown>, name: string): number {
  const value = record[name];
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new ProtocolDecodeError(`Invalid non-negative number field: ${name}`);
  }
  return value;
}

function normalizeType(type: string): ServerMessage["type"] | string {
  if (type === "command_result") return "commandResult";
  if (type === "resync_required") return "resyncRequired";
  return type;
}

function decodeRemoteError(value: unknown): RemoteError {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid error payload");
  return {
    code: stringField(value, "code"),
    message: stringField(value, "message"),
    ...(typeof value.retryable === "boolean" ? { retryable: value.retryable } : {}),
  };
}

function optionalString(record: Record<string, unknown>, name: string): string | undefined {
  const value = record[name];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") throw new ProtocolDecodeError(`Invalid string field: ${name}`);
  return value;
}

function booleanField(record: Record<string, unknown>, name: string): boolean {
  const value = record[name];
  if (typeof value !== "boolean") throw new ProtocolDecodeError(`Missing boolean field: ${name}`);
  return value;
}

function optionalBoolean(record: Record<string, unknown>, name: string): boolean | undefined {
  const value = record[name];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "boolean") throw new ProtocolDecodeError(`Invalid boolean field: ${name}`);
  return value;
}

const ITEM_STATUSES = new Set<ItemStatus>([
  "pending",
  "running",
  "complete",
  "failed",
  "cancelled",
]);

function optionalItemStatus(
  record: Record<string, unknown>,
  name = "status",
): ItemStatus | undefined {
  const value = optionalString(record, name);
  if (value === undefined) return undefined;
  if (!ITEM_STATUSES.has(value as ItemStatus)) {
    throw new ProtocolDecodeError(`Invalid item status: ${value}`);
  }
  return value as ItemStatus;
}

function decodeModel(value: unknown): RemoteModel {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid model");
  let reasoningEffort: RemoteModel["reasoningEffort"];
  if (value.reasoningEffort !== undefined && value.reasoningEffort !== null) {
    if (!isRecord(value.reasoningEffort) || !Array.isArray(value.reasoningEffort.options)) {
      throw new ProtocolDecodeError("Invalid model reasoning effort");
    }
    reasoningEffort = { options: value.reasoningEffort.options.map(decodeReasoningOption) };
  }
  return {
    id: stringField(value, "id"),
    label: stringField(value, "label"),
    ...(optionalString(value, "description")
      ? { description: optionalString(value, "description") }
      : {}),
    ...(reasoningEffort ? { reasoningEffort } : {}),
  };
}

function decodeReasoningOption(value: unknown): RemoteReasoningOption {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid reasoning option");
  return {
    id: stringField(value, "id"),
    label: stringField(value, "label"),
    ...(optionalString(value, "description")
      ? { description: optionalString(value, "description") }
      : {}),
  };
}

const USAGE_STATUSES = new Set<RemoteUsageStatus>(["idle", "loading", "ready", "partial", "error"]);
const USAGE_COST_STATES = new Set<RemoteUsageCostState>(["exact", "partial", "unavailable"]);

function usageCostState(record: Record<string, unknown>): RemoteUsageCostState {
  const value = stringField(record, "costState");
  if (!USAGE_COST_STATES.has(value as RemoteUsageCostState)) {
    throw new ProtocolDecodeError(`Invalid usage cost state: ${value}`);
  }
  return value as RemoteUsageCostState;
}

function optionalCostTicks(record: Record<string, unknown>): { costUsdTicks?: string } {
  const value = optionalString(record, "costUsdTicks");
  if (value === undefined) return {};
  if (!/^\d+$/.test(value)) throw new ProtocolDecodeError("Invalid usage cost ticks");
  return { costUsdTicks: value };
}

function decodeUsageModel(value: unknown): RemoteUsageModel {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid usage model");
  return {
    modelId: stringField(value, "modelId"),
    inputTokens: numberField(value, "inputTokens"),
    cachedReadTokens: numberField(value, "cachedReadTokens"),
    cacheCreationTokens: numberField(value, "cacheCreationTokens"),
    outputTokens: numberField(value, "outputTokens"),
    reasoningTokens: numberField(value, "reasoningTokens"),
    totalTokens: numberField(value, "totalTokens"),
    modelCalls: numberField(value, "modelCalls"),
    apiDurationMs: numberField(value, "apiDurationMs"),
    ...optionalCostTicks(value),
    costState: usageCostState(value),
  };
}

function decodeUsage(value: unknown): RemoteUsageSnapshot {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid usage snapshot");
  const status = stringField(value, "status");
  if (!USAGE_STATUSES.has(status as RemoteUsageStatus)) {
    throw new ProtocolDecodeError(`Invalid usage status: ${status}`);
  }
  const usage: RemoteUsageSnapshot = {
    status: status as RemoteUsageStatus,
    ...(optionalString(value, "refreshedAt")
      ? { refreshedAt: optionalString(value, "refreshedAt") }
      : {}),
  };
  if (value.context !== undefined && value.context !== null) {
    if (!isRecord(value.context)) throw new ProtocolDecodeError("Invalid context usage");
    usage.context = {
      usedTokens: numberField(value.context, "usedTokens"),
      totalTokens: numberField(value.context, "totalTokens"),
      freeTokens: numberField(value.context, "freeTokens"),
      usedPercent: finiteNonNegativeNumberField(value.context, "usedPercent"),
      autoCompactPercent: finiteNonNegativeNumberField(value.context, "autoCompactPercent"),
    };
  }
  if (value.session !== undefined && value.session !== null) {
    if (!isRecord(value.session)) throw new ProtocolDecodeError("Invalid session usage");
    usage.session = {
      inputTokens: numberField(value.session, "inputTokens"),
      cachedReadTokens: numberField(value.session, "cachedReadTokens"),
      cacheCreationTokens: numberField(value.session, "cacheCreationTokens"),
      outputTokens: numberField(value.session, "outputTokens"),
      reasoningTokens: numberField(value.session, "reasoningTokens"),
      totalTokens: numberField(value.session, "totalTokens"),
      modelCalls: numberField(value.session, "modelCalls"),
      apiDurationMs: numberField(value.session, "apiDurationMs"),
      ...optionalCostTicks(value.session),
      costState: usageCostState(value.session),
      incomplete: booleanField(value.session, "incomplete"),
      ...(value.session.models === undefined || value.session.models === null
        ? {}
        : Array.isArray(value.session.models)
          ? { models: value.session.models.map(decodeUsageModel) }
          : (() => {
              throw new ProtocolDecodeError("Invalid usage model list");
            })()),
    };
  }
  if (value.account !== undefined && value.account !== null) {
    if (!isRecord(value.account) || !Array.isArray(value.account.windows)) {
      throw new ProtocolDecodeError("Invalid account usage");
    }
    const accountStatus = stringField(value.account, "status");
    if (!["ready", "unavailable", "error"].includes(accountStatus)) {
      throw new ProtocolDecodeError(`Invalid account usage status: ${accountStatus}`);
    }
    const credits = value.account.credits;
    if (credits !== undefined && credits !== null && !isRecord(credits)) {
      throw new ProtocolDecodeError("Invalid usage credits");
    }
    usage.account = {
      provider: stringField(value.account, "provider"),
      status: accountStatus as RemoteUsageAccount["status"],
      ...(optionalString(value.account, "plan")
        ? { plan: optionalString(value.account, "plan") }
        : {}),
      ...(optionalBoolean(value.account, "allowed") === undefined
        ? {}
        : { allowed: optionalBoolean(value.account, "allowed") }),
      windows: value.account.windows.map((window) => {
        if (!isRecord(window)) throw new ProtocolDecodeError("Invalid usage window");
        const windowSeconds = window.windowSeconds;
        const resetAt = window.resetAt;
        if (
          windowSeconds !== undefined &&
          (typeof windowSeconds !== "number" ||
            !Number.isSafeInteger(windowSeconds) ||
            windowSeconds < 0)
        ) {
          throw new ProtocolDecodeError("Invalid usage window seconds");
        }
        if (
          resetAt !== undefined &&
          (typeof resetAt !== "number" || !Number.isSafeInteger(resetAt) || resetAt < 0)
        ) {
          throw new ProtocolDecodeError("Invalid usage reset time");
        }
        return {
          label: stringField(window, "label"),
          usedPercent: finiteNonNegativeNumberField(window, "usedPercent"),
          ...(windowSeconds === undefined ? {} : { windowSeconds }),
          ...(resetAt === undefined ? {} : { resetAt }),
          ...(optionalString(window, "resetLabel")
            ? { resetLabel: optionalString(window, "resetLabel") }
            : {}),
        };
      }),
      ...(isRecord(credits)
        ? {
            credits: {
              balance: stringField(credits, "balance"),
              unlimited: booleanField(credits, "unlimited"),
            },
          }
        : {}),
      ...(optionalString(value.account, "message")
        ? { message: optionalString(value.account, "message") }
        : {}),
    };
  }
  if (value.errors !== undefined && value.errors !== null) {
    if (!isRecord(value.errors)) throw new ProtocolDecodeError("Invalid usage errors");
    usage.errors = {
      ...(optionalString(value.errors, "context")
        ? { context: optionalString(value.errors, "context") }
        : {}),
      ...(optionalString(value.errors, "session")
        ? { session: optionalString(value.errors, "session") }
        : {}),
      ...(optionalString(value.errors, "account")
        ? { account: optionalString(value.errors, "account") }
        : {}),
    };
  }
  return usage;
}

function timelineBase(record: Record<string, unknown>) {
  return {
    id: stringField(record, "id"),
    ...(optionalItemStatus(record) ? { status: optionalItemStatus(record) } : {}),
    ...(optionalString(record, "createdAt")
      ? { createdAt: optionalString(record, "createdAt") }
      : {}),
  };
}

function decodeWorkDisclosure(value: unknown): RemoteWorkDisclosure {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid work disclosure");
  const finalResponseItemId = value.finalResponseItemId;
  if (finalResponseItemId !== null && typeof finalResponseItemId !== "string") {
    throw new ProtocolDecodeError("Invalid work disclosure final response item id");
  }
  if (
    !Array.isArray(value.workItemIds) ||
    !value.workItemIds.every((id) => typeof id === "string")
  ) {
    throw new ProtocolDecodeError("Invalid work disclosure item ids");
  }
  return {
    durationMs: numberField(value, "durationMs"),
    finalResponseItemId,
    workItemIds: value.workItemIds,
  };
}

function decodeTimelineItem(value: unknown): RemoteTimelineItem {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid timeline item");
  const kind = stringField(value, "kind");
  const base = timelineBase(value);
  if (["user", "assistant", "reasoning", "error"].includes(kind)) {
    return { ...base, kind, text: stringField(value, "text") } as RemoteTimelineItem;
  }
  if (kind === "system") {
    return {
      ...base,
      kind,
      text: stringField(value, "text"),
      ...(value.workDisclosure === undefined
        ? {}
        : { workDisclosure: decodeWorkDisclosure(value.workDisclosure) }),
    };
  }
  if (kind === "tool") {
    const status = optionalItemStatus(value);
    if (!status) throw new ProtocolDecodeError("Tool timeline item requires status");
    return {
      ...base,
      kind,
      title: stringField(value, "title"),
      status,
      ...(optionalString(value, "detail") ? { detail: optionalString(value, "detail") } : {}),
      ...(optionalString(value, "input") ? { input: optionalString(value, "input") } : {}),
      ...(optionalString(value, "output") ? { output: optionalString(value, "output") } : {}),
    };
  }
  if (kind === "plan") {
    let steps: RemotePlanStep[] | undefined;
    if (value.steps !== undefined) {
      if (!Array.isArray(value.steps)) throw new ProtocolDecodeError("Invalid plan steps");
      steps = value.steps.map((step) => {
        if (!isRecord(step)) throw new ProtocolDecodeError("Invalid plan step");
        return {
          ...(optionalString(step, "id") ? { id: optionalString(step, "id") } : {}),
          text: stringField(step, "text"),
          ...(optionalItemStatus(step) ? { status: optionalItemStatus(step) } : {}),
        };
      });
    }
    return {
      ...base,
      kind,
      ...(optionalString(value, "title") ? { title: optionalString(value, "title") } : {}),
      ...(optionalString(value, "text") ? { text: optionalString(value, "text") } : {}),
      ...(steps ? { steps } : {}),
    };
  }
  if (kind === "btw") {
    return {
      ...base,
      kind,
      question: stringField(value, "question"),
      ...(optionalString(value, "response") ? { response: optionalString(value, "response") } : {}),
    };
  }
  if (kind === "background") {
    const status = optionalItemStatus(value);
    if (!status) throw new ProtocolDecodeError("Background timeline item requires status");
    return {
      ...base,
      kind,
      title: stringField(value, "title"),
      status,
      ...(optionalString(value, "detail") ? { detail: optionalString(value, "detail") } : {}),
    };
  }
  throw new ProtocolDecodeError(`Unknown timeline item: ${kind}`);
}

function decodeInteraction(value: unknown): RemoteInteraction {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid interaction");
  const kind = stringField(value, "kind");
  const status = optionalString(value, "status");
  if (status && !["pending", "resolved", "cancelled", "timed_out"].includes(status)) {
    throw new ProtocolDecodeError(`Invalid interaction status: ${status}`);
  }
  const base = {
    interactionId: stringField(value, "interactionId"),
    ...(optionalString(value, "title") ? { title: optionalString(value, "title") } : {}),
    ...(optionalString(value, "description")
      ? { description: optionalString(value, "description") }
      : {}),
    ...(status ? { status: status as RemoteInteraction["status"] } : {}),
  };
  if (kind === "permission") {
    if (!Array.isArray(value.options)) throw new ProtocolDecodeError("Invalid permission options");
    return {
      ...base,
      kind,
      options: value.options.map((option) => {
        if (!isRecord(option)) throw new ProtocolDecodeError("Invalid permission option");
        return {
          id: stringField(option, "id"),
          label: stringField(option, "label"),
          ...(optionalString(option, "description")
            ? { description: optionalString(option, "description") }
            : {}),
        };
      }),
      ...(optionalBoolean(value, "allowFollowup") === undefined
        ? {}
        : { allowFollowup: optionalBoolean(value, "allowFollowup") }),
    };
  }
  if (kind === "question") {
    if (!Array.isArray(value.questions)) throw new ProtocolDecodeError("Invalid question list");
    return {
      ...base,
      kind,
      questions: value.questions.map((question) => {
        if (!isRecord(question) || !Array.isArray(question.options)) {
          throw new ProtocolDecodeError("Invalid question");
        }
        return {
          prompt: stringField(question, "prompt"),
          options: question.options.map((option) => {
            if (!isRecord(option)) throw new ProtocolDecodeError("Invalid question option");
            return {
              label: stringField(option, "label"),
              ...(optionalString(option, "description")
                ? { description: optionalString(option, "description") }
                : {}),
            };
          }),
          ...(optionalBoolean(question, "multiple") === undefined
            ? {}
            : { multiple: optionalBoolean(question, "multiple") }),
          ...(optionalBoolean(question, "allowFreeform") === undefined
            ? {}
            : { allowFreeform: optionalBoolean(question, "allowFreeform") }),
        };
      }),
    };
  }
  if (kind === "plan") {
    return {
      ...base,
      kind,
      plan: stringField(value, "plan"),
      ...(optionalBoolean(value, "allowFeedback") === undefined
        ? {}
        : { allowFeedback: optionalBoolean(value, "allowFeedback") }),
    };
  }
  if (kind === "unsupported") return { ...base, kind, method: optionalString(value, "method") };
  throw new ProtocolDecodeError(`Unknown interaction: ${kind}`);
}

function decodeSnapshot(value: unknown): RemoteSessionSnapshot {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid session snapshot");
  const status = stringField(value, "status");
  if (!["idle", "running", "waiting_for_input", "error", "closed"].includes(status)) {
    throw new ProtocolDecodeError(`Invalid session status: ${status}`);
  }
  if (!Array.isArray(value.transcript)) throw new ProtocolDecodeError("Invalid transcript");
  if (!Array.isArray(value.availableModels)) throw new ProtocolDecodeError("Invalid model list");
  if (!Array.isArray(value.activeInteractions)) {
    throw new ProtocolDecodeError("Invalid active interaction list");
  }
  if (!isRecord(value.capabilities)) throw new ProtocolDecodeError("Invalid capabilities");
  const snapshot: RemoteSessionSnapshot = {
    sessionId: stringField(value, "sessionId"),
    ...(optionalString(value, "title") ? { title: optionalString(value, "title") } : {}),
    ...(optionalString(value, "cwd") ? { cwd: optionalString(value, "cwd") } : {}),
    status: status as SessionStatus,
    transcript: value.transcript.map(decodeTimelineItem),
    ...(value.currentModel === undefined || value.currentModel === null
      ? {}
      : { currentModel: decodeModel(value.currentModel) }),
    availableModels: value.availableModels.map(decodeModel),
    ...(optionalBoolean(value, "modelSwitchPending") === undefined
      ? {}
      : { modelSwitchPending: optionalBoolean(value, "modelSwitchPending") }),
    activeInteractions: value.activeInteractions.map(decodeInteraction),
    capabilities: {
      prompt: booleanField(value.capabilities, "prompt"),
      cancel: booleanField(value.capabilities, "cancel"),
      setModel: booleanField(value.capabilities, "setModel"),
      fastMode: optionalBoolean(value.capabilities, "fastMode") ?? false,
      reasoning: booleanField(value.capabilities, "reasoning"),
      btw: booleanField(value.capabilities, "btw"),
      usage: optionalBoolean(value.capabilities, "usage") ?? false,
      resolveInteractions: booleanField(value.capabilities, "resolveInteractions"),
      queueControl: optionalBoolean(value.capabilities, "queueControl") ?? false,
      newSession: optionalBoolean(value.capabilities, "newSession") ?? false,
    },
  };
  if (value.fastMode !== undefined && value.fastMode !== null) {
    if (!isRecord(value.fastMode)) throw new ProtocolDecodeError("Invalid fast mode state");
    snapshot.fastMode = {
      supported: booleanField(value.fastMode, "supported"),
      enabled: booleanField(value.fastMode, "enabled"),
      ...(optionalBoolean(value.fastMode, "pending") === undefined
        ? {}
        : { pending: optionalBoolean(value.fastMode, "pending") }),
    };
  }
  if (value.reasoningEffort !== undefined && value.reasoningEffort !== null) {
    if (!isRecord(value.reasoningEffort) || !Array.isArray(value.reasoningEffort.options)) {
      throw new ProtocolDecodeError("Invalid reasoning effort");
    }
    snapshot.reasoningEffort = {
      ...(optionalString(value.reasoningEffort, "current")
        ? { current: optionalString(value.reasoningEffort, "current") }
        : {}),
      options: value.reasoningEffort.options.map(decodeReasoningOption),
    };
  }
  if (value.planMode !== undefined && value.planMode !== null) {
    if (!isRecord(value.planMode)) throw new ProtocolDecodeError("Invalid plan mode");
    snapshot.planMode = {
      active: booleanField(value.planMode, "active"),
      ...(optionalString(value.planMode, "plan")
        ? { plan: optionalString(value.planMode, "plan") }
        : {}),
    };
  }
  if (value.queue !== undefined && value.queue !== null) {
    if (!Array.isArray(value.queue)) throw new ProtocolDecodeError("Invalid queue");
    snapshot.queue = value.queue.map((item) => {
      if (!isRecord(item)) throw new ProtocolDecodeError("Invalid queue item");
      const position = item.position;
      if (
        position !== undefined &&
        (typeof position !== "number" || !Number.isSafeInteger(position))
      ) {
        throw new ProtocolDecodeError("Invalid queue position");
      }
      return {
        id: stringField(item, "id"),
        text: stringField(item, "text"),
        ...(position === undefined ? {} : { position }),
        source:
          item.source === undefined
            ? "local"
            : item.source === "shared" || item.source === "local"
              ? item.source
              : (() => {
                  throw new ProtocolDecodeError("Invalid queue source");
                })(),
        ...(optionalString(item, "kind") ? { kind: optionalString(item, "kind") } : {}),
        ...(item.version === undefined ? {} : { version: numberField(item, "version") }),
        actions:
          item.actions === undefined
            ? { edit: false, steer: false, cancel: false }
            : isRecord(item.actions)
              ? {
                  edit: booleanField(item.actions, "edit"),
                  steer: booleanField(item.actions, "steer"),
                  cancel: booleanField(item.actions, "cancel"),
                }
              : (() => {
                  throw new ProtocolDecodeError("Invalid queue actions");
                })(),
      };
    });
  }
  if (value.taskState !== undefined && value.taskState !== null) {
    if (!isRecord(value.taskState)) throw new ProtocolDecodeError("Invalid task state");
    const progress = value.taskState.progress;
    const backgroundCount = value.taskState.backgroundCount;
    if (progress !== undefined && typeof progress !== "number") {
      throw new ProtocolDecodeError("Invalid task progress");
    }
    if (
      backgroundCount !== undefined &&
      (typeof backgroundCount !== "number" || !Number.isSafeInteger(backgroundCount))
    ) {
      throw new ProtocolDecodeError("Invalid background count");
    }
    snapshot.taskState = {
      ...(optionalString(value.taskState, "label")
        ? { label: optionalString(value.taskState, "label") }
        : {}),
      ...(progress === undefined ? {} : { progress }),
      ...(backgroundCount === undefined ? {} : { backgroundCount }),
    };
  }
  if (value.usage !== undefined && value.usage !== null) snapshot.usage = decodeUsage(value.usage);
  return snapshot;
}

function decodeEvent(value: unknown): RemoteSessionEvent {
  if (!isRecord(value)) throw new ProtocolDecodeError("Invalid delta event");
  const kind = stringField(value, "kind");
  if (kind === "stateReplaced") return { kind, session: decodeSnapshot(value.session) };
  if (kind === "transcriptSpliced") {
    if (!Array.isArray(value.items)) {
      throw new ProtocolDecodeError("Invalid transcript splice items");
    }
    return {
      kind,
      sessionId: stringField(value, "sessionId"),
      start: numberField(value, "start"),
      deleteCount: numberField(value, "deleteCount"),
      items: value.items.map(decodeTimelineItem),
    };
  }
  throw new ProtocolDecodeError(`Unknown delta event: ${kind}`);
}

export function decodeServerMessage(raw: string): ServerMessage {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new ProtocolDecodeError("Server sent invalid JSON");
  }
  if (!isRecord(parsed)) throw new ProtocolDecodeError("Server message must be an object");
  const type = normalizeType(stringField(parsed, "type"));
  switch (type) {
    case "connected":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        sessionId: stringField(parsed, "sessionId"),
        expiresAt: stringField(parsed, "expiresAt"),
      };
    case "snapshot":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        revision: numberField(parsed, "revision"),
        session: decodeSnapshot(parsed.session),
      };
    case "delta":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        baseRevision: numberField(parsed, "baseRevision"),
        revision: numberField(parsed, "revision"),
        event: decodeEvent(parsed.event),
      };
    case "commandResult": {
      const commandId = stringField(parsed, "commandId");
      const protocolVersion = numberField(parsed, "protocolVersion");
      if (!isRecord(parsed.outcome)) throw new ProtocolDecodeError("Invalid command outcome");
      const status = stringField(parsed.outcome, "status");
      if (status === "ok") return { type, protocolVersion, commandId, outcome: { status } };
      if (status === "error") {
        return {
          type,
          protocolVersion,
          commandId,
          outcome: { status, error: decodeRemoteError(parsed.outcome.error) },
        };
      }
      throw new ProtocolDecodeError("Invalid command result");
    }
    case "sessionCreated":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        commandId: stringField(parsed, "commandId"),
        sessionId: stringField(parsed, "sessionId"),
        pairingUrl: stringField(parsed, "pairingUrl"),
        expiresAt: stringField(parsed, "expiresAt"),
      };
    case "resyncRequired":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        reason: stringField(parsed, "reason"),
      };
    case "pong":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        commandId: stringField(parsed, "commandId"),
      };
    case "revoked": {
      const reason = stringField(parsed, "reason");
      if (!["stopped", "expired", "session_closed"].includes(reason)) {
        throw new ProtocolDecodeError(`Invalid revocation reason: ${reason}`);
      }
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        reason: reason as "stopped" | "expired" | "session_closed",
      };
    }
    case "error":
      return {
        type,
        protocolVersion: numberField(parsed, "protocolVersion"),
        error: decodeRemoteError(parsed.error),
      };
    default:
      throw new ProtocolDecodeError(`Unknown server message: ${type}`);
  }
}

export function commandId(): string {
  return globalThis.crypto.randomUUID();
}

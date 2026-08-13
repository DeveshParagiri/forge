import type {
  EnvironmentProject,
  EnvironmentThreadShell,
} from "@t3tools/client-runtime/state/shell";
import {
  ApprovalRequestId,
  EnvironmentId,
  MessageId,
  ProjectId,
  ProviderInstanceId,
  ThreadId,
  TurnId,
  type ModelSelection,
  type ServerConfig,
  type UserInputQuestion,
} from "@t3tools/contracts";

import type {
  PendingApproval,
  PendingUserInput,
  ThreadFeedActivity,
  ThreadFeedEntry,
} from "../lib/threadActivity";
import type { ForgeSessionView } from "./state/ForgeSessionsProvider";
import type {
  InteractionResponse,
  RemoteInteraction,
  RemoteSessionSnapshot,
  RemoteTimelineItem,
} from "./protocol/protocol";

const PERMISSION_OPTION_PREFIX = "permission-option:";
const PERMISSION_CANCEL_ACTION = "permission-cancel";
const PERMISSION_FOLLOWUP_ACTION = "permission-followup";
const PLAN_APPROVE_ACTION = "plan-approve";
const PLAN_CANCEL_ACTION = "plan-cancel";
const PLAN_FEEDBACK_ACTION = "plan-feedback";

function permissionActionTone(label: string): "primary" | "secondary" | "danger" {
  if (/reject|deny|decline|cancel|no\b/i.test(label)) return "danger";
  if (/allow|accept|approve|yes\b/i.test(label)) return "primary";
  return "secondary";
}

/** Maps only actions emitted by presentInteraction back to exact wire responses. */
export function remoteApprovalResponse(
  interaction: RemoteInteraction,
  actionId: string,
  feedback?: string,
): InteractionResponse | null {
  if (interaction.kind === "permission") {
    if (actionId.startsWith(PERMISSION_OPTION_PREFIX)) {
      const index = Number(actionId.slice(PERMISSION_OPTION_PREFIX.length));
      const option = Number.isSafeInteger(index) ? interaction.options[index] : undefined;
      return option ? { kind: "permission", optionId: option.id } : null;
    }
    if (actionId === PERMISSION_CANCEL_ACTION) return { kind: "cancel" };
    const text = feedback?.trim();
    if (actionId === PERMISSION_FOLLOWUP_ACTION && interaction.allowFollowup && text) {
      return { kind: "permissionFollowup", text };
    }
    return null;
  }
  if (interaction.kind === "plan") {
    if (actionId === PLAN_APPROVE_ACTION) return { kind: "plan", outcome: "approved" };
    if (actionId === PLAN_CANCEL_ACTION) return { kind: "plan", outcome: "cancelled" };
    const text = feedback?.trim();
    if (actionId === PLAN_FEEDBACK_ACTION && interaction.allowFeedback && text) {
      return { kind: "plan", outcome: "cancelled", feedback: text };
    }
  }
  return null;
}

const PROVIDER_INSTANCE = ProviderInstanceId.make("forge");
const FALLBACK_MODEL = "Forge agent";

function safeDate(value: string | undefined, fallback: string): string {
  return value && Number.isFinite(Date.parse(value)) ? value : fallback;
}

function basename(cwd: string | undefined): string {
  if (!cwd) return "Forge";
  const parts = cwd.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts.at(-1) ?? "Forge";
}

function itemText(item: RemoteTimelineItem): string {
  switch (item.kind) {
    case "user":
    case "assistant":
    case "reasoning":
    case "system":
    case "error":
      return item.text;
    case "tool":
      return [item.detail, item.output].filter(Boolean).join("\n\n");
    case "plan":
      return [
        item.title ? `### ${item.title}` : "### Plan",
        item.text,
        item.steps
          ?.map((step) => `${step.status === "complete" ? "- [x]" : "- [ ]"} ${step.text}`)
          .join("\n"),
      ]
        .filter(Boolean)
        .join("\n\n");
    case "btw":
      return [`**BTW:** ${item.question}`, item.response].filter(Boolean).join("\n\n");
    case "background":
      return [item.title, item.detail].filter(Boolean).join("\n\n");
  }
}

function timelineActivity(
  item: Extract<RemoteTimelineItem, { kind: "tool" | "background" }>,
): ThreadFeedActivity {
  const failed = item.status === "failed" || item.status === "cancelled";
  return {
    id: item.id,
    createdAt: item.createdAt ?? new Date(0).toISOString(),
    turnId: null,
    summary: item.title,
    detail: itemText(item) || null,
    canExpand: Boolean(itemText(item)),
    getFullDetail: () => itemText(item) || null,
    getCopyText: () => [item.title, itemText(item)].filter(Boolean).join("\n\n"),
    icon: item.kind === "tool" ? "wrench" : "agent",
    toolLike: true,
    status: failed ? "failure" : item.status === "complete" ? "success" : "neutral",
  };
}

function buildFeed(
  snapshot: RemoteSessionSnapshot | null,
  fallbackDate: string,
): ThreadFeedEntry[] {
  if (!snapshot) return [];
  const feed: ThreadFeedEntry[] = [];
  for (const item of snapshot.transcript) {
    const createdAt = safeDate(item.createdAt, fallbackDate);
    if (item.kind === "tool" || item.kind === "background") {
      const activity = timelineActivity({ ...item, createdAt });
      feed.push({
        type: "activity-group",
        id: `activity:${item.id}`,
        createdAt,
        turnId: null,
        activities: [activity],
      });
      continue;
    }
    const role = item.kind === "user" ? "user" : "assistant";
    const text = itemText(item);
    if (!text) continue;
    const id = MessageId.make(`remote:${item.id}`);
    feed.push({
      type: "message",
      id,
      createdAt,
      message: {
        id,
        role,
        text,
        attachments: [],
        turnId: null,
        streaming: role === "assistant" && item.status === "running",
        createdAt,
        updatedAt: createdAt,
      },
    });
  }
  if (snapshot.status === "running") {
    feed.push({ type: "working", id: "remote:working", createdAt: fallbackDate });
  }
  return feed;
}

function modelSelection(snapshot: RemoteSessionSnapshot | null): ModelSelection {
  const model = snapshot?.currentModel?.id ?? snapshot?.availableModels[0]?.id ?? FALLBACK_MODEL;
  const reasoning = snapshot?.reasoningEffort?.current;
  return {
    instanceId: PROVIDER_INSTANCE,
    model,
    ...(reasoning ? { options: [{ id: "reasoningEffort", value: reasoning }] } : {}),
  };
}

function sessionStatus(snapshot: RemoteSessionSnapshot | null) {
  switch (snapshot?.status) {
    case "running":
      return "running" as const;
    case "error":
      return "error" as const;
    case "closed":
      return "stopped" as const;
    default:
      return "ready" as const;
  }
}

export function buildServerConfig(snapshot: RemoteSessionSnapshot | null): ServerConfig | null {
  if (!snapshot || snapshot.availableModels.length === 0) return null;
  return {
    providers: [
      {
        instanceId: PROVIDER_INSTANCE,
        driver: "forge",
        displayName: "Forge",
        enabled: true,
        installed: true,
        auth: { status: "authenticated" },
        slashCommands: [],
        skills: [],
        models: snapshot.availableModels.map((model) => {
          const isCurrent = model.id === snapshot.currentModel?.id;
          const reasoningOptions = isCurrent
            ? (snapshot.reasoningEffort?.options ?? model.reasoningEffort?.options ?? [])
            : (model.reasoningEffort?.options ?? []);
          return {
            slug: model.id,
            name: model.label,
            isDefault: isCurrent,
            isLegacy: false,
            capabilities:
              reasoningOptions.length === 0
                ? null
                : {
                    optionDescriptors: [
                      {
                        id: "reasoningEffort",
                        label: "Reasoning",
                        type: "select",
                        ...(isCurrent && snapshot.reasoningEffort?.current
                          ? { currentValue: snapshot.reasoningEffort.current }
                          : {}),
                        options: reasoningOptions.map((option) => ({
                          id: option.id,
                          label: option.label,
                          ...(option.description ? { description: option.description } : {}),
                        })),
                      },
                    ],
                  },
          };
        }),
      },
    ],
  } as unknown as ServerConfig;
}

export interface RemotePresentation {
  readonly environmentId: EnvironmentId;
  readonly project: EnvironmentProject;
  readonly thread: EnvironmentThreadShell;
  readonly feed: ReadonlyArray<ThreadFeedEntry>;
  readonly serverConfig: ServerConfig | null;
  readonly activeWorkStartedAt: string | null;
}

export function presentRemoteSession(session: ForgeSessionView): RemotePresentation {
  const snapshot = session.snapshot;
  const environmentId = EnvironmentId.make(`pairing:${session.pairing.id}`);
  const projectId = ProjectId.make(`pairing-project:${session.pairing.id}`);
  const threadId = ThreadId.make(`pairing-thread:${session.pairing.id}`);
  const now = new Date().toISOString();
  const transcriptDates = snapshot?.transcript
    .map((item) => item.createdAt)
    .filter((value): value is string => Boolean(value) && Number.isFinite(Date.parse(value!)));
  const createdAt = transcriptDates?.[0] ?? session.pairing.addedAt;
  const updatedAt = transcriptDates?.at(-1) ?? session.pairing.metadata.lastSeenAt ?? createdAt;
  const selection = modelSelection(snapshot);
  const pending = snapshot?.activeInteractions.filter(
    (interaction) => interaction.status === undefined || interaction.status === "pending",
  );
  const title = snapshot?.title ?? session.pairing.metadata.title ?? "Forge session";
  const project: EnvironmentProject = {
    environmentId,
    id: projectId,
    title: basename(snapshot?.cwd),
    workspaceRoot: snapshot?.cwd ?? "/forge",
    repositoryIdentity: null,
    defaultModelSelection: selection,
    defaultThreadEnvMode: null,
    faviconPath: null,
    scripts: [],
    createdAt,
    updatedAt,
  };
  const status = sessionStatus(snapshot);
  const thread: EnvironmentThreadShell = {
    environmentId,
    id: threadId,
    projectId,
    title,
    modelSelection: selection,
    runtimeMode: "approval-required",
    interactionMode: snapshot?.planMode?.active ? "plan" : "default",
    branch: null,
    worktreePath: null,
    latestTurn:
      snapshot?.status === "running"
        ? {
            turnId: TurnId.make(`remote-turn:${session.revision ?? 0}`),
            state: "running",
            requestedAt: updatedAt,
            startedAt: updatedAt,
            completedAt: null,
            assistantMessageId: null,
          }
        : null,
    createdAt,
    updatedAt,
    archivedAt: null,
    settledOverride: null,
    settledAt: null,
    snoozedUntil: null,
    snoozedAt: null,
    pinnedAt: null,
    pinOrderKey: null,
    titleRegeneration: null,
    session: {
      threadId,
      status,
      providerName: "Forge",
      providerInstanceId: PROVIDER_INSTANCE,
      runtimeMode: "approval-required",
      activeTurnId: snapshot?.status === "running" ? TurnId.make("remote-active") : null,
      lastError: snapshot?.status === "error" ? "The Forge session reported an error." : null,
      updatedAt,
    },
    latestUserMessageAt:
      [...(snapshot?.transcript ?? [])].reverse().find((item) => item.kind === "user")?.createdAt ??
      null,
    hasPendingApprovals:
      pending?.some(
        (interaction) => interaction.kind === "permission" || interaction.kind === "plan",
      ) ?? false,
    hasPendingUserInput: pending?.some((interaction) => interaction.kind === "question") ?? false,
    hasActionableProposedPlan: pending?.some((interaction) => interaction.kind === "plan") ?? false,
    backgroundLiveness: snapshot?.status === "running" ? "working" : null,
    planProgress: null,
  };
  return {
    environmentId,
    project,
    thread,
    feed: buildFeed(snapshot, updatedAt || now),
    serverConfig: buildServerConfig(snapshot),
    activeWorkStartedAt: snapshot?.status === "running" ? updatedAt : null,
  };
}

export interface PresentedInteraction {
  readonly interaction: RemoteInteraction;
  readonly approval: PendingApproval | null;
  readonly userInput: PendingUserInput | null;
}

export function presentInteraction(
  interaction: RemoteInteraction | undefined,
): PresentedInteraction | null {
  if (!interaction) return null;
  const requestId = ApprovalRequestId.make(interaction.interactionId);
  if (interaction.kind === "question") {
    const questions: UserInputQuestion[] = interaction.questions.map((question, index) => ({
      id: `question-${index}`,
      header: interaction.title ?? `Question ${index + 1}`,
      question: question.prompt,
      options: question.options.map((option) => ({
        label: option.label,
        description: option.description ?? "",
      })),
      multiSelect: question.multiple === true,
      allowCustomAnswer: question.allowFreeform !== false,
    }));
    return {
      interaction,
      approval: null,
      userInput: {
        requestId,
        createdAt: new Date().toISOString(),
        questions,
      },
    };
  }
  if (interaction.kind === "permission" || interaction.kind === "plan") {
    const remoteActions =
      interaction.kind === "permission"
        ? [
            ...interaction.options.map((option, index) => ({
              id: `${PERMISSION_OPTION_PREFIX}${index}`,
              label: option.label,
              ...(option.description ? { description: option.description } : {}),
              tone: permissionActionTone(option.label),
            })),
            {
              id: PERMISSION_CANCEL_ACTION,
              label: "Cancel request",
              tone: "danger" as const,
            },
          ]
        : [
            { id: PLAN_APPROVE_ACTION, label: "Approve plan", tone: "primary" as const },
            { id: PLAN_CANCEL_ACTION, label: "Cancel", tone: "danger" as const },
          ];
    const remoteFeedback =
      interaction.kind === "permission" && interaction.allowFollowup
        ? {
            actionId: PERMISSION_FOLLOWUP_ACTION,
            actionLabel: "Send feedback",
            placeholder: "Tell Forge what to change",
          }
        : interaction.kind === "plan" && interaction.allowFeedback
          ? {
              actionId: PLAN_FEEDBACK_ACTION,
              actionLabel: "Request changes",
              placeholder: "Changes for Forge",
            }
          : undefined;
    return {
      interaction,
      userInput: null,
      approval: {
        requestId,
        requestKind: interaction.kind === "plan" ? "plan" : "command",
        createdAt: new Date().toISOString(),
        detail: interaction.kind === "plan" ? interaction.plan : interaction.description,
        displayLabel: interaction.title ?? (interaction.kind === "plan" ? "Plan ready" : undefined),
        remoteActions,
        ...(remoteFeedback ? { remoteFeedback } : {}),
      },
    };
  }
  return {
    interaction,
    userInput: null,
    approval: {
      requestId,
      requestKind: "command",
      createdAt: new Date().toISOString(),
      displayLabel: interaction.title ?? "Use the Forge terminal",
      detail: [
        interaction.description,
        "This request cannot be answered on the phone. Use the Forge terminal to continue.",
      ]
        .filter(Boolean)
        .join("\n\n"),
      remoteActions: [],
    },
  };
}

export function reasoningFromSelection(selection: ModelSelection): string | null {
  const value = selection.options?.find((option) => option.id === "reasoningEffort")?.value;
  return typeof value === "string" ? value : null;
}

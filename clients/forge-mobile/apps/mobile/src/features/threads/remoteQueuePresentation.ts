export type RemoteQueueActionId = "edit" | "steer" | "cancel";

export interface RemoteQueueItemInput {
  readonly id: string;
  readonly text: string;
  readonly position?: number;
  readonly source: "shared" | "local";
  readonly version?: number;
  readonly kind?: string;
  readonly actions: {
    readonly edit: boolean;
    readonly steer: boolean;
    readonly cancel: boolean;
  };
}

export interface RemoteQueuedMessagePresentation {
  readonly queueItemId: string;
  readonly messageId: string;
  readonly text: string;
  readonly position: number | null;
  readonly source: "shared" | "local";
  readonly expectedVersion: number | null;
  readonly kind: string | null;
  readonly commandPending: boolean;
  readonly allowedActions: ReadonlySet<RemoteQueueActionId>;
}

export interface RemoteQueueActionPresentation {
  readonly id: RemoteQueueActionId;
  readonly title: "Edit message" | "Steer instead" | "Cancel message";
  readonly systemImage: "pencil" | "arrow.turn.down.right" | "trash";
  readonly destructive: boolean;
}

const REMOTE_QUEUE_ACTIONS: ReadonlyArray<RemoteQueueActionPresentation> = [
  {
    id: "edit",
    title: "Edit message",
    systemImage: "pencil",
    destructive: false,
  },
  {
    id: "steer",
    title: "Steer instead",
    systemImage: "arrow.turn.down.right",
    destructive: false,
  },
  {
    id: "cancel",
    title: "Cancel message",
    systemImage: "trash",
    destructive: true,
  },
];

function validQueuePosition(value: number | undefined): number | null {
  return Number.isSafeInteger(value) && value! >= 0 ? value! : null;
}

function validQueueVersion(value: number | undefined): number | null {
  return Number.isSafeInteger(value) && value! >= 0 ? value! : null;
}

export function presentRemoteQueuedMessages(
  items: ReadonlyArray<RemoteQueueItemInput>,
  pendingQueueItemIds: ReadonlySet<string> = new Set(),
): ReadonlyArray<RemoteQueuedMessagePresentation> {
  return items
    .map((item, sourceIndex) => ({
      item,
      sourceIndex,
      position: validQueuePosition(item.position),
    }))
    .sort((left, right) => {
      if (left.position !== null && right.position !== null) {
        return left.position - right.position || left.sourceIndex - right.sourceIndex;
      }
      if (left.position !== null) return -1;
      if (right.position !== null) return 1;
      return left.sourceIndex - right.sourceIndex;
    })
    .map(({ item, position }) => {
      const expectedVersion = validQueueVersion(item.version);
      const allowedActions = new Set<RemoteQueueActionId>();
      if (item.source === "shared" && expectedVersion !== null) {
        if (item.actions.edit) allowedActions.add("edit");
        if (item.actions.steer) allowedActions.add("steer");
        if (item.actions.cancel) allowedActions.add("cancel");
      }
      return {
        queueItemId: item.id,
        messageId: `remote-queue:${item.id}`,
        text: item.text,
        position,
        source: item.source,
        expectedVersion,
        kind: item.kind?.trim() || null,
        commandPending: pendingQueueItemIds.has(item.id),
        allowedActions,
      };
    });
}

export function remoteQueueActionPresentations(
  message: RemoteQueuedMessagePresentation,
  handlers: Readonly<Record<RemoteQueueActionId, boolean>>,
): ReadonlyArray<RemoteQueueActionPresentation> {
  if (message.expectedVersion === null || message.commandPending) return [];
  return REMOTE_QUEUE_ACTIONS.filter(
    (action) => message.allowedActions.has(action.id) && handlers[action.id],
  );
}

export function showComposerQueueSummary(remoteOnly: boolean, queueCount: number): boolean {
  return !remoteOnly && queueCount > 0;
}

export const FORGE_REMOTE_MESSAGE_CHROME = {
  userBubbleBackgroundColor: "#212121",
  userBubbleBorderRadius: 22,
  userBubbleHorizontalPadding: 16,
  // The native selectable-markdown line box carries more space above its
  // visible glyphs than below them. Equal parent padding therefore makes a
  // one-line bubble look bottom-heavy even though its measured box is centred.
  // These optical insets produce the 44pt one-line ChatGPT geometry at the
  // default 23pt body line height without changing retained T3 chrome.
  userBubbleTopPadding: 8,
  userBubbleBottomPadding: 13,
  userMessageBottomSpacing: 16,
  assistantMessageBottomSpacing: 4,
  assistantResponseActionIconName: { ios: "square.on.square", android: "content_copy" },
  assistantResponseActionTopSpacing: 4,
  assistantResponseActionButtonSize: 24,
  assistantResponseActionIconSize: 16,
  assistantResponseActionHitSlop: 10,
  assistantResponseActionHorizontalOffset: -4,
  workedDisclosureMinHeight: 44,
  workedDisclosureBottomSpacing: 16,
  workedDisclosureHorizontalPadding: 0,
  workedDisclosureFontSize: 16,
  workedDisclosureLineHeight: 22,
  workedDisclosureChevronSize: 14,
  turnFoldMinHeight: 32,
  turnFoldBottomSpacing: 6,
  turnFoldHorizontalPadding: 4,
  workingRowVerticalPadding: 2,
  workingRowBottomSpacing: 6,
  workingRowHorizontalPadding: 6,
} as const;

export const FORGE_REMOTE_WORKED_DISCLOSURE_MINIMUM_MS = 120_000;

export interface RemoteWorkDisclosurePresentation {
  readonly markerMessageId: string;
  readonly label: string;
  readonly durationMs: number;
  readonly hiddenEntryIds: ReadonlySet<string>;
}

export function shouldShowWorkedDisclosure(durationMs: number): boolean {
  return Number.isFinite(durationMs) && durationMs >= FORGE_REMOTE_WORKED_DISCLOSURE_MINIMUM_MS;
}

export function formatWorkedDisclosureLabel(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1_000));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  const parts = [
    hours > 0 ? `${hours}h` : null,
    minutes > 0 ? `${minutes}m` : null,
    seconds > 0 || (hours === 0 && minutes === 0) ? `${seconds}s` : null,
  ].filter((part): part is string => part !== null);
  return `Worked for ${parts.join(" ")}`;
}

export function presentRemoteWorkEntries<T extends { readonly id: string }>(
  entries: ReadonlyArray<T>,
  disclosures: ReadonlyArray<RemoteWorkDisclosurePresentation>,
  expandedMarkerIds: ReadonlySet<string>,
): ReadonlyArray<T> {
  if (disclosures.length === 0) return entries;

  const entryById = new Map<string, T>();
  for (const entry of entries) {
    if (!entryById.has(entry.id)) entryById.set(entry.id, entry);
  }
  const activeDisclosures = disclosures.filter((disclosure) =>
    entryById.has(disclosure.markerMessageId),
  );
  if (activeDisclosures.length === 0) return entries;

  const markerIds = new Set(activeDisclosures.map((disclosure) => disclosure.markerMessageId));
  const disclosureByMarkerId = new Map<string, RemoteWorkDisclosurePresentation>();
  const workOwnerByEntryId = new Map<string, string>();
  for (const disclosure of activeDisclosures) {
    if (!disclosureByMarkerId.has(disclosure.markerMessageId)) {
      disclosureByMarkerId.set(disclosure.markerMessageId, disclosure);
    }
    for (const entryId of disclosure.hiddenEntryIds) {
      if (entryById.has(entryId) && !markerIds.has(entryId) && !workOwnerByEntryId.has(entryId)) {
        workOwnerByEntryId.set(entryId, disclosure.markerMessageId);
      }
    }
  }

  const result: T[] = [];
  const handledMarkerIds = new Set<string>();
  const emittedWorkEntryIds = new Set<string>();
  for (const entry of entries) {
    if (workOwnerByEntryId.has(entry.id)) continue;
    result.push(entry);
    const disclosure = disclosureByMarkerId.get(entry.id);
    if (!disclosure || handledMarkerIds.has(entry.id)) continue;
    handledMarkerIds.add(entry.id);
    if (!expandedMarkerIds.has(entry.id)) continue;
    for (const workEntryId of disclosure.hiddenEntryIds) {
      if (
        workOwnerByEntryId.get(workEntryId) !== entry.id ||
        emittedWorkEntryIds.has(workEntryId)
      ) {
        continue;
      }
      const workEntry = entryById.get(workEntryId);
      if (!workEntry) continue;
      result.push(workEntry);
      emittedWorkEntryIds.add(workEntryId);
    }
  }
  return result;
}

export const RETAINED_T3_FEED_CHROME = {
  turnFoldHeight: 56,
  workingRowVerticalExtras: 24,
} as const;

export function threadFeedFixedRowHeight(input: {
  readonly kind: "turn-fold" | "working";
  readonly remoteOnly: boolean;
  readonly textLineHeight: number;
}): number {
  if (!input.remoteOnly) {
    return input.kind === "turn-fold"
      ? RETAINED_T3_FEED_CHROME.turnFoldHeight
      : input.textLineHeight + RETAINED_T3_FEED_CHROME.workingRowVerticalExtras;
  }
  if (input.kind === "turn-fold") {
    return (
      FORGE_REMOTE_MESSAGE_CHROME.turnFoldMinHeight +
      FORGE_REMOTE_MESSAGE_CHROME.turnFoldBottomSpacing
    );
  }
  return (
    input.textLineHeight +
    FORGE_REMOTE_MESSAGE_CHROME.workingRowVerticalPadding * 2 +
    FORGE_REMOTE_MESSAGE_CHROME.workingRowBottomSpacing
  );
}

export function showUserMessageMeta(remoteOnly: boolean): boolean {
  return !remoteOnly;
}

export function showAssistantResponseCopy(input: {
  readonly isAssistant: boolean;
  readonly isTerminalResponse: boolean;
  readonly isTurnInProgress: boolean;
  readonly streaming: boolean;
  readonly text: string;
}): boolean {
  return (
    input.isAssistant &&
    input.isTerminalResponse &&
    !input.isTurnInProgress &&
    !input.streaming &&
    input.text.trim().length > 0
  );
}

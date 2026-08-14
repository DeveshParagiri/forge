import type { ItemStatus, RemoteTimelineItem, RemoteWorkDisclosure } from "../protocol";

export const WORKED_DISCLOSURE_MINIMUM_MS = 120_000;

export type TimelineWorkDisclosure = RemoteWorkDisclosure;

export type PresentedTimelineEntry =
  | { readonly kind: "item"; readonly item: RemoteTimelineItem }
  | {
      readonly kind: "work-disclosure";
      readonly markerId: string;
      readonly label: string;
      readonly durationMs: number;
      readonly workItemIds: ReadonlyArray<string>;
      readonly expanded: boolean;
    };

interface DisclosureMarker {
  readonly item: RemoteTimelineItem;
  readonly disclosure: TimelineWorkDisclosure;
  readonly workItemIds: ReadonlyArray<string>;
  readonly finalResponseItemId: string | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * The browser and native clients ship from the same wire contract, but this
 * structural reader keeps the presentation layer tolerant while an older
 * cached browser bundle reconnects to a session that already emits markers.
 */
export function timelineWorkDisclosure(item: RemoteTimelineItem): TimelineWorkDisclosure | null {
  if (item.kind !== "system") return null;
  const candidate = (item as RemoteTimelineItem & { readonly workDisclosure?: unknown })
    .workDisclosure;
  if (!isRecord(candidate)) return null;
  if (
    typeof candidate.durationMs !== "number" ||
    !Number.isFinite(candidate.durationMs) ||
    (candidate.finalResponseItemId !== null &&
      typeof candidate.finalResponseItemId !== "string") ||
    !Array.isArray(candidate.workItemIds) ||
    !candidate.workItemIds.every((id) => typeof id === "string")
  ) {
    return null;
  }
  return {
    durationMs: candidate.durationMs,
    finalResponseItemId: candidate.finalResponseItemId,
    workItemIds: candidate.workItemIds,
  };
}

export function shouldShowWorkedDisclosure(durationMs: number): boolean {
  return Number.isFinite(durationMs) && durationMs >= WORKED_DISCLOSURE_MINIMUM_MS;
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

/**
 * Mirrors the native transcript adapter. Disclosure markers under two minutes
 * disappear, qualifying markers move directly before their final response,
 * and only the exact work IDs named by the marker fold underneath it.
 */
export function presentTimelineEntries(
  items: ReadonlyArray<RemoteTimelineItem>,
  expandedMarkerIds: ReadonlySet<string>,
): ReadonlyArray<PresentedTimelineEntry> {
  const itemById = new Map(items.map((item) => [item.id, item]));
  const markers: DisclosureMarker[] = [];
  const markerById = new Map<string, DisclosureMarker>();
  const markerIdsByFinalResponseId = new Map<string, string[]>();
  const workOwnerByItemId = new Map<string, string>();

  for (const item of items) {
    const disclosure = timelineWorkDisclosure(item);
    if (!disclosure || !shouldShowWorkedDisclosure(disclosure.durationMs)) continue;
    const finalResponseItemId =
      disclosure.finalResponseItemId && itemById.has(disclosure.finalResponseItemId)
        ? disclosure.finalResponseItemId
        : null;
    const workItemIds = disclosure.workItemIds.filter(
      (id, index, ids) =>
        itemById.has(id) &&
        id !== item.id &&
        id !== finalResponseItemId &&
        ids.indexOf(id) === index,
    );
    const marker = { item, disclosure, workItemIds, finalResponseItemId };
    markers.push(marker);
    markerById.set(item.id, marker);
    if (finalResponseItemId) {
      const markerIds = markerIdsByFinalResponseId.get(finalResponseItemId) ?? [];
      markerIds.push(item.id);
      markerIdsByFinalResponseId.set(finalResponseItemId, markerIds);
    }
  }

  for (const marker of markers) {
    for (const workItemId of marker.workItemIds) {
      if (!markerById.has(workItemId) && !workOwnerByItemId.has(workItemId)) {
        workOwnerByItemId.set(workItemId, marker.item.id);
      }
    }
  }

  const result: PresentedTimelineEntry[] = [];
  const emittedMarkerIds = new Set<string>();
  const emitMarker = (markerId: string) => {
    if (emittedMarkerIds.has(markerId)) return;
    const marker = markerById.get(markerId);
    if (!marker) return;
    emittedMarkerIds.add(markerId);
    const expanded = expandedMarkerIds.has(markerId);
    result.push({
      kind: "work-disclosure",
      markerId,
      label: formatWorkedDisclosureLabel(marker.disclosure.durationMs),
      durationMs: marker.disclosure.durationMs,
      workItemIds: marker.workItemIds,
      expanded,
    });
    if (!expanded) return;
    for (const workItemId of marker.workItemIds) {
      if (workOwnerByItemId.get(workItemId) !== markerId) continue;
      const workItem = itemById.get(workItemId);
      if (workItem) result.push({ kind: "item", item: workItem });
    }
  };

  for (const item of items) {
    for (const markerId of markerIdsByFinalResponseId.get(item.id) ?? []) {
      emitMarker(markerId);
    }
    if (markerById.has(item.id)) {
      const marker = markerById.get(item.id)!;
      if (!marker.finalResponseItemId) emitMarker(item.id);
      continue;
    }
    if (item.kind === "system") continue;
    if (workOwnerByItemId.has(item.id)) continue;
    result.push({ kind: "item", item });
  }

  // A malformed or late final-response reference must not erase a valid
  // disclosure. Keep it visible at the tail as a safe authoritative fallback.
  for (const marker of markers) emitMarker(marker.item.id);
  return result;
}

export function compactRemoteToolTitle(title: string): string {
  const normalized = title.replace(/[?!.]+$/g, "").trim();
  if (/^run(?:\s+command)?$/i.test(normalized)) return "Run";
  if (/^edit(?:\s+file)?$/i.test(normalized)) return "Edit";
  if (/^read(?:\s+file)?$/i.test(normalized)) return "Read";
  if (/^list(?:\s+directory)?$/i.test(normalized)) return "List";
  if (/^(?:search(?:\s+memory)?|web search)$/i.test(normalized)) return "Search";
  if (/^fetch(?:\s+website)?$/i.test(normalized)) return "Fetch";
  return normalized || title.trim();
}

export function compactWorkDetail(detail: string | undefined): string | null {
  if (!detail) return null;
  const trimmed = detail.trim();
  const shellMatch = trimmed.match(/^\/bin\/zsh -lc ['"]?([\s\S]*?)['"]?$/);
  const compact = (shellMatch?.[1] ?? trimmed).replace(/\s+/g, " ").trim();
  return compact || null;
}

export function workStatusTone(status: ItemStatus | undefined): "success" | "failure" | "neutral" {
  if (status === "complete") return "success";
  if (status === "failed" || status === "cancelled") return "failure";
  return "neutral";
}

/** Returns only terminal assistant responses, never intermediate commentary. */
export function assistantResponseItemIds(
  items: ReadonlyArray<RemoteTimelineItem>,
  sessionWorking: boolean,
): ReadonlySet<string> {
  const result = new Set<string>();
  let sawUser = false;
  let candidate: { readonly id: string; readonly status?: ItemStatus } | null = null;

  const settleCandidate = () => {
    if (candidate && candidate.status !== "running") result.add(candidate.id);
    candidate = null;
  };

  for (const item of items) {
    if (item.kind === "user") {
      settleCandidate();
      sawUser = true;
      continue;
    }
    if (sawUser && item.kind === "assistant") {
      candidate = item;
    }
  }
  if (!sessionWorking) settleCandidate();
  return result;
}

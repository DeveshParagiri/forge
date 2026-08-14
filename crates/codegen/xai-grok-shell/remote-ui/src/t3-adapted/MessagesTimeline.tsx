/*
 * Adapted from T3 Code's apps/web/src/components/chat/MessagesTimeline.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import Check from "lucide-react/dist/esm/icons/check.js";
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down.js";
import CircleAlert from "lucide-react/dist/esm/icons/circle-alert.js";
import CircleDashed from "lucide-react/dist/esm/icons/circle-dashed.js";
import Copy from "lucide-react/dist/esm/icons/copy.js";
import CornerDownRight from "lucide-react/dist/esm/icons/corner-down-right.js";
import Ellipsis from "lucide-react/dist/esm/icons/ellipsis.js";
import GitBranch from "lucide-react/dist/esm/icons/git-branch.js";
import Hammer from "lucide-react/dist/esm/icons/hammer.js";
import ListChecks from "lucide-react/dist/esm/icons/list-checks.js";
import MessageCircleMore from "lucide-react/dist/esm/icons/message-circle-more.js";
import Pencil from "lucide-react/dist/esm/icons/pencil.js";
import Trash2 from "lucide-react/dist/esm/icons/trash-2.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import type { ItemStatus, RemoteQueueItem, RemoteTimelineItem } from "../protocol";
import { Markdown } from "../components/Markdown";
import {
  assistantResponseItemIds,
  compactRemoteToolTitle,
  compactWorkDetail,
  presentTimelineEntries,
  workStatusTone,
  type PresentedTimelineEntry,
} from "./timelinePresentation";

function statusLabel(status: ItemStatus): string {
  switch (status) {
    case "pending":
      return "Pending";
    case "running":
      return "Running";
    case "complete":
      return "Complete";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
  }
}

function StatusIcon({ status }: { status: ItemStatus }) {
  if (status === "complete") return <Check aria-hidden="true" />;
  if (status === "failed") return <X aria-hidden="true" />;
  if (status === "cancelled") return <CircleAlert aria-hidden="true" />;
  return (
    <CircleDashed
      className={status === "running" ? "status-spin" : undefined}
      aria-hidden="true"
    />
  );
}

function WorkLogRow({
  id,
  label,
  status,
  preview,
  children,
}: {
  id: string;
  label: string;
  status?: ItemStatus;
  preview: string | null;
  children?: React.ReactNode;
}) {
  const expandable = children !== undefined;
  return (
    <details
      className="work-entry work-log-row remote-work-row"
      data-item-id={id}
      data-status={status ?? "pending"}
      data-status-tone={workStatusTone(status)}
    >
      <summary aria-label={`${label}, ${status ? statusLabel(status) : "Pending"}`}>
        <span className="work-entry-copy work-log-summary">
          <strong className="work-log-action-label" data-status-tone={workStatusTone(status)}>
            {label}
          </strong>
          {preview ? <span className="work-log-preview">{preview}</span> : null}
        </span>
        {expandable ? <ChevronDown className="work-entry-chevron" aria-hidden="true" /> : null}
      </summary>
      {expandable ? <div className="work-entry-detail work-log-detail">{children}</div> : null}
    </details>
  );
}

function ToolRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "tool" }> }) {
  const label = compactRemoteToolTitle(item.title);
  const preview = compactWorkDetail(item.detail ?? item.output);
  const fullDetail = [item.detail, item.input, item.output].filter(Boolean);
  return (
    <WorkLogRow
      id={item.id}
      label={label}
      status={item.status}
      preview={preview}
    >
      {fullDetail.length > 0
        ? fullDetail.map((detail, index) => (
            <pre key={`${item.id}:detail:${index}`}>{detail}</pre>
          ))
        : undefined}
    </WorkLogRow>
  );
}

type ReasoningTimelineItem = {
  readonly id: string;
  readonly kind: "reasoning";
  readonly text: string;
  readonly status?: ItemStatus;
};

function ThoughtRow({ item }: { item: ReasoningTimelineItem }) {
  const detail = item.text.trim();
  return (
    <WorkLogRow
      id={item.id}
      label="Thought"
      status={item.status}
      preview={compactWorkDetail(detail)}
    >
      {detail ? <Markdown text={detail} /> : undefined}
    </WorkLogRow>
  );
}

function PlanRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "plan" }> }) {
  return (
    <section className="plan-card" aria-label={item.title || "Plan"}>
      <header>
        <ListChecks aria-hidden="true" />
        <strong>{item.title || "Plan"}</strong>
      </header>
      {item.text ? <Markdown text={item.text} /> : null}
      {item.steps?.length ? (
        <ol>
          {item.steps.map((step, index) => (
            <li key={step.id || `${item.id}:${index}`} data-status={step.status || "pending"}>
              <StatusIcon status={step.status || "pending"} />
              <span>{step.text}</span>
            </li>
          ))}
        </ol>
      ) : null}
    </section>
  );
}

function BtwRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "btw" }> }) {
  return (
    <section className="btw-card" aria-label="Side question">
      <header>
        <MessageCircleMore aria-hidden="true" />
        <strong>BTW</strong>
      </header>
      <p>{item.question}</p>
      {item.response ? (
        <Markdown text={item.response} />
      ) : (
        <span className="btw-pending">Waiting for response</span>
      )}
    </section>
  );
}

function BackgroundRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "background" }> }) {
  return (
    <div className="background-row" data-status={item.status}>
      <GitBranch aria-hidden="true" />
      <span>
        <strong>{item.title}</strong>
        {item.detail ? ` · ${item.detail}` : ""}
      </span>
      <StatusIcon status={item.status} />
    </div>
  );
}

async function writeClipboardText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.append(textarea);
  textarea.select();
  document.execCommand("copy");
  textarea.remove();
}

function AssistantCopyAction({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current);
    },
    [],
  );
  return (
    <div className="assistant-response-actions">
      <button
        type="button"
        className="assistant-copy-button"
        data-copied={copied || undefined}
        aria-label={copied ? "Copied response" : "Copy response"}
        onClick={() => {
          void writeClipboardText(text)
            .then(() => {
              setCopied(true);
              if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current);
              resetTimerRef.current = window.setTimeout(() => {
                setCopied(false);
                resetTimerRef.current = null;
              }, 1_200);
            })
            .catch(() => undefined);
        }}
      >
        {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
      </button>
      <span className="sr-only" aria-live="polite">
        {copied ? "Response copied" : ""}
      </span>
    </div>
  );
}

const TimelineRow = memo(function TimelineRow({
  item,
  copyResponse,
}: {
  item: RemoteTimelineItem;
  copyResponse: boolean;
}) {
  switch (item.kind) {
    case "user":
      return (
        <article className="timeline-row user-row remote-user-row" data-item-id={item.id}>
          <div className="user-message remote-user-bubble">
            <Markdown text={item.text} />
          </div>
        </article>
      );
    case "assistant":
      return (
        <article
          className="timeline-row assistant-row remote-assistant-row"
          data-item-id={item.id}
        >
          <div className="assistant-message-content">
            <Markdown text={item.text} />
            {item.status === "running" ? (
              <span className="stream-caret" aria-label="Streaming" />
            ) : null}
          </div>
          {copyResponse && item.text.trim() ? <AssistantCopyAction text={item.text} /> : null}
        </article>
      );
    case "reasoning":
      return <ThoughtRow item={item as ReasoningTimelineItem} />;
    case "system":
      return null;
    case "error":
      return (
        <div className="error-row" role="alert">
          <CircleAlert aria-hidden="true" />
          <Markdown text={item.text} />
        </div>
      );
    case "tool":
      return <ToolRow item={item} />;
    case "plan":
      return <PlanRow item={item} />;
    case "btw":
      return <BtwRow item={item} />;
    case "background":
      return <BackgroundRow item={item} />;
  }
});

function WorkedDisclosureRow({
  entry,
  onToggle,
}: {
  entry: Extract<PresentedTimelineEntry, { kind: "work-disclosure" }>;
  onToggle(markerId: string): void;
}) {
  const canExpand = entry.workItemIds.length > 0;
  const content = (
    <>
      <span className="worked-disclosure-label">{entry.label}</span>
      {canExpand ? (
        <ChevronDown className="worked-disclosure-chevron" aria-hidden="true" />
      ) : null}
    </>
  );
  return (
    <section
      className="worked-disclosure-row"
      data-item-id={entry.markerId}
      data-expanded={entry.expanded || undefined}
    >
      {canExpand ? (
        <button
          type="button"
          className="worked-disclosure-toggle"
          aria-expanded={entry.expanded}
          aria-label={`${entry.label}. ${entry.expanded ? "Hide" : "Show"} work details`}
          onClick={() => onToggle(entry.markerId)}
        >
          {content}
        </button>
      ) : (
        <div className="worked-disclosure-static">{content}</div>
      )}
    </section>
  );
}

export interface QueueTimelineActions {
  readonly onEdit?: (item: RemoteQueueItem) => void;
  readonly onSteer?: (item: RemoteQueueItem) => void;
  readonly onCancel?: (item: RemoteQueueItem) => void;
}

function QueuedMessageRow({
  actions,
  item,
  pending,
}: {
  actions?: QueueTimelineActions;
  item: RemoteQueueItem;
  pending: boolean;
}) {
  const canEdit = !pending && item.actions.edit && actions?.onEdit !== undefined;
  const canSteer = !pending && item.actions.steer && actions?.onSteer !== undefined;
  const canCancel = !pending && item.actions.cancel && actions?.onCancel !== undefined;
  const hasActions = item.source === "shared" && (canEdit || canSteer || canCancel);
  return (
    <article
      className="timeline-row user-row remote-user-row queued-message-row"
      data-queue-item-id={item.id}
      data-pending={pending || undefined}
    >
      <div className="queued-message-meta">
        <CornerDownRight className="queued-message-icon" aria-hidden="true" />
        <span>Queued</span>
        {hasActions ? (
          <details className="queued-message-menu">
            <summary aria-label="Queued message actions">
              <Ellipsis aria-hidden="true" />
            </summary>
            <div className="queued-message-menu-popover">
              {canEdit ? (
                <button type="button" onClick={() => actions?.onEdit?.(item)}>
                  <Pencil aria-hidden="true" />
                  <span>Edit message</span>
                </button>
              ) : null}
              {canSteer ? (
                <button type="button" onClick={() => actions?.onSteer?.(item)}>
                  <CornerDownRight aria-hidden="true" />
                  <span>Steer instead</span>
                </button>
              ) : null}
              {canCancel ? (
                <button
                  type="button"
                  className="queued-message-menu-danger"
                  onClick={() => actions?.onCancel?.(item)}
                >
                  <Trash2 aria-hidden="true" />
                  <span>Cancel message</span>
                </button>
              ) : null}
            </div>
          </details>
        ) : null}
      </div>
      <div className="user-message remote-user-bubble queued-message-bubble">
        <Markdown text={item.text} />
      </div>
    </article>
  );
}

export const MessagesTimeline = memo(function MessagesTimeline({
  items,
  isWorking,
  queue = [],
  queueActions,
  pendingQueueItemIds,
  revision,
}: {
  items: RemoteTimelineItem[];
  isWorking: boolean;
  queue?: ReadonlyArray<RemoteQueueItem>;
  queueActions?: QueueTimelineActions;
  pendingQueueItemIds?: ReadonlySet<string>;
  workingLabel?: string;
  revision?: number;
}) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const followRef = useRef(true);
  const [expandedDisclosureIds, setExpandedDisclosureIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const presentedEntries = useMemo(
    () => presentTimelineEntries(items, expandedDisclosureIds),
    [expandedDisclosureIds, items],
  );
  const responseItemIds = useMemo(
    () => assistantResponseItemIds(items, isWorking),
    [isWorking, items],
  );

  useEffect(() => {
    if (!followRef.current) return;
    const viewport = viewportRef.current;
    if (!viewport) return;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    viewport.scrollTo({
      top: viewport.scrollHeight,
      behavior: reducedMotion ? "auto" : "smooth",
    });
  }, [revision, presentedEntries.length, isWorking]);

  return (
    <div
      ref={viewportRef}
      className="timeline-viewport"
      onScroll={(event) => {
        const element = event.currentTarget;
        followRef.current = element.scrollHeight - element.scrollTop - element.clientHeight < 96;
      }}
    >
      <div className="timeline-root remote-thread-timeline" data-timeline-root="true">
        {presentedEntries.length === 0 ? (
          <div className="timeline-empty">
            <Hammer aria-hidden="true" />
            <p>This Forge session has no messages yet.</p>
          </div>
        ) : (
          presentedEntries.map((entry) =>
            entry.kind === "work-disclosure" ? (
              <WorkedDisclosureRow
                key={`disclosure:${entry.markerId}`}
                entry={entry}
                onToggle={(markerId) => {
                  setExpandedDisclosureIds((current) => {
                    const next = new Set(current);
                    if (next.has(markerId)) next.delete(markerId);
                    else next.add(markerId);
                    return next;
                  });
                }}
              />
            ) : (
              <TimelineRow
                item={entry.item}
                copyResponse={responseItemIds.has(entry.item.id)}
                key={entry.item.id}
              />
            ),
          )
        )}
        {queue.map((item) => (
          <QueuedMessageRow
            actions={queueActions}
            item={item}
            key={`queue:${item.id}`}
            pending={pendingQueueItemIds?.has(item.id) ?? false}
          />
        ))}
        <div className="timeline-end" aria-hidden="true" />
      </div>
    </div>
  );
});

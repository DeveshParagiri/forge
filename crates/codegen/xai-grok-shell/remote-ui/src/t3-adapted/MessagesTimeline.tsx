/*
 * Adapted from T3 Code's apps/web/src/components/chat/MessagesTimeline.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import Bot from "lucide-react/dist/esm/icons/bot.js";
import Check from "lucide-react/dist/esm/icons/check.js";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right.js";
import CircleAlert from "lucide-react/dist/esm/icons/circle-alert.js";
import CircleDashed from "lucide-react/dist/esm/icons/circle-dashed.js";
import GitBranch from "lucide-react/dist/esm/icons/git-branch.js";
import Hammer from "lucide-react/dist/esm/icons/hammer.js";
import ListChecks from "lucide-react/dist/esm/icons/list-checks.js";
import MessageCircleMore from "lucide-react/dist/esm/icons/message-circle-more.js";
import Terminal from "lucide-react/dist/esm/icons/terminal.js";
import Wrench from "lucide-react/dist/esm/icons/wrench.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { memo, useEffect, useRef } from "react";
import type { ItemStatus, RemoteTimelineItem } from "../protocol";
import { Markdown } from "../components/Markdown";

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
  return <CircleDashed className={status === "running" ? "status-spin" : undefined} aria-hidden="true" />;
}

function ToolRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "tool" }> }) {
  const hasDetail = Boolean(item.detail || item.input || item.output);
  return (
    <details className="work-entry" data-status={item.status}>
      <summary aria-label={`${item.title}, ${statusLabel(item.status)}`}>
        <span className="work-entry-icon"><Wrench aria-hidden="true" /></span>
        <span className="work-entry-copy">
          <strong>{item.title}</strong>
          {item.detail ? <span>{item.detail}</span> : null}
        </span>
        <span className="work-entry-status"><StatusIcon status={item.status} /></span>
        {hasDetail ? <ChevronRight className="work-entry-chevron" aria-hidden="true" /> : null}
      </summary>
      {hasDetail ? (
        <div className="work-entry-detail">
          {item.input ? <pre aria-label="Tool input">{item.input}</pre> : null}
          {item.output ? <pre aria-label="Tool output">{item.output}</pre> : null}
        </div>
      ) : null}
    </details>
  );
}

function PlanRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "plan" }> }) {
  return (
    <section className="plan-card" aria-label={item.title || "Plan"}>
      <header><ListChecks aria-hidden="true" /><strong>{item.title || "Plan"}</strong></header>
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
      <header><MessageCircleMore aria-hidden="true" /><strong>BTW</strong></header>
      <p>{item.question}</p>
      {item.response ? <Markdown text={item.response} /> : <span className="btw-pending">Waiting for response</span>}
    </section>
  );
}

function BackgroundRow({ item }: { item: Extract<RemoteTimelineItem, { kind: "background" }> }) {
  return (
    <div className="background-row" data-status={item.status}>
      <GitBranch aria-hidden="true" />
      <span><strong>{item.title}</strong>{item.detail ? ` · ${item.detail}` : ""}</span>
      <StatusIcon status={item.status} />
    </div>
  );
}

const TimelineRow = memo(function TimelineRow({ item }: { item: RemoteTimelineItem }) {
  switch (item.kind) {
    case "user":
      return (
        <article className="timeline-row user-row" data-item-id={item.id}>
          <div className="user-message"><Markdown text={item.text} /></div>
        </article>
      );
    case "assistant":
      return (
        <article className="timeline-row assistant-row" data-item-id={item.id}>
          <Markdown text={item.text} />
          {item.status === "running" ? <span className="stream-caret" aria-label="Streaming" /> : null}
        </article>
      );
    case "reasoning":
      return (
        <details className="reasoning-row" data-item-id={item.id} open={item.status === "running"}>
          <summary><Bot aria-hidden="true" />{item.status === "running" ? "Thinking" : "Thought process"}</summary>
          <Markdown text={item.text} />
        </details>
      );
    case "system":
      return <div className="system-row"><Terminal aria-hidden="true" /><Markdown text={item.text} /></div>;
    case "error":
      return <div className="error-row" role="alert"><CircleAlert aria-hidden="true" /><Markdown text={item.text} /></div>;
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

function WorkingIndicator({ label }: { label?: string }) {
  return (
    <div className="working-indicator" aria-live="polite">
      <span className="working-dots" aria-hidden="true"><i /><i /><i /></span>
      <span>{label || "Forge is working"}</span>
    </div>
  );
}

export const MessagesTimeline = memo(function MessagesTimeline({
  items,
  isWorking,
  workingLabel,
  revision,
}: {
  items: RemoteTimelineItem[];
  isWorking: boolean;
  workingLabel?: string;
  revision?: number;
}) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const followRef = useRef(true);

  useEffect(() => {
    if (!followRef.current) return;
    const viewport = viewportRef.current;
    if (!viewport) return;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    viewport.scrollTo({
      top: viewport.scrollHeight,
      behavior: reducedMotion ? "auto" : "smooth",
    });
  }, [revision, items.length, isWorking]);

  return (
    <div
      ref={viewportRef}
      className="timeline-viewport"
      onScroll={(event) => {
        const element = event.currentTarget;
        followRef.current = element.scrollHeight - element.scrollTop - element.clientHeight < 96;
      }}
    >
      <div className="timeline-root" data-timeline-root="true">
        {items.length === 0 ? (
          <div className="timeline-empty">
            <Hammer aria-hidden="true" />
            <p>This Forge session has no messages yet.</p>
          </div>
        ) : (
          items.map((item) => <TimelineRow item={item} key={item.id} />)
        )}
        {isWorking ? <WorkingIndicator label={workingLabel} /> : null}
        <div className="timeline-end" aria-hidden="true" />
      </div>
    </div>
  );
});

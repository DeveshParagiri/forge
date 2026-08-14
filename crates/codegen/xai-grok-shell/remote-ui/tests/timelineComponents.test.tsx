import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RemoteQueueItem, RemoteTimelineItem } from "../src/protocol";
import { MessagesTimeline } from "../src/t3-adapted/MessagesTimeline";

describe("native-parity browser transcript components", () => {
  it("renders phone-like messages, a terminal copy action, and no system or working leak", () => {
    const items: RemoteTimelineItem[] = [
      { id: "system", kind: "system", text: "Private system reminder" },
      { id: "u1", kind: "user", text: "Please fix it", status: "complete" },
      { id: "commentary", kind: "assistant", text: "Checking now", status: "complete" },
      { id: "answer", kind: "assistant", text: "Fixed.", status: "complete" },
    ];
    const { container } = render(
      <MessagesTimeline items={items} isWorking={false} workingLabel="Working for 17s" />,
    );
    expect(container.querySelector(".remote-user-bubble")).toHaveTextContent("Please fix it");
    expect(container.querySelector(".remote-assistant-row")).toBeInTheDocument();
    expect(screen.queryByText("Private system reminder")).not.toBeInTheDocument();
    expect(screen.queryByText("Working for 17s")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Copy response" })).toHaveLength(1);
    expect(container.querySelector('[data-item-id="answer"] .assistant-copy-button')).toBeVisible();
  });

  it("colors the compact action label semantically without a status checkmark", () => {
    const items: RemoteTimelineItem[] = [
      {
        id: "run",
        kind: "tool",
        title: "Run command",
        detail: "pnpm test",
        output: "passed",
        status: "complete",
      },
      { id: "thought", kind: "reasoning", text: "Verified output", status: "failed" },
    ];
    const { container } = render(<MessagesTimeline items={items} isWorking={false} />);
    expect(container.querySelector('[data-item-id="run"] .work-log-action-label')).toHaveTextContent(
      "Run",
    );
    expect(
      container.querySelector('[data-item-id="run"] .work-log-action-label'),
    ).toHaveAttribute("data-status-tone", "success");
    expect(
      container.querySelector('[data-item-id="thought"] .work-log-action-label'),
    ).toHaveAttribute("data-status-tone", "failure");
    expect(container.querySelector(".work-entry-status")).not.toBeInTheDocument();
  });

  it("expands exact disclosed work and renders queued messages after the transcript", () => {
    const items: RemoteTimelineItem[] = [
      { id: "u1", kind: "user", text: "Ship it" },
      {
        id: "run",
        kind: "tool",
        title: "Run command",
        detail: "pnpm test",
        status: "complete",
      },
      {
        id: "worked",
        kind: "system",
        text: "Worked for 2m 5s",
        workDisclosure: {
          durationMs: 125_000,
          finalResponseItemId: "answer",
          workItemIds: ["run"],
        },
      },
      { id: "answer", kind: "assistant", text: "Done", status: "complete" },
    ];
    const queue: RemoteQueueItem[] = [
      {
        id: "queued",
        text: "And verify release",
        position: 1,
        source: "shared",
        version: 1,
        actions: { edit: true, steer: true, cancel: true },
      },
    ];
    const { container } = render(
      <MessagesTimeline items={items} isWorking={false} queue={queue} />,
    );
    expect(container.querySelector('[data-item-id="run"]')).not.toBeInTheDocument();
    expect(screen.getByText("Worked for 2m 5s")).toBeVisible();
    expect(container.querySelector('[data-queue-item-id="queued"]')).toHaveTextContent(
      "QueuedAnd verify release",
    );
    fireEvent.click(screen.getByRole("button", { name: /Show work details/ }));
    expect(container.querySelector('[data-item-id="run"] summary')).toBeVisible();
    expect(container.querySelector('[data-item-id="worked"]')).toHaveAttribute(
      "data-expanded",
      "true",
    );
  });

  it("offers the native queue actions only when their typed handlers exist", () => {
    const onEdit = vi.fn();
    const onSteer = vi.fn();
    const onCancel = vi.fn();
    const item: RemoteQueueItem = {
      id: "queued",
      text: "Verify release",
      position: 1,
      source: "shared",
      version: 2,
      actions: { edit: true, steer: true, cancel: true },
    };
    render(
      <MessagesTimeline
        items={[]}
        isWorking
        queue={[item]}
        queueActions={{ onEdit, onSteer, onCancel }}
      />,
    );
    fireEvent.click(screen.getByLabelText("Queued message actions"));
    fireEvent.click(screen.getByRole("button", { name: "Edit message" }));
    fireEvent.click(screen.getByRole("button", { name: "Steer instead" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel message" }));
    expect(onEdit).toHaveBeenCalledWith(item);
    expect(onSteer).toHaveBeenCalledWith(item);
    expect(onCancel).toHaveBeenCalledWith(item);
  });

  it("suppresses queue actions while that authoritative queue item has a command pending", () => {
    const onEdit = vi.fn();
    const onSteer = vi.fn();
    const onCancel = vi.fn();
    const item: RemoteQueueItem = {
      id: "queued-pending",
      text: "Verify release",
      position: 1,
      source: "shared",
      version: 3,
      actions: { edit: true, steer: true, cancel: true },
    };
    const { container } = render(
      <MessagesTimeline
        items={[]}
        isWorking
        queue={[item]}
        pendingQueueItemIds={new Set([item.id])}
        queueActions={{ onEdit, onSteer, onCancel }}
      />,
    );
    expect(container.querySelector(`[data-queue-item-id="${item.id}"]`)).toHaveAttribute(
      "data-pending",
      "true",
    );
    expect(screen.queryByLabelText("Queued message actions")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit message" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Steer instead" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancel message" })).not.toBeInTheDocument();
    expect(onEdit).not.toHaveBeenCalled();
    expect(onSteer).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });
});

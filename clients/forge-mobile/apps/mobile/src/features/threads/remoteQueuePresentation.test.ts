import { describe, expect, it } from "vite-plus/test";

import {
  presentRemoteQueuedMessages,
  remoteQueueActionPresentations,
  showComposerQueueSummary,
} from "./remoteQueuePresentation";

const allActions = { edit: true, steer: true, cancel: true } as const;

describe("presentRemoteQueuedMessages", () => {
  it("orders positioned queue rows first and preserves stable source order for ties", () => {
    const presented = presentRemoteQueuedMessages([
      { id: "unpositioned", text: "Last", source: "shared", version: 1, actions: allActions },
      {
        id: "second-a",
        text: "Second A",
        position: 2,
        source: "shared",
        version: 2,
        actions: allActions,
      },
      {
        id: "first",
        text: "First",
        position: 1,
        source: "shared",
        version: 3,
        actions: allActions,
      },
      {
        id: "second-b",
        text: "Second B",
        position: 2,
        source: "shared",
        version: 4,
        actions: allActions,
      },
    ]);
    expect(presented.map((message) => message.queueItemId)).toEqual([
      "first",
      "second-a",
      "second-b",
      "unpositioned",
    ]);
    expect(presented[0]?.messageId).toBe("remote-queue:first");
  });

  it("allows mutations only for versioned shared queue items", () => {
    const [shared, unversioned, local] = presentRemoteQueuedMessages([
      { id: "shared", text: "A", source: "shared", version: 0, actions: allActions },
      { id: "unversioned", text: "B", source: "shared", actions: allActions },
      { id: "local", text: "C", source: "local", version: 2, actions: allActions },
    ]);
    expect([...shared!.allowedActions]).toEqual(["edit", "steer", "cancel"]);
    expect([...unversioned!.allowedActions]).toEqual([]);
    expect([...local!.allowedActions]).toEqual([]);
  });
});

describe("showComposerQueueSummary", () => {
  it("replaces the Forge Remote footer with transcript queue bubbles only", () => {
    expect(showComposerQueueSummary(true, 2)).toBe(false);
    expect(showComposerQueueSummary(false, 2)).toBe(true);
    expect(showComposerQueueSummary(false, 0)).toBe(false);
  });
});

describe("remoteQueueActionPresentations", () => {
  it("uses the exact native menu copy and hides actions without callable handlers", () => {
    const [message] = presentRemoteQueuedMessages([
      { id: "queued", text: "Ship it", source: "shared", version: 7, actions: allActions },
    ]);
    expect(
      remoteQueueActionPresentations(message!, {
        edit: true,
        steer: false,
        cancel: true,
      }),
    ).toEqual([
      {
        id: "edit",
        title: "Edit message",
        systemImage: "pencil",
        destructive: false,
      },
      {
        id: "cancel",
        title: "Cancel message",
        systemImage: "trash",
        destructive: true,
      },
    ]);
  });

  it("uses the ChatGPT-style native symbol for every queued-message action", () => {
    const [message] = presentRemoteQueuedMessages([
      { id: "queued", text: "Ship it", source: "shared", version: 7, actions: allActions },
    ]);
    expect(
      remoteQueueActionPresentations(message!, { edit: true, steer: true, cancel: true }).map(
        ({ id, systemImage }) => [id, systemImage],
      ),
    ).toEqual([
      ["edit", "pencil"],
      ["steer", "arrow.turn.down.right"],
      ["cancel", "trash"],
    ]);
  });

  it("renders no dead menu controls for an unversioned item", () => {
    const [message] = presentRemoteQueuedMessages([
      { id: "queued", text: "Ship it", source: "shared", actions: allActions },
    ]);
    expect(
      remoteQueueActionPresentations(message!, { edit: true, steer: true, cancel: true }),
    ).toEqual([]);
  });

  it("hides a row's controls while its queue mutation is pending", () => {
    const [message] = presentRemoteQueuedMessages(
      [{ id: "queued", text: "Ship it", source: "shared", version: 7, actions: allActions }],
      new Set(["queued"]),
    );
    expect(message?.commandPending).toBe(true);
    expect(
      remoteQueueActionPresentations(message!, { edit: true, steer: true, cancel: true }),
    ).toEqual([]);
  });
});

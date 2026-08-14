import { describe, expect, it } from "vite-plus/test";

import {
  FORGE_REMOTE_MESSAGE_CHROME,
  FORGE_REMOTE_WORKED_DISCLOSURE_MINIMUM_MS,
  RETAINED_T3_FEED_CHROME,
  formatWorkedDisclosureLabel,
  presentRemoteWorkEntries,
  showAssistantResponseCopy,
  showUserMessageMeta,
  shouldShowWorkedDisclosure,
  threadFeedFixedRowHeight,
} from "./threadMessageChrome";

describe("FORGE_REMOTE_MESSAGE_CHROME", () => {
  it("uses the exact neutral ChatGPT-style bubble and compact response spacing", () => {
    expect(FORGE_REMOTE_MESSAGE_CHROME).toEqual({
      userBubbleBackgroundColor: "#212121",
      userBubbleBorderRadius: 22,
      userBubbleHorizontalPadding: 16,
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
    });
  });

  it("optically centers one-line user text in the exact 44pt remote bubble", () => {
    const defaultBodyLineHeight = 23;
    expect(
      FORGE_REMOTE_MESSAGE_CHROME.userBubbleTopPadding +
        defaultBodyLineHeight +
        FORGE_REMOTE_MESSAGE_CHROME.userBubbleBottomPadding,
    ).toBe(44);
    expect(FORGE_REMOTE_MESSAGE_CHROME.userBubbleHorizontalPadding).toBe(16);
    expect(FORGE_REMOTE_MESSAGE_CHROME.userBubbleTopPadding).toBeLessThan(
      FORGE_REMOTE_MESSAGE_CHROME.userBubbleBottomPadding,
    );
  });

  it("keeps the copy glyph visually compact while preserving a 44pt hit region", () => {
    expect(FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionIconName).toEqual({
      ios: "square.on.square",
      android: "content_copy",
    });
    expect(FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionButtonSize).toBe(24);
    expect(FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionIconSize).toBe(16);
    expect(FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionHitSlop).toBe(10);
    expect(
      FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionButtonSize +
        FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionHitSlop * 2,
    ).toBe(44);
    expect(
      FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionHorizontalOffset +
        (FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionButtonSize -
          FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionIconSize) /
          2,
    ).toBe(0);
    expect(FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionIconSize).toBeLessThanOrEqual(
      FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionButtonSize,
    );
    expect(
      FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionHorizontalOffset +
        (FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionButtonSize -
          FORGE_REMOTE_MESSAGE_CHROME.assistantResponseActionIconSize) /
          2,
    ).toBeGreaterThanOrEqual(0);
  });

  it("uses compact fixed heights for completed and active work rows", () => {
    expect(
      threadFeedFixedRowHeight({ kind: "turn-fold", remoteOnly: true, textLineHeight: 20 }),
    ).toBe(38);
    expect(
      threadFeedFixedRowHeight({ kind: "working", remoteOnly: true, textLineHeight: 18 }),
    ).toBe(28);
  });

  it("retains the exact T3 fixed-height geometry outside Forge Remote", () => {
    expect(RETAINED_T3_FEED_CHROME).toEqual({
      turnFoldHeight: 56,
      workingRowVerticalExtras: 24,
    });
    expect(
      threadFeedFixedRowHeight({ kind: "turn-fold", remoteOnly: false, textLineHeight: 20 }),
    ).toBe(56);
    expect(
      threadFeedFixedRowHeight({ kind: "working", remoteOnly: false, textLineHeight: 18 }),
    ).toBe(42);
  });
});

describe("Worked disclosure duration policy", () => {
  it("hides 1m59.9s and shows the exact two-minute boundary", () => {
    expect(FORGE_REMOTE_WORKED_DISCLOSURE_MINIMUM_MS).toBe(120_000);
    expect(shouldShowWorkedDisclosure(119_900)).toBe(false);
    expect(shouldShowWorkedDisclosure(120_000)).toBe(true);
  });

  it("formats the authoritative duration with ChatGPT-style spacing", () => {
    expect(formatWorkedDisclosureLabel(221_000)).toBe("Worked for 3m 41s");
    expect(formatWorkedDisclosureLabel(120_000)).toBe("Worked for 2m");
    expect(formatWorkedDisclosureLabel(3_661_000)).toBe("Worked for 1h 1m 1s");
  });
});

describe("presentRemoteWorkEntries", () => {
  const entryIds = (values: ReadonlyArray<{ readonly id: string }>) =>
    values.map((entry) => entry.id);
  const disclosure = (markerMessageId: string, workEntryIds: ReadonlyArray<string>) => ({
    markerMessageId,
    label: "Worked for 3m 41s",
    durationMs: 221_000,
    hiddenEntryIds: new Set(workEntryIds),
  });

  it("collapses exact work entries into their marker", () => {
    const entries = ["user", "reasoning", "tool", "marker", "answer"].map((id) => ({ id }));
    expect(
      entryIds(
        presentRemoteWorkEntries(entries, [disclosure("marker", ["reasoning", "tool"])], new Set()),
      ),
    ).toEqual(["user", "marker", "answer"]);
  });

  it("inserts expanded work immediately after the marker in metadata order", () => {
    const entries = ["user", "reasoning", "tool", "marker", "answer"].map((id) => ({ id }));
    expect(
      entryIds(
        presentRemoteWorkEntries(
          entries,
          [disclosure("marker", ["reasoning", "tool"])],
          new Set(["marker"]),
        ),
      ),
    ).toEqual(["user", "marker", "reasoning", "tool", "answer"]);
  });

  it("handles two expanded disclosures deterministically without duplicating shared work", () => {
    const entries = [
      "user-1",
      "work-1",
      "marker-1",
      "answer-1",
      "user-2",
      "work-2",
      "work-3",
      "marker-2",
      "answer-2",
    ].map((id) => ({ id }));
    expect(
      entryIds(
        presentRemoteWorkEntries(
          entries,
          [
            disclosure("marker-1", ["work-1"]),
            disclosure("marker-2", ["work-2", "work-1", "work-3"]),
          ],
          new Set(["marker-1", "marker-2"]),
        ),
      ),
    ).toEqual([
      "user-1",
      "marker-1",
      "work-1",
      "answer-1",
      "user-2",
      "marker-2",
      "work-2",
      "work-3",
      "answer-2",
    ]);
  });

  it("leaves entries in place when marker or work IDs are missing", () => {
    const entries = ["user", "orphan-work", "marker", "answer"].map((id) => ({ id }));
    expect(
      entryIds(
        presentRemoteWorkEntries(
          entries,
          [disclosure("missing-marker", ["orphan-work"]), disclosure("marker", ["missing-work"])],
          new Set(["missing-marker", "marker"]),
        ),
      ),
    ).toEqual(["user", "orphan-work", "marker", "answer"]);
  });

  it("returns the original feed when there is no disclosure", () => {
    const entries = ["user", "answer"].map((id) => ({ id }));
    expect(presentRemoteWorkEntries(entries, [], new Set())).toBe(entries);
  });
});

describe("showUserMessageMeta", () => {
  it("removes timestamp and copy footer only from Forge Remote user messages", () => {
    expect(showUserMessageMeta(true)).toBe(false);
    expect(showUserMessageMeta(false)).toBe(true);
  });
});

describe("showAssistantResponseCopy", () => {
  it("adds copy immediately after a completed terminal assistant response", () => {
    expect(
      showAssistantResponseCopy({
        isAssistant: true,
        isTerminalResponse: true,
        isTurnInProgress: false,
        streaming: false,
        text: "Finished response",
      }),
    ).toBe(true);
  });

  it("keeps user and in-progress message chrome clean", () => {
    const completed = {
      isTerminalResponse: true,
      isTurnInProgress: false,
      streaming: false,
      text: "Message",
    } as const;
    expect(showAssistantResponseCopy({ ...completed, isAssistant: false })).toBe(false);
    expect(
      showAssistantResponseCopy({
        ...completed,
        isAssistant: true,
        isTurnInProgress: true,
      }),
    ).toBe(false);
  });
  it("keeps a historical response copyable while a later response streams", () => {
    expect(
      showAssistantResponseCopy({
        isAssistant: true,
        isTerminalResponse: true,
        isTurnInProgress: false,
        streaming: false,
        text: "Earlier answer",
      }),
    ).toBe(true);
  });
});

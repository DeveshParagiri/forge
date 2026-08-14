import { describe, expect, it } from "vitest";
import type { RemoteTimelineItem } from "../src/protocol";
import {
  assistantResponseItemIds,
  compactRemoteToolTitle,
  formatWorkedDisclosureLabel,
  presentTimelineEntries,
  shouldShowWorkedDisclosure,
} from "../src/t3-adapted/timelinePresentation";

function workedMarker(
  durationMs: number,
  workItemIds: string[],
  finalResponseItemId: string | null,
): RemoteTimelineItem {
  return {
    id: "worked-1",
    kind: "system",
    text: `Worked for ${durationMs}ms`,
    status: "complete",
    workDisclosure: { durationMs, workItemIds, finalResponseItemId },
  };
}

describe("native-parity remote timeline presentation", () => {
  it("shows Worked only at two minutes and formats the native duration label", () => {
    expect(shouldShowWorkedDisclosure(119_999)).toBe(false);
    expect(shouldShowWorkedDisclosure(120_000)).toBe(true);
    expect(formatWorkedDisclosureLabel(3_701_000)).toBe("Worked for 1h 1m 41s");
  });

  it("moves a disclosure before its final answer and folds only exact work IDs", () => {
    const items: RemoteTimelineItem[] = [
      { id: "u1", kind: "user", text: "Fix this", status: "complete" },
      {
        id: "thought-1",
        kind: "reasoning",
        text: "Inspecting",
        status: "complete",
      },
      {
        id: "run-1",
        kind: "tool",
        title: "Run command",
        detail: "pnpm test",
        status: "complete",
      },
      { id: "aside", kind: "assistant", text: "Still working", status: "complete" },
      { id: "a1", kind: "assistant", text: "Done", status: "complete" },
      workedMarker(221_000, ["thought-1", "run-1", "missing"], "a1"),
    ];

    const collapsed = presentTimelineEntries(items, new Set());
    expect(
      collapsed.map((entry) =>
        entry.kind === "item" ? entry.item.id : `disclosure:${entry.markerId}`,
      ),
    ).toEqual(["u1", "aside", "disclosure:worked-1", "a1"]);

    const expanded = presentTimelineEntries(items, new Set(["worked-1"]));
    expect(
      expanded.map((entry) =>
        entry.kind === "item" ? entry.item.id : `disclosure:${entry.markerId}`,
      ),
    ).toEqual([
      "u1",
      "aside",
      "disclosure:worked-1",
      "thought-1",
      "run-1",
      "a1",
    ]);
  });

  it("removes sub-two-minute markers and arbitrary system text without hiding work", () => {
    const items: RemoteTimelineItem[] = [
      { id: "system-leak", kind: "system", text: "private hidden instruction" },
      {
        id: "run-1",
        kind: "tool",
        title: "Run",
        detail: "pnpm test",
        status: "complete",
      },
      workedMarker(17_000, ["run-1"], null),
    ];
    const presented = presentTimelineEntries(items, new Set());
    expect(presented).toHaveLength(1);
    expect(presented[0]).toMatchObject({ kind: "item", item: { id: "run-1" } });
  });

  it("normalizes native action names and exposes status on the label rather than a tick", () => {
    expect(compactRemoteToolTitle("Run command")).toBe("Run");
    expect(compactRemoteToolTitle("Edit file.")).toBe("Edit");
    expect(compactRemoteToolTitle("Web search")).toBe("Search");
  });

  it("offers copy only on terminal completed assistant responses", () => {
    const items: RemoteTimelineItem[] = [
      { id: "u1", kind: "user", text: "First" },
      { id: "commentary", kind: "assistant", text: "Checking", status: "complete" },
      { id: "a1", kind: "assistant", text: "First answer", status: "complete" },
      { id: "u2", kind: "user", text: "Second" },
      { id: "a2", kind: "assistant", text: "Streaming", status: "running" },
    ];
    expect([...assistantResponseItemIds(items, true)]).toEqual(["a1"]);
    expect([...assistantResponseItemIds(items, false)]).toEqual(["a1"]);
    expect(
      [...assistantResponseItemIds([...items, { id: "a3", kind: "assistant", text: "Done" }], false)],
    ).toEqual(["a1", "a3"]);
  });
});

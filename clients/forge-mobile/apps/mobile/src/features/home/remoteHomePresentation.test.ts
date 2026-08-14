import { describe, expect, it } from "vitest";

import { DEFAULT_GROUP_DISPLAY_STATE, nextGroupDisplayState } from "./homeListItems";
import {
  REMOTE_HOME_DISCLOSURE_DURATION_MS,
  readRemoteHomeGroupDisplayStates,
  remoteHomeEmptyState,
  remoteHomeNewSessionActionPresentation,
  remoteHomeProjectGroupKey,
  shouldAnimateRemoteHomeGroupAction,
  writeRemoteHomeGroupDisplayState,
} from "./remoteHomePresentation";

describe("Forge Remote Home empty presentation", () => {
  it("stays neutral without connection/loading prose or a spinner state", () => {
    const presentations = [remoteHomeEmptyState(false), remoteHomeEmptyState(true)];

    expect(presentations).toEqual([
      {
        title: "No paired sessions",
        detail: "Pair a Forge session to see it here.",
        loading: false,
      },
      {
        title: "No sessions yet",
        detail: "Your paired Forge sessions will appear here.",
        loading: false,
      },
    ]);
    expect(presentations.map(({ title, detail }) => `${title} ${detail}`).join(" ")).not.toMatch(
      /connect|loading|retry|offline|unavailable|failed/i,
    );
  });
});

describe("Forge Remote Home project identity", () => {
  it("groups sibling sessions only when both host and normalized cwd match", () => {
    const first = remoteHomeProjectGroupKey("Devs-MacBook.ts.net", "/Users/dev/forge/");

    expect(remoteHomeProjectGroupKey("devs-macbook.ts.net", "/Users/dev/forge")).toBe(first);
    expect(remoteHomeProjectGroupKey("other-mac.ts.net", "/Users/dev/forge")).not.toBe(first);
    expect(remoteHomeProjectGroupKey("Devs-MacBook.ts.net", "/Users/dev/other")).not.toBe(first);
  });

  it("retains disclosure state across Home screen remounts without device storage", () => {
    const key = "remote-project:test:disclosure";
    const collapsed = nextGroupDisplayState(DEFAULT_GROUP_DISPLAY_STATE, "toggle-collapsed");

    writeRemoteHomeGroupDisplayState(key, collapsed);

    expect(readRemoteHomeGroupDisplayStates().get(key)).toEqual(collapsed);
  });
});

describe("Forge Remote Home directory controls", () => {
  it("uses the pencil for an exact-directory session instead of competing with Pair", () => {
    expect(
      remoteHomeNewSessionActionPresentation({ projectTitle: "forge", pending: false }),
    ).toEqual({
      accessibilityLabel: "Create new session in forge",
      disabled: false,
      systemImage: "square.and.pencil",
    });
    expect(
      remoteHomeNewSessionActionPresentation({ projectTitle: "forge", pending: true }),
    ).toEqual({
      accessibilityLabel: "Creating new session in forge",
      disabled: true,
      systemImage: "square.and.pencil",
    });
  });

  it("animates only Remote disclosure toggles and honors reduced motion", () => {
    expect(REMOTE_HOME_DISCLOSURE_DURATION_MS).toBe(180);
    expect(
      shouldAnimateRemoteHomeGroupAction({
        action: "toggle-collapsed",
        reduceMotionEnabled: false,
        remoteOnly: true,
      }),
    ).toBe(true);
    expect(
      shouldAnimateRemoteHomeGroupAction({
        action: "toggle-collapsed",
        reduceMotionEnabled: true,
        remoteOnly: true,
      }),
    ).toBe(false);
    expect(
      shouldAnimateRemoteHomeGroupAction({
        action: "show-more",
        reduceMotionEnabled: false,
        remoteOnly: true,
      }),
    ).toBe(false);
    expect(
      shouldAnimateRemoteHomeGroupAction({
        action: "toggle-collapsed",
        reduceMotionEnabled: false,
        remoteOnly: false,
      }),
    ).toBe(false);
  });
});

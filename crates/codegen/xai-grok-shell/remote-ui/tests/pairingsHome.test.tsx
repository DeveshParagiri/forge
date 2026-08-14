import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { StoredPairing } from "../src/pairingRegistry";
import { PairingsHome } from "../src/t3-adapted/PairingsHome";
import {
  buildPairingHomeProjectGroups,
  normalizePairingProjectCwd,
  pairingHomeProjectKey,
} from "../src/t3-adapted/pairingsHomePresentation";

function pairing(
  id: string,
  overrides: Partial<StoredPairing> = {},
): StoredPairing {
  const tokenCharacter = id.charCodeAt(0).toString(16).at(-1) ?? "a";
  return {
    id,
    baseUrl: `https://devs-macbook.tail.example/forge/${tokenCharacter.repeat(64)}/`,
    addedAt: "2030-01-01T00:00:00Z",
    lastSeenAt: "2030-01-01T00:01:00Z",
    title: id,
    cwd: "/Users/dev/Projects/forge",
    status: "idle",
    ...overrides,
  };
}

describe("Forge browser Home project presentation", () => {
  it("groups only sessions with the same origin host and normalized directory", () => {
    const first = pairing("a", { cwd: "/Users/dev/Projects/Forge/" });
    const sibling = pairing("b", { cwd: "\\Users\\dev\\Projects\\forge" });
    const otherHost = pairing("c", {
      baseUrl: `https://other-mac.tail.example/forge/${"c".repeat(64)}/`,
      cwd: "/Users/dev/Projects/forge",
    });
    const otherDirectory = pairing("d", { cwd: "/Users/dev/Projects/other" });

    expect(normalizePairingProjectCwd(" /Users/dev//Projects/forge/ ")).toBe(
      "/Users/dev/Projects/forge",
    );
    expect(pairingHomeProjectKey(first)).toBe(pairingHomeProjectKey(sibling));
    expect(buildPairingHomeProjectGroups([first, sibling, otherHost, otherDirectory])).toHaveLength(3);
  });

  it("orders projects and their representative sessions by latest activity", () => {
    const olderForge = pairing("older Forge", { lastSeenAt: "2030-01-01T00:01:00Z" });
    const newestForge = pairing("newest Forge", { lastSeenAt: "2030-01-01T00:03:00Z" });
    const other = pairing("other", {
      cwd: "/Users/dev/Projects/other",
      lastSeenAt: "2030-01-01T00:02:00Z",
    });

    const groups = buildPairingHomeProjectGroups([olderForge, other, newestForge]);

    expect(groups.map((group) => group.title)).toEqual(["forge", "other"]);
    expect(groups[0]?.pairings.map(({ title }) => title)).toEqual(["newest Forge", "older Forge"]);
    expect(groups[0]?.representative).toBe(newestForge);
  });
});

describe("Forge browser Home interactions", () => {
  it("collapses a project, forces search matches open, then restores its disclosure state", () => {
    const pairings = [
      pairing("Search target", { cwd: "/Users/dev/Projects/collapse-search" }),
      pairing("Another session", { cwd: "/Users/dev/Projects/collapse-search" }),
    ];
    render(<PairingsHome pairings={pairings} onSelect={vi.fn()} />);

    const disclosure = screen.getByRole("button", { name: "Collapse collapse-search, 2 sessions" });
    fireEvent.click(disclosure);
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText("Search target")).not.toBeVisible();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search sessions" }), {
      target: { value: "Search target" },
    });
    expect(screen.getByText("Search target")).toBeVisible();
    expect(screen.getByRole("button", { name: "Collapse collapse-search, 1 session" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );

    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByText("Search target")).not.toBeVisible();
    expect(screen.getByRole("button", { name: "Expand collapse-search, 2 sessions" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("renders no dead creation or overflow controls without their callbacks", () => {
    render(
      <PairingsHome
        pairings={[pairing("Callback-free", { cwd: "/Users/dev/Projects/no-actions" })]}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Create new Forge session" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Create new session in/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /More options for/ })).not.toBeInTheDocument();
  });

  it("executes global, project, and overflow-menu actions when callbacks exist", () => {
    const item = pairing("Executable actions", { cwd: "/Users/dev/Projects/actions" });
    const onCreateSession = vi.fn();
    const onCreateSessionInProject = vi.fn();
    const onRemove = vi.fn();
    render(
      <PairingsHome
        pairings={[item]}
        onSelect={vi.fn()}
        onRemove={onRemove}
        onCreateSession={onCreateSession}
        onCreateSessionInProject={onCreateSessionInProject}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create new Forge session" }));
    fireEvent.click(screen.getByRole("button", { name: "Create new session in actions" }));
    fireEvent.click(screen.getByRole("button", { name: "More options for Executable actions" }));
    const menu = screen.getByRole("menu", { name: "Options for Executable actions" });
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Remove session" }));

    expect(onCreateSession).toHaveBeenCalledOnce();
    expect(onCreateSessionInProject).toHaveBeenCalledOnce();
    expect(onCreateSessionInProject.mock.calls[0]?.[0].cwd).toBe("/Users/dev/Projects/actions");
    expect(onRemove).toHaveBeenCalledWith(item);
  });

  it("focuses the overflow menu and restores its trigger when Escape closes it", async () => {
    const item = pairing("Keyboard actions", { cwd: "/Users/dev/Projects/actions" });
    render(
      <PairingsHome
        pairings={[item]}
        onSelect={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    const trigger = screen.getByRole("button", { name: "More options for Keyboard actions" });
    fireEvent.click(trigger);
    const action = screen.getByRole("menuitem", { name: "Remove session" });
    await waitFor(() => expect(action).toHaveFocus());

    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByRole("menu", { name: "Options for Keyboard actions" })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("marks disclosure motion as reduced when the system preference requests it", () => {
    vi.mocked(window.matchMedia).mockImplementation((query: string) => ({
      matches: query === "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    const { container } = render(
      <PairingsHome
        pairings={[pairing("Reduced", { cwd: "/Users/dev/Projects/reduced-motion" })]}
        onSelect={vi.fn()}
      />,
    );

    expect(container.querySelector(".pairing-project-group")).toHaveAttribute(
      "data-disclosure-motion",
      "reduce",
    );
  });

  it("shows title, status, relative time, and navigation chrome in each thread row", () => {
    vi.spyOn(Date, "now").mockReturnValue(Date.parse("2030-01-01T00:02:00Z"));
    const { container } = render(
      <PairingsHome
        pairings={[
          pairing("Live thread", {
            cwd: "/Users/dev/Projects/thread-chrome",
            status: "running",
            lastSeenAt: "2030-01-01T00:01:00Z",
          }),
        ]}
        onSelect={vi.fn()}
      />,
    );

    const row = screen.getByRole("button", { name: "Open Live thread" });
    expect(row).toHaveTextContent("Live thread");
    expect(row).toHaveTextContent("Working");
    expect(row).toHaveTextContent("1m");
    expect(container.querySelector(".pairing-row-chevron")).toBeInTheDocument();
  });
});

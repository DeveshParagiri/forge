import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RemoteClientAction } from "../src/reducer";
import { writePairings } from "../src/pairingRegistry";
import { sessionFixture } from "./fixtures";

const socketHarness = vi.hoisted(() => ({
  instances: [] as Array<{ baseUrl: string; dispatch(action: RemoteClientAction): void }>,
  onConnect: undefined as
    | ((socket: { baseUrl: string; dispatch(action: RemoteClientAction): void }) => void)
    | undefined,
  newSession: vi.fn(),
  acceptNewSession: vi.fn(),
}));

vi.mock("../src/remoteSocket", () => {
  class FakeForgeRemoteSocket {
    constructor(
      readonly dispatch: (action: RemoteClientAction) => void,
      readonly baseUrl: string,
    ) {
      socketHarness.instances.push(this);
    }

    connect() {
      socketHarness.onConnect?.(this);
    }

    stop() {}
    relinquish() { return Promise.resolve(); }
    sendPrompt() { return null; }
    cancel() { return null; }
    setModel() { return null; }
    setFastMode() { return null; }
    askBtw() { return null; }
    refreshUsage() { return null; }
    resolveInteraction() { return null; }
    editQueuedPrompt() { return null; }
    steerQueuedPrompt() { return null; }
    cancelQueuedPrompt() { return null; }
    newSession() { return socketHarness.newSession(this); }
    acceptNewSession(sessionId: string) {
      return socketHarness.acceptNewSession(this, sessionId);
    }
    resync() { return null; }
  }

  return {
    ForgeRemoteSocket: FakeForgeRemoteSocket,
    bindRemoteSocketVisibility: (socket: FakeForgeRemoteSocket) => {
      socket.connect();
      return {
        activate: () => socket.connect(),
        dispose: () => socket.stop(),
        whenSettled: () => Promise.resolve(),
      };
    },
  };
});

import { App } from "../src/App";

const TOKEN_A = "a".repeat(64);
const TOKEN_B = "b".repeat(64);
const storageValues = new Map<string, string>();
Object.defineProperty(window, "localStorage", {
  configurable: true,
  value: {
    getItem: (key: string) => storageValues.get(key) ?? null,
    setItem: (key: string, value: string) => storageValues.set(key, value),
    removeItem: (key: string) => storageValues.delete(key),
    clear: () => storageValues.clear(),
    key: (index: number) => [...storageValues.keys()][index] ?? null,
    get length() { return storageValues.size; },
  } satisfies Storage,
});

function deliverLiveSnapshot(
  socket: { dispatch(action: RemoteClientAction): void },
  sessionId: string,
  title: string,
) {
  socket.dispatch({ type: "socketConnecting", reconnecting: false, attempt: 0 });
  socket.dispatch({ type: "socketOpen" });
  socket.dispatch({
    type: "serverMessage",
    message: {
      type: "connected",
      protocolVersion: 1,
      sessionId,
      expiresAt: "2030-01-02T00:00:00Z",
    },
  });
  socket.dispatch({
    type: "serverMessage",
    message: {
      type: "snapshot",
      protocolVersion: 1,
      revision: 1,
      session: sessionFixture({
        sessionId,
        title,
        cwd: "/Users/dev/Projects/forge",
        capabilities: {
          ...sessionFixture().capabilities,
          newSession: true,
        },
      }),
    },
  });
}

describe("Forge browser pairing actions", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/");
    window.localStorage.clear();
    socketHarness.instances.length = 0;
    socketHarness.onConnect = undefined;
    socketHarness.newSession.mockReset();
    socketHarness.acceptNewSession.mockReset();
  });

  it("uses the Home plus for a real same-origin private-link form", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Create new Forge session" }));
    expect(screen.getByRole("dialog", { name: "Add Forge session" })).toBeVisible();
    expect(screen.getByText(/stays in this browser's local storage/i)).toBeVisible();

    const input = screen.getByLabelText("Private /rc link");
    fireEvent.change(input, {
      target: { value: `https://another.example/forge/${TOKEN_A}/` },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add session" }));
    expect(screen.getByRole("alert")).toHaveTextContent(window.location.origin);

    fireEvent.change(input, {
      target: { value: `${window.location.origin}/forge/${TOKEN_A}/` },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add session" }));

    expect(screen.queryByRole("dialog", { name: "Add Forge session" })).not.toBeInTheDocument();
    expect(window.localStorage.getItem("forge.remote.pairings.v1")).toContain(TOKEN_A);
  });

  it("closes the add-session sheet with Escape and returns focus to its trigger", async () => {
    render(<App />);

    const trigger = screen.getByRole("button", { name: "Create new Forge session" });
    trigger.focus();
    fireEvent.click(trigger);
    const dialog = screen.getByRole("dialog", { name: "Add Forge session" });
    expect(dialog).toBeVisible();
    await waitFor(() => expect(screen.getByLabelText("Private /rc link")).toHaveFocus());

    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByRole("dialog", { name: "Add Forge session" })).not.toBeInTheDocument();
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("validates and accepts a new project session before navigating to it", async () => {
    const sourceUrl = `${window.location.origin}/forge/${TOKEN_A}/`;
    const childUrl = `${window.location.origin}/forge/${TOKEN_B}/`;
    writePairings([
      {
        id: "source",
        baseUrl: sourceUrl,
        addedAt: "2030-01-01T00:00:00Z",
        lastSeenAt: "2030-01-01T00:01:00Z",
        sessionId: "session-source",
        title: "Source session",
        cwd: "/Users/dev/Projects/forge",
      },
    ]);
    socketHarness.onConnect = (socket) => {
      if (socket.baseUrl === sourceUrl) {
        deliverLiveSnapshot(socket, "session-source", "Source session");
      } else if (socket.baseUrl === childUrl) {
        deliverLiveSnapshot(socket, "session-child", "Child session");
      }
    };
    socketHarness.newSession.mockResolvedValue({
      sessionId: "session-child",
      pairingUrl: childUrl,
      expiresAt: "2030-01-02T00:00:00Z",
    });
    socketHarness.acceptNewSession.mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Create new session in forge" }));

    await waitFor(() => expect(socketHarness.acceptNewSession).toHaveBeenCalledOnce());
    expect(socketHarness.acceptNewSession).toHaveBeenCalledWith(
      expect.objectContaining({ baseUrl: sourceUrl }),
      "session-child",
    );
    expect(await screen.findByText("Child session")).toBeVisible();
    const stored = window.localStorage.getItem("forge.remote.pairings.v1");
    expect(stored).toContain("session-child");
    expect(stored).toContain(TOKEN_B);
  });

  it("rejects a child bearer for the wrong session and removes its provisional row", async () => {
    const sourceUrl = `${window.location.origin}/forge/${TOKEN_A}/`;
    const childUrl = `${window.location.origin}/forge/${TOKEN_B}/`;
    writePairings([
      {
        id: "source",
        baseUrl: sourceUrl,
        addedAt: "2030-01-01T00:00:00Z",
        sessionId: "session-source",
        title: "Source session",
        cwd: "/Users/dev/Projects/forge",
      },
    ]);
    socketHarness.onConnect = (socket) => {
      if (socket.baseUrl === sourceUrl) {
        deliverLiveSnapshot(socket, "session-source", "Source session");
      } else if (socket.baseUrl === childUrl) {
        deliverLiveSnapshot(socket, "wrong-session", "Wrong session");
      }
    };
    socketHarness.newSession.mockResolvedValue({
      sessionId: "session-child",
      pairingUrl: childUrl,
      expiresAt: "2030-01-02T00:00:00Z",
    });

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Create new session in forge" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("different Forge session");
    expect(socketHarness.acceptNewSession).not.toHaveBeenCalled();
    expect(window.localStorage.getItem("forge.remote.pairings.v1")).not.toContain(TOKEN_B);
    expect(screen.getByRole("button", { name: "Open Source session" })).toBeVisible();
  });
});

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { InteractionResponse } from "../src/protocol";
import type { StoredPairing } from "../src/pairingRegistry";
import type { RemoteClientState } from "../src/reducer";
import type { ForgeRemoteCommands } from "../src/remoteSocket";
import { parseComposerSubmission } from "../src/t3-adapted/ChatComposer";
import { ChatView, usageNeedsRefresh } from "../src/t3-adapted/ChatView";
import { PairingHandoff } from "../src/t3-adapted/PairingHandoff";
import { PairingsHome } from "../src/t3-adapted/PairingsHome";
import { sessionFixture, usageFixture } from "./fixtures";
import "../src/styles.css";

function commandsFixture(): ForgeRemoteCommands {
  return {
    sendPrompt: vi.fn(() => "prompt-1"),
    cancel: vi.fn(() => "cancel-1"),
    setModel: vi.fn(() => "model-1"),
    setFastMode: vi.fn(() => "fast-mode-1"),
    askBtw: vi.fn(() => "btw-1"),
    refreshUsage: vi.fn(() => "usage-1"),
    resolveInteraction: vi.fn(() => "interaction-1"),
    editQueuedPrompt: vi.fn(() => "queue-edit-1"),
    steerQueuedPrompt: vi.fn(() => "queue-steer-1"),
    cancelQueuedPrompt: vi.fn(() => "queue-cancel-1"),
    newSession: vi.fn(() =>
      Promise.resolve({
        sessionId: "session-new",
        pairingUrl: `https://forge.example/forge/${"b".repeat(64)}/`,
        expiresAt: "2030-01-01T00:00:00Z",
      }),
    ),
    acceptNewSession: vi.fn(() => Promise.resolve()),
    resync: vi.fn(() => "resync-1"),
  };
}

function liveState(overrides: Partial<RemoteClientState> = {}): RemoteClientState {
  return {
    phase: "live",
    sessionId: "session-123",
    expiresAt: "2030-01-01T00:00:00Z",
    revision: 7,
    session: sessionFixture(),
    pendingCommands: {},
    needsResync: false,
    reconnectAttempt: 0,
    ...overrides,
  };
}

function openComposerSettings(): HTMLElement {
  fireEvent.focus(screen.getByLabelText("Message Forge"));
  const trigger = screen.getByLabelText(/^Model and reasoning, current /);
  fireEvent.click(trigger);
  expect(trigger.closest("details")).toHaveAttribute("open");
  return trigger;
}

describe("T3-derived Forge conversation surface", () => {
  it("renders the authoritative transcript and server-provided controls", () => {
    render(<ChatView state={liveState()} commands={commandsFixture()} />);
    expect(screen.getByText("Keep the terminal active.")).toBeVisible();
    expect(screen.getByText("Forge session")).toBeVisible();
    openComposerSettings();
    expect(screen.getByRole("group", { name: "Model" })).toBeVisible();
    expect(
      within(screen.getByRole("group", { name: "Model" })).getByRole("button", {
        name: "5.6 Sol",
        pressed: true,
      }),
    ).toBeVisible();
    expect(screen.getByRole("group", { name: "Reasoning" })).toBeVisible();
    expect(
      within(screen.getByRole("group", { name: "Reasoning" })).getByRole("button", {
        name: "Medium",
        pressed: true,
      }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Usage" })).toBeVisible();
    expect(screen.getByText("Remote feature")).toBeVisible();
    expect(document.querySelector('.session-connection-dot[data-connection-state="connected"]')).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent("forge · GPT-5.6 Sol");
    expect(document.body).not.toHaveTextContent("Live · Private");
    expect(document.body).not.toHaveTextContent("Expires");
    expect(document.body).not.toHaveTextContent("Tailnet only");
    expect(document.querySelector(".session-status-summary")).not.toBeInTheDocument();
    expect(document.querySelector(".connection-chip")).not.toBeInTheDocument();
    expect(document.querySelector(".private-label")).not.toBeInTheDocument();
    expect(document.querySelector(".connection-summary")).not.toBeInTheDocument();
    expect(document.querySelector(".detail-control")).not.toBeInTheDocument();
  });

  it("keeps entered composer text explicitly visible on WebKit", () => {
    render(<ChatView state={liveState()} commands={commandsFixture()} />);
    const editor = screen.getByLabelText("Message Forge");
    fireEvent.change(editor, { target: { value: "Visible above the keyboard" } });
    const style = getComputedStyle(editor);
    expect(style.color).toBe("var(--foreground)");
    expect(style.getPropertyValue("-webkit-text-fill-color")).toBe("var(--foreground)");
    expect(editor).toHaveValue("Visible above the keyboard");
  });

  it("grows the multiline editor with its content and caps it at 112 pixels", () => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 1;
    });
    render(<ChatView state={liveState()} commands={commandsFixture()} />);
    const editor = screen.getByLabelText("Message Forge");
    let scrollHeight = 72;
    Object.defineProperty(editor, "scrollHeight", {
      configurable: true,
      get: () => scrollHeight,
    });

    fireEvent.change(editor, { target: { value: "First line\nSecond line\nThird line" } });
    expect(editor).toHaveStyle({ height: "72px", overflowY: "hidden" });
    expect(editor.closest(".composer-dock")).toHaveAttribute("data-expanded", "true");

    scrollHeight = 180;
    fireEvent.change(editor, {
      target: { value: "First line\nSecond line\nThird line\nFourth line\nFifth line" },
    });
    expect(editor).toHaveStyle({ height: "112px", overflowY: "auto" });
  });

  it("provides the T3-style back affordance without rendering the bearer token", () => {
    const onBack = vi.fn();
    const bearer = "a".repeat(64);
    render(<ChatView state={liveState()} commands={commandsFixture()} onBack={onBack} />);
    fireEvent.click(screen.getByRole("button", { name: "Back to sessions" }));
    expect(onBack).toHaveBeenCalledOnce();
    expect(document.body).not.toHaveTextContent(bearer);
  });

  it("sends plain text but does not optimistically append it", () => {
    const commands = commandsFixture();
    render(<ChatView state={liveState()} commands={commands} />);
    const editor = screen.getByLabelText("Message Forge");
    fireEvent.change(editor, { target: { value: "Continue from my phone" } });
    fireEvent.click(screen.getByLabelText("Send message"));
    expect(commands.sendPrompt).toHaveBeenCalledWith("Continue from my phone", []);
    expect(screen.queryByText("Continue from my phone")).not.toBeInTheDocument();
  });

  it("offers distinct Photos and Files actions and sends an image-only payload", async () => {
    const commands = commandsFixture();
    render(<ChatView state={liveState()} commands={commands} />);
    fireEvent.focus(screen.getByLabelText("Message Forge"));
    fireEvent.click(screen.getByLabelText("Add photos or files"));
    const photosAction = screen.getByRole("button", { name: "Photos" });
    const filesAction = screen.getByRole("button", { name: "Files" });
    expect(photosAction.querySelector(".lucide-image")).toBeInTheDocument();
    expect(filesAction.querySelector(".lucide-file-text")).toBeInTheDocument();

    const photosInput = screen.getByLabelText("Choose photos") as HTMLInputElement;
    const filesInput = screen.getByLabelText("Choose image files") as HTMLInputElement;
    const openPhotos = vi.spyOn(photosInput, "click");
    const openFiles = vi.spyOn(filesInput, "click");
    fireEvent.click(photosAction);
    fireEvent.click(filesAction);
    expect(openPhotos).toHaveBeenCalledOnce();
    expect(openFiles).toHaveBeenCalledOnce();
    expect(photosInput).toHaveAttribute("accept", "image/*");
    expect(filesInput).toHaveAttribute("accept", "image/*");

    const image = new File(["pixels"], "release.png", { type: "image/png" });
    fireEvent.change(filesInput, { target: { files: [image] } });
    await waitFor(() => expect(screen.getByLabelText("Attached images")).toBeVisible());
    expect(screen.getByLabelText("Send message")).toBeEnabled();
    fireEvent.click(screen.getByLabelText("Send message"));
    await waitFor(() =>
      expect(commands.sendPrompt).toHaveBeenCalledWith("", [
        {
          name: "release.png",
          mimeType: "image/png",
          data: "cGl4ZWxz",
        },
      ]),
    );
  });

  it("lets the target model choose its own default reasoning effort", () => {
    const commands = commandsFixture();
    render(<ChatView state={liveState()} commands={commands} />);
    openComposerSettings();
    fireEvent.click(screen.getByRole("button", { name: "5.6 Terra" }));
    expect(commands.setModel).toHaveBeenCalledWith("gpt-5.6-terra", null);
  });

  it("shows Fast mode only when both capability and authoritative support allow it", () => {
    const commands = commandsFixture();
    const capabilityDisabled = sessionFixture({
      capabilities: { ...sessionFixture().capabilities, fastMode: false },
    });
    const { rerender } = render(
      <ChatView state={liveState({ session: capabilityDisabled })} commands={commands} />,
    );
    openComposerSettings();
    expect(screen.queryByRole("button", { name: "Fast mode" })).not.toBeInTheDocument();

    rerender(
      <ChatView
        state={liveState({ session: sessionFixture({ fastMode: { supported: false, enabled: false } }) })}
        commands={commands}
      />,
    );
    expect(screen.queryByRole("button", { name: "Fast mode" })).not.toBeInTheDocument();

    rerender(<ChatView state={liveState()} commands={commands} />);
    const fastMode = screen.getByRole("button", { name: "Fast mode" });
    expect(fastMode).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(fastMode);
    expect(commands.setFastMode).toHaveBeenCalledWith(true);
  });

  it("shows exactly one Stop action while the same session is running", () => {
    const commands = commandsFixture();
    const state = liveState({ session: sessionFixture({ status: "running" }) });
    render(<ChatView state={state} commands={commands} />);
    const actions = document.querySelector(".composer-primary-actions");
    expect(actions).not.toBeNull();
    expect(within(actions as HTMLElement).getAllByRole("button")).toHaveLength(1);
    expect(screen.queryByRole("tab")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Stop current turn")).toBeEnabled();
    fireEvent.click(screen.getByLabelText("Stop current turn"));
    expect(commands.cancel).toHaveBeenCalledOnce();
  });

  it("parses a leading-space /btw command and keeps it sendable while running", () => {
    const commands = commandsFixture();
    const state = liveState({ session: sessionFixture({ status: "running" }) });
    render(<ChatView state={state} commands={commands} />);
    fireEvent.change(screen.getByLabelText("Message Forge"), {
      target: { value: "   /btw   did the tests finish?   " },
    });
    expect(screen.queryByLabelText("Stop current turn")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Ask side question")).toBeEnabled();
    fireEvent.click(screen.getByLabelText("Ask side question"));
    expect(commands.askBtw).toHaveBeenCalledWith("did the tests finish?");
    expect(commands.sendPrompt).not.toHaveBeenCalled();
  });

  it("traps focus in the queued-message editor and restores the action on Escape", async () => {
    const session = sessionFixture({
      queue: [
        {
          id: "queued-for-edit",
          text: "Verify release",
          position: 0,
          source: "shared",
          version: 3,
          actions: { edit: true, steer: true, cancel: true },
        },
      ],
    });
    render(<ChatView state={liveState({ session })} commands={commandsFixture()} />);

    fireEvent.click(screen.getByLabelText("Queued message actions"));
    const editAction = screen.getByRole("button", { name: "Edit message" });
    editAction.focus();
    fireEvent.click(editAction);
    const dialog = screen.getByRole("dialog", { name: "Edit message" });
    const editor = within(dialog).getByRole("textbox");
    await waitFor(() => expect(editor).toHaveFocus());

    const save = within(dialog).getByRole("button", { name: "Save" });
    save.focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(editor).toHaveFocus();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Edit message" })).not.toBeInTheDocument();
    await waitFor(() => expect(editAction).toHaveFocus());
  });

  it("does not submit a bare /btw command", () => {
    const commands = commandsFixture();
    render(<ChatView state={liveState()} commands={commands} />);
    fireEvent.change(screen.getByLabelText("Message Forge"), { target: { value: " /btw " } });
    expect(screen.getByLabelText("Ask side question")).toBeDisabled();
    fireEvent.click(screen.getByLabelText("Ask side question"));
    expect(commands.askBtw).not.toHaveBeenCalled();
    expect(commands.sendPrompt).not.toHaveBeenCalled();
  });

  it("intercepts an exact whitespace-tolerant /usage locally while work is running", () => {
    const commands = commandsFixture();
    const state = liveState({ session: sessionFixture({ status: "running", usage: undefined }) });
    render(<ChatView state={state} commands={commands} />);
    fireEvent.change(screen.getByLabelText("Message Forge"), {
      target: { value: "   /usage   " },
    });
    expect(screen.queryByLabelText("Stop current turn")).not.toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("Check usage"));
    expect(screen.getByRole("dialog", { name: "Usage" })).toBeVisible();
    expect(commands.refreshUsage).toHaveBeenCalledOnce();
    expect(commands.sendPrompt).not.toHaveBeenCalled();
    expect(commands.askBtw).not.toHaveBeenCalled();
  });

  it("treats only the exact /usage command as local", () => {
    expect(parseComposerSubmission("\n /usage \t")).toEqual({ type: "usage" });
    expect(parseComposerSubmission("/usage now")).toEqual({ type: "prompt", text: "/usage now" });
    expect(parseComposerSubmission("/USAGE")).toEqual({ type: "prompt", text: "/USAGE" });
  });

  it("opens fresh cached usage without forcing a refresh and separates all three surfaces", () => {
    const commands = commandsFixture();
    const fresh = usageFixture({ refreshedAt: new Date(Date.now()).toISOString() });
    render(
      <ChatView
        state={liveState({ session: sessionFixture({ usage: fresh }) })}
        commands={commands}
      />,
    );
    openComposerSettings();
    fireEvent.click(screen.getByRole("button", { name: "Usage" }));
    expect(commands.refreshUsage).not.toHaveBeenCalled();
    expect(screen.getByRole("tabpanel", { name: "Context" })).toHaveTextContent("48%");
    fireEvent.click(screen.getByRole("tab", { name: "This session" }));
    expect(screen.getByRole("tabpanel", { name: "This session" })).toHaveTextContent("30.0K");
    fireEvent.click(screen.getByRole("tab", { name: "Account limits" }));
    expect(screen.getByRole("tabpanel", { name: "Account limits" })).toHaveTextContent("Weekly limit");
    fireEvent.click(screen.getByRole("button", { name: "Refresh usage" }));
    expect(commands.refreshUsage).toHaveBeenCalledOnce();
  });

  it("keeps cached values visible while a usage refresh is loading", () => {
    const commands = commandsFixture();
    const state = liveState({
      session: sessionFixture({ usage: usageFixture({ status: "loading" }) }),
    });
    render(<ChatView state={state} commands={commands} />);
    openComposerSettings();
    fireEvent.click(screen.getByRole("button", { name: "Usage" }));
    expect(screen.getByText("Refreshing usage…")).toBeVisible();
    expect(screen.getByRole("tabpanel", { name: "Context" })).toHaveTextContent("48%");
  });

  it("labels cached refresh failures and never presents partial cost as authoritative", () => {
    const commands = commandsFixture();
    const state = liveState({
      session: sessionFixture({
        usage: usageFixture({
          status: "error",
          session: {
            ...usageFixture().session!,
            costState: "partial",
            incomplete: true,
          },
        }),
      }),
    });
    render(<ChatView state={state} commands={commands} />);
    openComposerSettings();
    fireEvent.click(screen.getByRole("button", { name: "Usage" }));
    expect(screen.getByText("Usage could not be refreshed. Showing cached data.")).toBeVisible();
    fireEvent.click(screen.getByRole("tab", { name: "This session" }));
    expect(screen.getByRole("tabpanel", { name: "This session" })).toHaveTextContent("Unavailable");
    expect(screen.getByRole("tabpanel", { name: "This session" })).toHaveTextContent(/partial/i);
  });

  it("gates usage controls and the local command behind the server capability", () => {
    const commands = commandsFixture();
    const session = sessionFixture({
      capabilities: { ...sessionFixture().capabilities, usage: false },
    });
    render(<ChatView state={liveState({ session })} commands={commands} />);
    expect(screen.queryByRole("button", { name: "Usage" })).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Message Forge"), { target: { value: "/usage" } });
    expect(screen.getByLabelText("Check usage")).toBeDisabled();
    fireEvent.click(screen.getByLabelText("Check usage"));
    expect(commands.refreshUsage).not.toHaveBeenCalled();
    expect(commands.sendPrompt).not.toHaveBeenCalled();
  });

  it("refreshes absent, failed, malformed, or stale usage but not fresh/loading usage", () => {
    const now = Date.parse("2030-01-01T00:02:00Z");
    expect(usageNeedsRefresh(sessionFixture({ usage: undefined }), now)).toBe(true);
    expect(usageNeedsRefresh(sessionFixture({ usage: usageFixture({ status: "error" }) }), now)).toBe(true);
    expect(usageNeedsRefresh(sessionFixture({ usage: usageFixture({ refreshedAt: "bad" }) }), now)).toBe(true);
    expect(usageNeedsRefresh(sessionFixture({ usage: usageFixture({ refreshedAt: "2030-01-01T00:00:00Z" }) }), now)).toBe(true);
    expect(usageNeedsRefresh(sessionFixture({ usage: usageFixture({ refreshedAt: "2030-01-01T00:01:30Z" }) }), now)).toBe(false);
    expect(usageNeedsRefresh(sessionFixture({ usage: usageFixture({ status: "loading" }) }), now)).toBe(false);
  });

  it("disables input but preserves visible context while reconnecting", () => {
    render(
      <ChatView
        state={liveState({ phase: "reconnecting", lastError: "Network changed" })}
        commands={commandsFixture()}
      />,
    );
    expect(screen.getByText("Keep the terminal active.")).toBeVisible();
    expect(screen.getByText(/Work continues in the terminal/)).toBeVisible();
    expect(screen.getByLabelText("Message Forge")).toBeDisabled();
  });

  it("submits an opaque permission option through the shared interaction", () => {
    const commands = commandsFixture();
    const state = liveState({
      session: sessionFixture({
        activeInteractions: [
          {
            interactionId: "permission:opaque",
            kind: "permission",
            title: "Allow this command?",
            description: "pnpm test",
            options: [
              { id: "once", label: "Allow once" },
              { id: "deny", label: "Deny" },
            ],
          },
        ],
      }),
    });
    render(<ChatView state={state} commands={commands} />);
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    expect(commands.resolveInteraction).toHaveBeenCalledWith("permission:opaque", {
      kind: "permission",
      optionId: "once",
    } satisfies InteractionResponse);
  });

  it("collects question options and freeform answers in the typed response", () => {
    const commands = commandsFixture();
    const state = liveState({
      session: sessionFixture({
        activeInteractions: [
          {
            interactionId: "question:opaque",
            kind: "question",
            questions: [
              {
                prompt: "Which target?",
                options: [{ label: "Gateway" }, { label: "Pager" }],
                allowFreeform: true,
              },
            ],
          },
        ],
      }),
    });
    render(<ChatView state={state} commands={commands} />);
    const card = screen.getByRole("group", { name: "Which target?" });
    fireEvent.click(within(card).getByLabelText("Pager"));
    fireEvent.change(screen.getByLabelText("Your answer"), { target: { value: "Keep it scoped" } });
    fireEvent.click(screen.getByRole("button", { name: "Send answer" }));
    expect(commands.resolveInteraction).toHaveBeenCalledWith("question:opaque", {
      kind: "question",
      answers: [{ questionIndex: 0, optionIndices: [], freeform: "Keep it scoped" }],
    });
  });

  it("clears single-select freeform when an advertised option is chosen", () => {
    const commands = commandsFixture();
    const state = liveState({
      session: sessionFixture({
        activeInteractions: [
          {
            interactionId: "question:exclusive",
            kind: "question",
            questions: [
              {
                prompt: "Which target?",
                options: [{ label: "Gateway" }, { label: "Pager" }],
                allowFreeform: true,
              },
            ],
          },
        ],
      }),
    });
    render(<ChatView state={state} commands={commands} />);
    fireEvent.change(screen.getByLabelText("Your answer"), { target: { value: "Custom" } });
    fireEvent.click(screen.getByLabelText("Pager"));
    fireEvent.click(screen.getByRole("button", { name: "Send answer" }));
    expect(commands.resolveInteraction).toHaveBeenCalledWith("question:exclusive", {
      kind: "question",
      answers: [{ questionIndex: 0, optionIndices: [1] }],
    });
  });

  it("sends plan change feedback as a cancelled plan outcome", () => {
    const commands = commandsFixture();
    const state = liveState({
      session: sessionFixture({
        activeInteractions: [
          {
            interactionId: "plan:opaque",
            kind: "plan",
            plan: "1. Inspect\n2. Implement",
            allowFeedback: true,
          },
        ],
      }),
    });
    render(<ChatView state={state} commands={commands} />);
    fireEvent.change(screen.getByLabelText("Changes for Forge"), {
      target: { value: "Add a rollback check" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Request changes" }));
    expect(commands.resolveInteraction).toHaveBeenCalledWith("plan:opaque", {
      kind: "plan",
      outcome: "cancelled",
      feedback: "Add a rollback check",
    } satisfies InteractionResponse);
  });

  it("renders revocation as terminal state instead of reconnecting", () => {
    render(
      <ChatView
        state={liveState({ phase: "revoked", revocationReason: "expired" })}
        commands={commandsFixture()}
      />,
    );
    expect(screen.getByRole("heading", { name: "This private link expired" })).toBeVisible();
    expect(screen.queryByText("Keep the terminal active.")).not.toBeInTheDocument();
  });
});

describe("Forge session index", () => {
  const pairing: StoredPairing = {
    id: "pairing-one",
    baseUrl: `https://forge.example/forge/${"a".repeat(64)}/`,
    addedAt: "2030-01-01T00:00:00Z",
    lastSeenAt: "2030-01-01T00:01:00Z",
    title: "Keep the browser quiet",
    cwd: "/workspace/forge",
    status: "running",
    modelLabel: "GPT-5.6 Sol",
  };

  it("uses a native-like project header and compact thread row", () => {
    const { container } = render(
      <PairingsHome pairings={[pairing]} onSelect={vi.fn()} onRemove={vi.fn()} />,
    );
    expect(screen.getByText("forge")).toBeVisible();
    expect(screen.getByText("Keep the browser quiet")).toBeVisible();
    expect(screen.getByText("Working")).toBeVisible();
    expect(screen.queryByText("/workspace/forge")).not.toBeInTheDocument();
    expect(screen.queryByText("GPT-5.6 Sol")).not.toBeInTheDocument();
    expect(container.querySelector(".pairing-project-mark")).not.toBeInTheDocument();
    expect(screen.queryByText("ALPHA")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/saved pairings/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Remove Keep the browser quiet/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /More options for Keep the browser quiet/ })).toBeVisible();
  });

  it("keeps the empty state typographic instead of substituting another fake icon", () => {
    const { container } = render(
      <PairingsHome pairings={[]} onSelect={vi.fn()} onRemove={vi.fn()} />,
    );
    expect(screen.getByRole("heading", { name: "No Forge sessions yet" })).toBeVisible();
    expect(container.querySelector(".pairing-project-mark")).not.toBeInTheDocument();
  });

  it("uses the plain Forge wordmark during native handoff", () => {
    render(
      <PairingHandoff nativeClaimed={false} onOpenNative={vi.fn()} onContinueBrowser={vi.fn()} />,
    );
    expect(screen.getByLabelText("Forge")).toHaveTextContent("Forge");
    expect(screen.queryByText("ALPHA")).not.toBeInTheDocument();
  });
});

import { fireEvent, render, screen, within } from "@testing-library/react";
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
    askBtw: vi.fn(() => "btw-1"),
    refreshUsage: vi.fn(() => "usage-1"),
    resolveInteraction: vi.fn(() => "interaction-1"),
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

describe("T3-derived Forge conversation surface", () => {
  it("renders the authoritative transcript and server-provided controls", () => {
    render(<ChatView state={liveState()} commands={commandsFixture()} />);
    expect(screen.getByText("Keep the terminal active.")).toBeVisible();
    expect(screen.getByText("Forge session")).toBeVisible();
    expect(screen.getByText("Model")).toBeVisible();
    expect(screen.getByLabelText("Model")).toHaveValue("gpt-5.6-sol");
    expect(screen.getByLabelText("Reasoning effort")).toHaveValue("medium");
    expect(screen.getByRole("button", { name: "Usage" })).toBeVisible();
    expect(screen.getByText("forge · GPT-5.6 Sol")).toBeVisible();
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
    expect(commands.sendPrompt).toHaveBeenCalledWith("Continue from my phone");
    expect(screen.queryByText("Continue from my phone")).not.toBeInTheDocument();
  });

  it("lets the target model choose its own default reasoning effort", () => {
    const commands = commandsFixture();
    render(<ChatView state={liveState()} commands={commands} />);
    fireEvent.change(screen.getByLabelText("Model"), { target: { value: "gpt-5.6-terra" } });
    expect(commands.setModel).toHaveBeenCalledWith("gpt-5.6-terra", null);
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

  it("keeps the T3 row hierarchy without an invented project badge", () => {
    const { container } = render(
      <PairingsHome pairings={[pairing]} onSelect={vi.fn()} onRemove={vi.fn()} />,
    );
    expect(screen.getByText("forge")).toBeVisible();
    expect(screen.getByText("Keep the browser quiet")).toBeVisible();
    expect(screen.getByText("/workspace/forge")).toBeVisible();
    expect(screen.getByText("GPT-5.6 Sol")).toBeVisible();
    expect(container.querySelector(".pairing-project-mark")).not.toBeInTheDocument();
    expect(screen.queryByText("ALPHA")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/saved pairings/i)).not.toBeInTheDocument();
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

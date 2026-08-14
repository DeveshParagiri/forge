import { createRoot } from "react-dom/client";
import { ChatView } from "../src/t3-adapted/ChatView";
import type { ForgeRemoteCommands } from "../src/remoteSocket";
import type { RemoteClientState } from "../src/reducer";
import { sessionFixture } from "./fixtures";
import { bindVisualViewport } from "../src/visualViewport";
import "../src/styles.css";

const commands: ForgeRemoteCommands = {
  sendPrompt: () => "fixture-prompt",
  cancel: () => "fixture-cancel",
  setModel: () => "fixture-model",
  setFastMode: () => "fixture-fast-mode",
  askBtw: () => "fixture-btw",
  refreshUsage: () => "fixture-usage",
  resolveInteraction: () => "fixture-interaction",
  editQueuedPrompt: () => "fixture-queue-edit",
  steerQueuedPrompt: () => "fixture-queue-steer",
  cancelQueuedPrompt: () => "fixture-queue-cancel",
  newSession: () => Promise.resolve({
    sessionId: "session-new",
    pairingUrl: `https://forge.example/forge/${"b".repeat(64)}/`,
    expiresAt: "2030-01-01T00:00:00Z",
  }),
  acceptNewSession: () => Promise.resolve(),
  resync: () => "fixture-resync",
};

const state: RemoteClientState = {
  phase: "live",
  sessionId: "session-123",
  expiresAt: new Date(Date.now() + 8 * 60 * 60 * 1000).toISOString(),
  revision: 18,
  session: sessionFixture({
    title: "Forge remote browser client",
    status: "idle",
    transcript: [
      { id: "u1", kind: "user", text: "Continue implementing `/rc` and preserve the live terminal session.", status: "complete" },
      {
        id: "thought-1",
        kind: "reasoning",
        text: "Checked the browser and phone presentation contracts.",
        status: "complete",
      },
      {
        id: "tool-1",
        kind: "tool",
        title: "Run command",
        detail: "pnpm test",
        output: "104 tests passed",
        status: "complete",
      },
      {
        id: "worked-1",
        kind: "system",
        text: "Worked for 3m 41s",
        workDisclosure: {
          durationMs: 221_000,
          workItemIds: ["thought-1", "tool-1"],
          finalResponseItemId: "a1",
        },
      },
      { id: "a1", kind: "assistant", text: "The browser is now attached to the **same Forge session**.\n\nExisting context remains authoritative, and the terminal and phone stay usable together.", status: "complete" },
    ],
    taskState: undefined,
    queue: [
      {
        id: "q1",
        text: "Also verify the browser composer on a phone-sized viewport.",
        position: 0,
        source: "shared",
        version: 1,
        actions: { edit: true, steer: true, cancel: true },
      },
    ],
    activeInteractions: [],
  }),
  pendingCommands: {},
  needsResync: false,
  reconnectAttempt: 0,
};

const root = document.getElementById("root");
if (!root) throw new Error("Visual fixture root was not found");
bindVisualViewport();
createRoot(root).render(
  <ChatView
    state={state}
    commands={commands}
    onBack={() => {}}
    onCreateSession={() => {}}
  />,
);
if (new URLSearchParams(window.location.search).get("usage") === "open") {
  window.requestAnimationFrame(() => {
    document.querySelector<HTMLElement>('summary[aria-label^="Model and reasoning"]')?.click();
    window.requestAnimationFrame(() => {
      Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
        .find((button) => button.textContent?.trim() === "Usage")
        ?.click();
    });
  });
}

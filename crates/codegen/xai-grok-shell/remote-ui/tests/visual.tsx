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
  askBtw: () => "fixture-btw",
  refreshUsage: () => "fixture-usage",
  resolveInteraction: () => "fixture-interaction",
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
      { id: "a1", kind: "assistant", text: "The browser is now attached to the **same Forge session**.\n\nExisting context remains authoritative, and the terminal and phone stay usable together.", status: "complete" },
    ],
    taskState: undefined,
    queue: [],
    activeInteractions: [],
  }),
  pendingCommands: {},
  needsResync: false,
  reconnectAttempt: 0,
};

const root = document.getElementById("root");
if (!root) throw new Error("Visual fixture root was not found");
bindVisualViewport();
createRoot(root).render(<ChatView state={state} commands={commands} onBack={() => {}} />);
if (new URLSearchParams(window.location.search).get("usage") === "open") {
  window.requestAnimationFrame(() => {
    document.querySelector<HTMLButtonElement>('button[aria-label="Usage"]')?.click();
  });
}

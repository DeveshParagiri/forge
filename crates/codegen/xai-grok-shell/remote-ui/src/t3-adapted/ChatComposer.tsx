/*
 * Adapted from T3 Code's apps/web/src/components/chat/ChatComposer.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import ArrowUp from "lucide-react/dist/esm/icons/arrow-up.js";
import ChartBar from "lucide-react/dist/esm/icons/chart-bar.js";
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down.js";
import CircleStop from "lucide-react/dist/esm/icons/circle-stop.js";
import LoaderCircle from "lucide-react/dist/esm/icons/loader-circle.js";
import { memo, useRef, useState } from "react";
import type { RemoteSessionSnapshot } from "../protocol";
import type { ForgeRemoteCommands } from "../remoteSocket";
import { ComposerPromptEditor, type ComposerPromptEditorHandle } from "./ComposerPromptEditor";

export type ComposerSubmission =
  | { type: "prompt"; text: string }
  | { type: "btw"; question: string }
  | { type: "usage" };

export function parseComposerSubmission(value: string): ComposerSubmission | null {
  const trimmed = value.trim();
  if (!trimmed || trimmed === "/btw") return null;
  if (trimmed === "/usage") return { type: "usage" };
  const btw = /^\/btw\s+([\s\S]+)$/.exec(trimmed);
  if (btw) {
    const question = btw[1]?.trim();
    return question ? { type: "btw", question } : null;
  }
  return { type: "prompt", text: trimmed };
}

export const ChatComposer = memo(function ChatComposer({
  session,
  commands,
  connected,
  commandPending,
  usagePending,
  onOpenUsage,
}: {
  session: RemoteSessionSnapshot;
  commands: ForgeRemoteCommands;
  connected: boolean;
  commandPending: boolean;
  usagePending: boolean;
  onOpenUsage(): void;
}) {
  const [prompt, setPrompt] = useState("");
  const [expanded, setExpanded] = useState(false);
  const editorRef = useRef<ComposerPromptEditorHandle>(null);
  const isRunning = session.status === "running";
  const isWorking = ["running", "waiting_for_input"].includes(session.status);
  const canPrompt = connected && session.capabilities.prompt && session.status !== "closed";
  const canBtw = connected && session.capabilities.btw && session.status !== "closed";
  const canUsage = connected && session.capabilities.usage && session.status !== "closed";
  const trimmedPrompt = prompt.trim();
  const wantsBtw = /^\/btw(?:\s|$)/.test(trimmedPrompt);
  const wantsUsage = trimmedPrompt === "/usage";
  const submission = parseComposerSubmission(prompt);
  const canSend = submission?.type === "btw"
    ? canBtw
    : submission?.type === "usage"
      ? canUsage
      : submission?.type === "prompt"
        ? canPrompt
        : false;
  const actionKind = wantsBtw || wantsUsage || !isRunning || !session.capabilities.cancel ? "send" : "stop";
  const selectedModel = session.currentModel?.id || "";
  const selectedEffort = session.reasoningEffort?.current || "";

  const submit = () => {
    if (!canSend) return;
    if (submission?.type === "usage") {
      onOpenUsage();
    } else {
      if (commandPending) return;
      const id = submission?.type === "btw"
        ? commands.askBtw(submission.question)
        : submission?.type === "prompt"
          ? commands.sendPrompt(submission.text)
          : null;
      if (!id) return;
    }
    setPrompt("");
    window.requestAnimationFrame(() => editorRef.current?.focus());
  };

  const updateModel = (modelId: string, effort: string | null) => {
    if (!modelId || !connected || session.modelSwitchPending) return;
    commands.setModel(modelId, effort);
  };

  return (
    <div className="composer-dock">
      <form
        className="chat-composer-shell"
        data-expanded={expanded || prompt.length > 0}
        aria-label="Forge message composer"
        onFocusCapture={() => setExpanded(true)}
        onBlurCapture={(event) => {
          const nextTarget = event.relatedTarget;
          if (!(nextTarget instanceof Node) || !event.currentTarget.contains(nextTarget)) {
            setExpanded(false);
          }
        }}
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <ComposerPromptEditor
          ref={editorRef}
          value={prompt}
          onChange={setPrompt}
          onSubmit={submit}
          disabled={!connected || session.status === "closed"}
          label={wantsBtw ? "Ask Forge a side question" : wantsUsage ? "Check Forge usage" : "Message Forge"}
          placeholder={
            !connected
              ? "Waiting for the private session…"
              : wantsBtw
                ? "Ask without interrupting the current work…"
                : wantsUsage
                  ? "Open usage for this session…"
                : isWorking
                  ? "Queue a message for this session…"
                  : "Message this Forge session…"
          }
        />
        <footer className="composer-footer">
          <div className="composer-selects">
            {session.capabilities.setModel && session.availableModels.length ? (
              <label className="composer-select-label composer-model-select">
                <span className="composer-control-prefix">Model</span>
                {session.modelSwitchPending ? <LoaderCircle className="status-spin" aria-hidden="true" /> : null}
                <select
                  aria-label="Model"
                  value={selectedModel}
                  disabled={!connected || session.modelSwitchPending}
                  onChange={(event) => updateModel(event.target.value, null)}
                >
                  {session.availableModels.map((model) => (
                    <option key={model.id} value={model.id}>{model.label}</option>
                  ))}
                </select>
                <ChevronDown aria-hidden="true" />
              </label>
            ) : null}
            {session.capabilities.reasoning && session.reasoningEffort?.options.length ? (
              <label className="composer-select-label composer-effort-select">
                <span className="composer-control-prefix">Effort</span>
                <select
                  aria-label="Reasoning effort"
                  value={selectedEffort}
                  disabled={!connected || session.modelSwitchPending}
                  onChange={(event) => updateModel(selectedModel, event.target.value || null)}
                >
                  {session.reasoningEffort.options.map((effort) => (
                    <option key={effort.id} value={effort.id}>{effort.label}</option>
                  ))}
                </select>
                <ChevronDown aria-hidden="true" />
              </label>
            ) : null}
            {session.capabilities.usage ? (
              <button
                type="button"
                className="composer-usage-button"
                aria-label="Usage"
                disabled={!connected || session.status === "closed"}
                onClick={onOpenUsage}
              >
                {usagePending ? <LoaderCircle className="status-spin" aria-hidden="true" /> : <ChartBar aria-hidden="true" />}
                <span>Usage</span>
              </button>
            ) : null}
          </div>
          <div className="composer-primary-actions">
            {actionKind === "stop" ? (
              <button
                type="button"
                className="send-button"
                aria-label="Stop current turn"
                disabled={!connected || commandPending}
                onClick={() => commands.cancel()}
              >
                <CircleStop aria-hidden="true" />
              </button>
            ) : (
              <button
                type="submit"
                className="send-button"
                aria-label={
                  submission?.type === "btw" || wantsBtw
                    ? "Ask side question"
                    : submission?.type === "usage" || wantsUsage
                      ? "Check usage"
                      : "Send message"
                }
                disabled={!canSend || (commandPending && submission?.type !== "usage")}
              >
                {commandPending && submission?.type !== "usage" ? (
                  <LoaderCircle className="status-spin" aria-hidden="true" />
                ) : (
                  <ArrowUp aria-hidden="true" />
                )}
                <span className="sr-only">Send</span>
              </button>
            )}
          </div>
        </footer>
      </form>
    </div>
  );
});

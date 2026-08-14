/*
 * Adapted from T3 Code's apps/web/src/components/chat/ChatComposer.tsx
 * at b73232bdd31e83914a8a943960c7dc4b6390b39b. See ../../UPSTREAM.md.
 */
import ArrowUp from "lucide-react/dist/esm/icons/arrow-up.js";
import ChartBar from "lucide-react/dist/esm/icons/chart-bar.js";
import Bolt from "lucide-react/dist/esm/icons/bolt.js";
import Check from "lucide-react/dist/esm/icons/check.js";
import FileText from "lucide-react/dist/esm/icons/file-text.js";
import Image from "lucide-react/dist/esm/icons/image.js";
import LoaderCircle from "lucide-react/dist/esm/icons/loader-circle.js";
import Plus from "lucide-react/dist/esm/icons/plus.js";
import Square from "lucide-react/dist/esm/icons/square.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { memo, useRef, useState } from "react";
import {
  readComposerImageFiles,
  toRemotePromptImages,
  type BrowserComposerAttachment,
} from "../composerAttachments";
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

function compactModelLabel(label: string): string {
  const normalized = label.trim().replace(/\s+/g, " ");
  const afterProvider = normalized.includes(" · ") ? normalized.split(" · ").at(-1) ?? normalized : normalized;
  return afterProvider.replace(/^gpt[-\s:]*(?=\d)/i, "") || label;
}

function compactReasoningLabel(label?: string): string | undefined {
  if (!label) return undefined;
  return /^(extra[ -]?high|xhigh)$/i.test(label.trim()) ? "Ultra" : label.trim();
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
  const [attachments, setAttachments] = useState<BrowserComposerAttachment[]>([]);
  const [attachmentError, setAttachmentError] = useState<string>();
  const editorRef = useRef<ComposerPromptEditorHandle>(null);
  const photosInputRef = useRef<HTMLInputElement>(null);
  const filesInputRef = useRef<HTMLInputElement>(null);
  const attachmentMenuRef = useRef<HTMLDetailsElement>(null);
  const settingsMenuRef = useRef<HTMLDetailsElement>(null);
  const isRunning = session.status === "running";
  const canPrompt = connected && session.capabilities.prompt && session.status !== "closed";
  const canBtw = connected && session.capabilities.btw && session.status !== "closed";
  const canUsage = connected && session.capabilities.usage && session.status !== "closed";
  const trimmedPrompt = prompt.trim();
  const wantsBtw = /^\/btw(?:\s|$)/.test(trimmedPrompt);
  const wantsUsage = trimmedPrompt === "/usage";
  const submission = parseComposerSubmission(prompt);
  const hasAttachments = attachments.length > 0;
  const hasDraft = trimmedPrompt.length > 0 || hasAttachments;
  const canSend = submission?.type === "btw"
    ? canBtw
    : submission?.type === "usage"
      ? canUsage
      : submission?.type === "prompt" || (!trimmedPrompt && hasAttachments)
        ? canPrompt
        : false;
  const actionKind = hasDraft || wantsBtw || wantsUsage || !isRunning || !session.capabilities.cancel ? "send" : "stop";
  const selectedModel = session.currentModel?.id || "";
  const selectedEffort = session.reasoningEffort?.current || "";
  const usagePercent = session.usage?.account?.windows[0]?.usedPercent ?? session.usage?.context?.usedPercent;

  const submit = () => {
    if (!canSend) return;
    if (submission?.type === "usage") {
      onOpenUsage();
    } else {
      if (commandPending) return;
      const id = submission?.type === "btw"
        ? commands.askBtw(submission.question)
        : submission?.type === "prompt"
          ? commands.sendPrompt(submission.text, toRemotePromptImages(attachments))
          : hasAttachments
            ? commands.sendPrompt("", toRemotePromptImages(attachments))
          : null;
      if (!id) return;
    }
    setPrompt("");
    setAttachments([]);
    window.requestAnimationFrame(() => editorRef.current?.focus());
  };

  const updateModel = (modelId: string, effort: string | null) => {
    if (!modelId || !connected || session.modelSwitchPending) return;
    commands.setModel(modelId, effort);
    settingsMenuRef.current?.removeAttribute("open");
  };

  const chooseAttachments = async (files: FileList | null) => {
    if (!files?.length) return;
    const result = await readComposerImageFiles(files, attachments.length);
    if (result.attachments.length) {
      setAttachments((current) => [...current, ...result.attachments]);
      setExpanded(true);
    }
    setAttachmentError(result.error);
    attachmentMenuRef.current?.removeAttribute("open");
  };

  const modelLabel = compactModelLabel(session.currentModel?.label || "Model");
  const reasoningOption = session.reasoningEffort?.options.find(
    (option) => option.id === selectedEffort,
  );
  const reasoningLabel = compactReasoningLabel(reasoningOption?.label ?? selectedEffort);
  const composerExpanded = expanded || prompt.length > 0 || attachments.length > 0;

  return (
    <div className="composer-dock" data-expanded={composerExpanded}>
      <form
        className="chat-composer-shell"
        data-expanded={composerExpanded}
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
        <input
          ref={photosInputRef}
          className="sr-only"
          type="file"
          accept="image/*"
          multiple
          aria-label="Choose photos"
          onChange={(event) => {
            void chooseAttachments(event.currentTarget.files);
            event.currentTarget.value = "";
          }}
        />
        <input
          ref={filesInputRef}
          className="sr-only"
          type="file"
          accept="image/*"
          multiple
          aria-label="Choose image files"
          onChange={(event) => {
            void chooseAttachments(event.currentTarget.files);
            event.currentTarget.value = "";
          }}
        />
        {attachments.length ? (
          <div className="composer-attachments" aria-label="Attached images">
            {attachments.map((attachment) => (
              <span className="composer-attachment" key={attachment.id}>
                <img src={attachment.previewUrl} alt="" />
                <button
                  type="button"
                  aria-label={`Remove ${attachment.name}`}
                  onClick={() =>
                    setAttachments((current) => current.filter((item) => item.id !== attachment.id))
                  }
                >
                  <X aria-hidden="true" />
                </button>
              </span>
            ))}
          </div>
        ) : null}
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
                : "Follow up"
          }
        />
        {attachmentError ? (
          <p className="composer-attachment-error" role="alert">{attachmentError}</p>
        ) : null}
        <footer className="composer-footer">
          <div className="composer-secondary-actions">
            <details ref={attachmentMenuRef} className="composer-menu composer-attachment-menu">
              <summary aria-label="Add photos or files"><Plus aria-hidden="true" /></summary>
              <div className="composer-popover composer-attachment-popover">
                <button type="button" onClick={() => photosInputRef.current?.click()}>
                  <Image aria-hidden="true" /><span>Photos</span>
                </button>
                <button type="button" onClick={() => filesInputRef.current?.click()}>
                  <FileText aria-hidden="true" /><span>Files</span>
                </button>
              </div>
            </details>
            {(session.capabilities.setModel || session.capabilities.reasoning || session.capabilities.fastMode || session.capabilities.usage) ? (
              <details ref={settingsMenuRef} className="composer-menu composer-settings-menu">
                <summary aria-label={`Model and reasoning, current ${[modelLabel, reasoningLabel].filter(Boolean).join(" · ")}`}>
                  {session.modelSwitchPending ? <LoaderCircle className="status-spin" aria-hidden="true" /> : <Bolt aria-hidden="true" />}
                  <strong>{modelLabel}</strong>
                  {reasoningLabel ? <span>{reasoningLabel}</span> : null}
                  {usagePercent !== undefined ? (
                    <span className="composer-usage-compact">{Math.round(usagePercent)}%</span>
                  ) : null}
                </summary>
                <div className="composer-popover composer-settings-popover">
                  {session.capabilities.setModel && session.availableModels.length ? (
                    <fieldset>
                      <legend>Model</legend>
                      {session.availableModels.map((model) => (
                        <button
                          type="button"
                          key={model.id}
                          aria-pressed={model.id === selectedModel}
                          onClick={() => updateModel(model.id, null)}
                        >
                          <span>{compactModelLabel(model.label)}</span>
                          {model.id === selectedModel ? <Check aria-hidden="true" /> : null}
                        </button>
                      ))}
                    </fieldset>
                  ) : null}
                  {session.capabilities.reasoning && session.reasoningEffort?.options.length ? (
                    <fieldset>
                      <legend>Reasoning</legend>
                      {session.reasoningEffort.options.map((effort) => (
                        <button
                          type="button"
                          key={effort.id}
                          aria-pressed={effort.id === selectedEffort}
                          onClick={() => updateModel(selectedModel, effort.id)}
                        >
                          <span>{compactReasoningLabel(effort.label)}</span>
                          {effort.id === selectedEffort ? <Check aria-hidden="true" /> : null}
                        </button>
                      ))}
                    </fieldset>
                  ) : null}
                  {session.capabilities.fastMode && session.fastMode?.supported ? (
                    <button
                      type="button"
                      className="composer-menu-fast-mode"
                      aria-pressed={session.fastMode.enabled}
                      disabled={!connected || session.fastMode.pending || session.modelSwitchPending}
                      onClick={() => commands.setFastMode(!session.fastMode?.enabled)}
                    >
                      <Bolt aria-hidden="true" />
                      <span>Fast mode</span>
                      {session.fastMode.enabled ? <Check aria-hidden="true" /> : null}
                    </button>
                  ) : null}
                  {session.capabilities.usage ? (
                    <button
                      type="button"
                      className="composer-menu-usage"
                      disabled={!connected || session.status === "closed"}
                      onClick={() => {
                        settingsMenuRef.current?.removeAttribute("open");
                        onOpenUsage();
                      }}
                    >
                      {usagePending ? <LoaderCircle className="status-spin" aria-hidden="true" /> : <ChartBar aria-hidden="true" />}
                      <span>Usage</span>
                    </button>
                  ) : null}
                </div>
              </details>
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
                <Square aria-hidden="true" />
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

import Check from "lucide-react/dist/esm/icons/check.js";
import CircleAlert from "lucide-react/dist/esm/icons/circle-alert.js";
import ClipboardCheck from "lucide-react/dist/esm/icons/clipboard-check.js";
import HelpCircle from "lucide-react/dist/esm/icons/circle-help.js";
import Send from "lucide-react/dist/esm/icons/send.js";
import ShieldCheck from "lucide-react/dist/esm/icons/shield-check.js";
import X from "lucide-react/dist/esm/icons/x.js";
import { useState } from "react";
import type {
  InteractionResponse,
  RemoteInteraction,
  RemoteQuestionAnswer,
} from "../protocol";
import type { ForgeRemoteCommands } from "../remoteSocket";
import { Markdown } from "./Markdown";

function InteractionShell({
  interaction,
  icon,
  children,
}: {
  interaction: RemoteInteraction;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  const resolved = interaction.status && interaction.status !== "pending";
  return (
    <section className="interaction-card" data-resolved={resolved || undefined}>
      <header>
        <span className="interaction-icon">{icon}</span>
        <span>
          <strong>{interaction.title || "Forge needs your input"}</strong>
          {interaction.description ? <small>{interaction.description}</small> : null}
        </span>
      </header>
      {resolved ? (
        <div className="interaction-resolved"><Check aria-hidden="true" />{interaction.description || "Answered."}</div>
      ) : children}
    </section>
  );
}

function PermissionCard({
  interaction,
  submit,
  disabled,
}: {
  interaction: Extract<RemoteInteraction, { kind: "permission" }>;
  submit(response: InteractionResponse): void;
  disabled: boolean;
}) {
  const [followup, setFollowup] = useState("");
  return (
    <InteractionShell interaction={interaction} icon={<ShieldCheck aria-hidden="true" />}>
      <div className="interaction-actions permission-actions">
        {interaction.options.map((option) => (
          <button key={option.id} type="button" disabled={disabled} onClick={() => submit({ kind: "permission", optionId: option.id })}>
            <strong>{option.label}</strong>{option.description ? <small>{option.description}</small> : null}
          </button>
        ))}
      </div>
      {interaction.allowFollowup ? (
        <div className="interaction-followup">
          <label><span>Tell Forge what to change</span><textarea rows={2} value={followup} onChange={(event) => setFollowup(event.target.value)} /></label>
          <button type="button" disabled={disabled || !followup.trim()} onClick={() => submit({ kind: "permissionFollowup", text: followup.trim() })}><Send aria-hidden="true" />Send feedback</button>
        </div>
      ) : null}
      <button className="interaction-cancel" type="button" disabled={disabled} onClick={() => submit({ kind: "cancel" })}><X aria-hidden="true" />Cancel request</button>
    </InteractionShell>
  );
}

function QuestionCard({
  interaction,
  submit,
  disabled,
}: {
  interaction: Extract<RemoteInteraction, { kind: "question" }>;
  submit(response: InteractionResponse): void;
  disabled: boolean;
}) {
  const [selected, setSelected] = useState<Record<number, number[]>>({});
  const [freeform, setFreeform] = useState<Record<number, string>>({});
  const answers: RemoteQuestionAnswer[] = interaction.questions.map((question, questionIndex) => ({
    questionIndex,
    optionIndices: selected[questionIndex] || [],
    ...(freeform[questionIndex]?.trim() ? { freeform: freeform[questionIndex].trim() } : {}),
  }));
  const complete = interaction.questions.every((question, index) =>
    (selected[index]?.length || 0) > 0 || (question.allowFreeform && freeform[index]?.trim()),
  );
  return (
    <InteractionShell interaction={interaction} icon={<HelpCircle aria-hidden="true" />}>
      <div className="question-list">
        {interaction.questions.map((question, questionIndex) => (
          <fieldset key={`${interaction.interactionId}:${questionIndex}`}>
            <legend>{question.prompt}</legend>
            {question.options.map((option, optionIndex) => {
              const checked = selected[questionIndex]?.includes(optionIndex) || false;
              return (
                <label className="question-option" key={`${questionIndex}:${optionIndex}`}>
                  <input
                    type={question.multiple ? "checkbox" : "radio"}
                    name={`${interaction.interactionId}:${questionIndex}`}
                    checked={checked}
                    disabled={disabled}
                    onChange={() => {
                      if (!question.multiple) {
                        setFreeform((current) => ({ ...current, [questionIndex]: "" }));
                      }
                      setSelected((current) => ({
                        ...current,
                        [questionIndex]: question.multiple
                          ? checked
                            ? (current[questionIndex] || []).filter((index) => index !== optionIndex)
                            : [...(current[questionIndex] || []), optionIndex]
                          : [optionIndex],
                      }));
                    }}
                  />
                  <span><strong>{option.label}</strong>{option.description ? <small>{option.description}</small> : null}</span>
                </label>
              );
            })}
            {question.allowFreeform ? (
              <label className="freeform-answer"><span>Your answer</span><textarea rows={2} value={freeform[questionIndex] || ""} disabled={disabled} onChange={(event) => {
                const value = event.target.value;
                if (!question.multiple && value.trim()) {
                  setSelected((current) => ({ ...current, [questionIndex]: [] }));
                }
                setFreeform((current) => ({ ...current, [questionIndex]: value }));
              }} /></label>
            ) : null}
          </fieldset>
        ))}
      </div>
      <div className="interaction-submit-row">
        <button type="button" className="interaction-cancel" disabled={disabled} onClick={() => submit({ kind: "cancel" })}>Cancel</button>
        <button type="button" className="interaction-submit" disabled={disabled || !complete} onClick={() => submit({ kind: "question", answers })}><Send aria-hidden="true" />Send answer</button>
      </div>
    </InteractionShell>
  );
}

function PlanCard({
  interaction,
  submit,
  disabled,
}: {
  interaction: Extract<RemoteInteraction, { kind: "plan" }>;
  submit(response: InteractionResponse): void;
  disabled: boolean;
}) {
  const [feedback, setFeedback] = useState("");
  return (
    <InteractionShell interaction={interaction} icon={<ClipboardCheck aria-hidden="true" />}>
      <div className="interaction-plan"><Markdown text={interaction.plan} /></div>
      {interaction.allowFeedback ? <label className="plan-feedback"><span>Changes for Forge</span><textarea rows={3} value={feedback} disabled={disabled} onChange={(event) => setFeedback(event.target.value)} /></label> : null}
      <div className="interaction-submit-row">
        <button type="button" className="interaction-cancel" disabled={disabled} onClick={() => submit({ kind: "plan", outcome: "cancelled" })}>Cancel</button>
        {interaction.allowFeedback ? <button type="button" disabled={disabled || !feedback.trim()} onClick={() => submit({ kind: "plan", outcome: "cancelled", feedback: feedback.trim() })}>Request changes</button> : null}
        <button type="button" className="interaction-submit" disabled={disabled} onClick={() => submit({ kind: "plan", outcome: "approved" })}><Check aria-hidden="true" />Approve plan</button>
      </div>
    </InteractionShell>
  );
}

export function InteractionCards({
  interactions,
  commands,
  disabled,
}: {
  interactions: RemoteInteraction[];
  commands: ForgeRemoteCommands;
  disabled: boolean;
}) {
  if (!interactions.length) return null;
  return (
    <div className="interaction-deck" aria-label="Requests from Forge">
      {interactions.map((interaction) => {
        const submit = (response: InteractionResponse) => commands.resolveInteraction(interaction.interactionId, response);
        if (interaction.kind === "permission") return <PermissionCard key={interaction.interactionId} interaction={interaction} submit={submit} disabled={disabled} />;
        if (interaction.kind === "question") return <QuestionCard key={interaction.interactionId} interaction={interaction} submit={submit} disabled={disabled} />;
        if (interaction.kind === "plan") return <PlanCard key={interaction.interactionId} interaction={interaction} submit={submit} disabled={disabled} />;
        return (
          <InteractionShell key={interaction.interactionId} interaction={interaction} icon={<CircleAlert aria-hidden="true" />}>
            <p className="unsupported-copy">This request cannot be answered on the phone. Use the Forge terminal to continue.</p>
          </InteractionShell>
        );
      })}
    </div>
  );
}

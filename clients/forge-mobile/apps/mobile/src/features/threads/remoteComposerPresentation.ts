import type { ProviderOptionDescriptor } from "@t3tools/contracts";

import { providerOptionValueLabels } from "../../lib/providerOptions";

const REMOTE_PROVIDER_SEPARATOR = " · ";

function titleCaseSlugToken(token: string): string {
  return /^[a-z]/.test(token) ? `${token[0]?.toUpperCase() ?? ""}${token.slice(1)}` : token;
}

function compactRemoteReasoningLabel(label: string): string {
  return /^(extra[ -]?high|xhigh)$/i.test(label.trim()) ? "Ultra" : label.trim();
}

/**
 * Forge Remote's composer identifies the model, not the transport harness.
 * Server labels are intentionally accepted as-is apart from provider prefixes
 * and the GPT product prefix so custom model names remain recognizable.
 */
export function compactRemoteModelLabel(input: string): string {
  const normalized = input.trim().replace(/\s+/g, " ");
  const separatorIndex = normalized.indexOf(REMOTE_PROVIDER_SEPARATOR);
  let label =
    separatorIndex >= 0
      ? normalized.slice(separatorIndex + REMOTE_PROVIDER_SEPARATOR.length).trim()
      : normalized;

  label = label.replace(/^gpt[-\s:]*(?=\d)/i, "");

  if (/^[a-z0-9._-]+$/.test(label) && /[-_]/.test(label)) {
    label = label.split(/[-_]+/).filter(Boolean).map(titleCaseSlugToken).join(" ");
  }

  return label || input.trim();
}

export function remoteComposerPresentation(
  modelLabel: string,
  optionDescriptors: ReadonlyArray<ProviderOptionDescriptor>,
): { readonly accessibilityLabel: string; readonly model: string; readonly reasoning?: string } {
  const model = compactRemoteModelLabel(modelLabel);
  const reasoningDescriptor = optionDescriptors.find(
    (descriptor) => descriptor.id === "reasoningEffort",
  );
  const verboseReasoning = reasoningDescriptor
    ? providerOptionValueLabels([reasoningDescriptor])[0]
    : undefined;
  const reasoning = verboseReasoning ? compactRemoteReasoningLabel(verboseReasoning) : undefined;
  return {
    model,
    accessibilityLabel: [model, reasoning].filter(Boolean).join(" · "),
    ...(reasoning ? { reasoning } : {}),
  };
}

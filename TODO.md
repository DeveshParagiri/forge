# Forge Feature Requirements

This document tracks fork-specific product requirements for Forge, the upstream-friendly extension of Grok Build.

## Engineering constraints

All work below must follow these constraints:

- Minimize changes to upstream-owned source files to reduce rebase and merge conflicts.
- Put substantial fork-specific behavior behind additive, crate-local Forge modules and narrow integration hooks.
- Prefer capability-driven interfaces over provider- or harness-specific conditionals.
- Reuse existing abstractions before introducing new ones.
- Keep behavior changes focused, tested, and independently reviewable; do not mix them with unrelated refactors.
- Preserve existing SpaceXAI behavior and compatibility unless a requirement explicitly changes it.
- Add focused unit or integration coverage for each feature and regression.
- Treat credentials, imported sessions, prompts, and learned preferences as private local data.

## Priority summary

| ID | Requirement | Priority |
|---|---|---|
| FR-2 | Product README and extensibility documentation | P1 |
| FR-5 | Reliable, extensible external subagent harnesses | P0 |
| FR-6 | Prompt and navigate subagents | P1 |
| FR-7 | Complete extension context in harness prompts | P0 |
| FR-8 | Personalized, self-evolving harness/model learning | P2 |
| FR-9 | Keyboard shortcut consistency and regressions | P0 |

## FR-2: README and extensibility architecture

### Goal

Provide a proper user-facing README that explains Forge features and how extensions are structured.

### Requirements

The main README must cover:

- What Forge is and its relationship to upstream Grok Build.
- Installation, update, configuration, and authentication flows.
- Supported providers and external harnesses.
- `/fast`, `/sessions`, subagent interaction, session import/resume, and personalization features.
- Privacy and local-data behavior.
- Extension architecture, including stable capabilities, adapters, registries, narrow upstream hooks, and test expectations.
- How to add a provider, model capability, session importer, or harness without modifying unrelated core logic.
- The Forge naming and compatibility policy.

### Acceptance criteria

- A new user can install Forge and understand its major features from the README alone.
- A contributor can identify the intended extension points without reverse-engineering provider-specific branches.
- Examples and paths match current behavior.

## FR-3: Opt-in external sessions in `/sessions` — completed

### Behavior

- `/sessions` shows native Forge sessions by default.
- Setting `[sessions].show_external = true` adds locally available Claude Code and Codex sessions.
- External rows display their harness name (`Claude Code` or `Codex`).
- Selecting an external row starts a fresh Forge session and invokes the matching `/resume-claude` or `/resume-codex` skill with its native session ID.
- Forge owns only the opt-in and labeling policy under the pager crate's `forge/` module; existing upstream discovery, normalization, picker, and resume dispatch remain reused through narrow hooks.
- Missing skills or inaccessible session stores fail soft through the existing foreign-session gates.

### Acceptance criteria

- External sessions are absent unless the Forge flag is enabled.
- Enabling the flag preserves the existing per-harness compatibility settings and only scans locally supported sources.
- Claude Code and Codex entries have unambiguous harness labels.
- Selecting either source follows the existing `/resume-*` import flow.

## FR-5: Reliable and extensible external subagent harnesses

### Goal

Make external subagent harnesses reliable and straightforward to extend beyond Claude Code and Codex.

### Requirements

- Define a harness adapter contract covering identity, availability detection, launch, resume, prompt delivery, event streaming, cancellation, shutdown, capabilities, and error normalization.
- Keep process supervision, lifecycle state, output/event normalization, and UI presentation harness-neutral.
- Harness-specific CLI flags, session identifiers, output parsing, and authentication checks live only in their adapter.
- Detect missing or unauthenticated CLIs before launch and report actionable errors; never auto-install tools.
- Support clean cancellation, process cleanup, timeouts, abnormal exits, and partial output.
- Avoid shell-string construction where structured process arguments can be used.
- Expose capability metadata for resume, interactive prompting, streaming, tool events, working-directory support, and isolation support.
- Provide a small registration mechanism and contributor documentation for adding another harness.

### Acceptance criteria

- Claude Code and Codex adapters pass the same contract test suite.
- Launch, event delivery, cancellation, resume, failure, and cleanup behavior are covered.
- Adding a fixture harness requires no changes to core orchestration beyond registration.
- Harness failures cannot crash or wedge the parent Forge session.

## FR-6: Prompt and navigate subagents

### Goal

Allow users to inspect, navigate, and send follow-up prompts to active or resumable subagents.

### Requirements

- Provide a discoverable view or command showing subagent identity, harness/model, status, task, start time, and parent relationship.
- Users can select a subagent and send it a follow-up prompt when its harness supports interactive continuation.
- Users can move between the parent and child views without losing draft input or context.
- Output clearly identifies which agent produced each event.
- Finished agents can be resumed when supported; unsupported actions are visibly disabled or explained.
- Cancellation targets the selected subagent and requires no ambiguous global process kill.
- Keyboard and command navigation must be documented and represented in shortcut help.

### Acceptance criteria

- A user can find an active subagent, inspect its state, prompt it, return to the parent, and later revisit its output.
- Multiple simultaneous subagents remain distinguishable.
- Interaction behavior degrades clearly for non-interactive harnesses.
- Tests cover selection, routing, parent/child navigation, follow-up delivery, and cancellation.

## FR-7: Complete extension context in harness prompts

### Goal

Ensure external harnesses receive the Forge extension context required to behave consistently and safely.

### Requirements

- Build harness prompts from composable, typed sections rather than one duplicated monolithic string.
- Include the task, working directory, applicable repository instructions, Forge extension conventions, tool/capability constraints, parent-agent context, expected result format, and relevant safety boundaries.
- Include only context relevant to the selected harness and task; avoid token-heavy duplication.
- Preserve instruction precedence and clearly separate trusted system-generated context from user content and imported session text.
- Version the prompt contract so adapters can evolve without silently changing assumptions.
- Redact credentials and sensitive local values from generated prompts and debug logs.
- Provide prompt snapshots or contract tests for every built-in harness.

### Acceptance criteria

- Claude Code and Codex receive equivalent semantic context expressed in their supported format.
- Repository-specific instructions are present exactly once and follow precedence rules.
- Harness prompts include Forge’s additive-extension and minimal-upstream-intervention requirements when relevant.
- Snapshot tests detect accidental prompt omissions or duplication.

## FR-8: Personalized, self-evolving model and harness learning

### Goal

Remember which models and harnesses work well for a user and use those learnings to improve future recommendations without creating opaque or unsafe autonomous behavior.

### Requirements

- Store local, structured observations about task type, selected model/harness, outcome signals, latency, failures, explicit user feedback, and user preferences.
- Separate explicit preferences from inferred observations; explicit choices always take precedence.
- Recommendations must be explainable, for example: “Codex is preferred for Rust fixes because it succeeded in 8 of 9 recent tasks.”
- Learning affects suggestions or defaults, not irreversible actions or silent provider changes.
- Users can inspect, edit, disable, export, and clear learned data.
- Support project-local and global scopes with clear precedence.
- Use a versioned storage schema and pluggable scorer/policy interface so future learning strategies do not require UI or orchestration rewrites.
- Apply bounded retention, avoid storing raw prompts by default, and never store credentials.
- Cold-start behavior remains deterministic and sensible with no history.

### Acceptance criteria

- Explicit harness/model preferences persist and are honored.
- Repeated success/failure signals can change a recommendation in a deterministic test.
- The UI explains why a recommendation was made and allows one-time override.
- Disabling or clearing learning restores baseline behavior.
- Corrupt or old learning data fails safely and can be migrated or ignored.

## FR-9: Keyboard shortcut consistency and regressions

### Goal

Fix broken or theme-dependent shortcuts and make key behavior consistent with current product semantics.

### Requirements

- `Shift+Tab` must perform the documented action consistently across themes; reasoning-effort cycling must not depend on selecting one theme.
- Permission/mode switching must have its own explicit binding if still supported.
- Audit custom behavior added by Forge, including `Ctrl+C`, `Esc`, overlays, selection states, active generation, subagent views, and shortcut help/footer rendering.
- Centralize shortcut definitions and action resolution so UI labels cannot drift from runtime behavior.
- Define precedence for overlays, focused inputs, selections, running turns, and global actions.
- Preserve terminal-standard `Ctrl+C` expectations: cancel the current operation when possible, and use safe repeated/idle behavior for application exit rather than accidental termination or ignored input.
- Update help text, footer hints, tests, and documentation together with behavior.
- Shortcut behavior must not vary by color theme unless a shortcut explicitly controls theme behavior.

### Acceptance criteria

- `Shift+Tab` cycles reasoning effort in every supported theme.
- Mode switching uses the same documented binding in every theme.
- `Ctrl+C` and `Esc` behavior is deterministic in idle, editing, overlay, selection, running-turn, and subagent states.
- A table-driven shortcut test suite verifies action resolution by state and theme.
- Displayed shortcut hints are generated from or validated against the active keymap.

## Extensibility architecture

Implement these features around a small set of stable contracts:

- `ProviderCapabilities`: fast mode, reasoning effort, model switching, request options, and authentication traits.
- `SessionSource`: discovery, normalized metadata, context extraction, filtering, and resume handoff.
- `HarnessAdapter`: availability, launch, resume, prompt, events, cancellation, lifecycle, and capability reporting.
- `PromptContextBuilder`: ordered, versioned prompt sections shared across harnesses.
- `PreferenceStore` and `RecommendationPolicy`: versioned observations, explicit preferences, scoring, and explanations.
- `Action`/keymap registry: canonical shortcut actions separated from themes and rendering.

Core orchestration and UI should consume these contracts. Provider-, source-, and harness-specific code should be registered through adapters rather than spread across dispatch, rendering, and session code.

## Delivery order

1. **Foundation:** capability and adapter contracts.
2. **Correctness:** FR-9 keyboard regressions; FR-7 prompt-context contract; FR-5 harness lifecycle reliability.
3. **User workflows:** FR-3 unified sessions; FR-6 subagent navigation and prompting.
4. **Documentation:** FR-2 README updated alongside each shipped feature.
5. **Personalization:** FR-8 local learning after stable model, harness, and outcome telemetry exists.

Each item should land as a focused change with an explicit list of upstream-owned files touched, focused tests, and migration notes where applicable.

## Definition of done

A feature is complete only when:

- Its acceptance criteria pass.
- User-facing help and README content are current.
- Focused unit/integration tests cover normal, unsupported, and failure paths.
- Existing SpaceXAI and native-session behavior remains green.
- Compatibility and privacy implications are documented.
- The implementation keeps substantial fork logic additive and minimizes edits to upstream-owned files.

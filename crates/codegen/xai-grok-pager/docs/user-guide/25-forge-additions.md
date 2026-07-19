# Forge-Specific Additions

Forge is an independent, upstream-friendly extension of Grok Build. It preserves the native terminal workflow while adding model/provider choice, external coding harnesses, capability-aware subagent orchestration, and a simpler Forge presentation.

This guide documents behavior added by Forge. The preceding user-guide chapters continue to describe the shared Grok Build foundation.

---

## Models and providers

Forge exposes supported SpaceXAI, ChatGPT Codex, and OpenRouter models in the same model picker. The exact roster depends on the providers configured and authenticated for the current installation.

Provider-specific features are capability-driven:

- `/fast` is available when the active provider and model support fast mode.
- `/usage` reports usage through the active provider's supported accounting path.
- Explicit subagent model overrides are limited to the model slugs advertised by the `spawn_subagent` tool for the current session.

Forge does not infer capabilities merely from model-name substrings. Provider and model integrations declare the behavior they support.

---

## Multi-model and multi-harness subagents

In addition to native `general-purpose`, `explore`, and `plan` agents, Forge can delegate work to these external harnesses:

| Subagent type | Harness | Typical use |
|---|---|---|
| `claude-code` | Claude Code CLI | Implementation, architecture, or an independent review perspective |
| `codex-cli` | Codex CLI | Implementation, debugging, tests, or an independent review perspective |

External harnesses use their official CLI, native authentication, model defaults, and tools. They do not inherit Forge-only hosted tools. Forge normalizes their streaming output, lifecycle state, cancellation, and result delivery into the native Subagents UI.

### Prerequisites

Install and authenticate an external CLI separately before using its adapter:

```sh
claude --version
codex --version
```

Forge never installs these tools automatically. Native Forge agents remain available when neither external CLI is installed.

### Capability-aware orchestration

Forge gives the primary agent a compact routing policy rather than a fixed model-to-task table. The agent is instructed to:

- Delegate only when parallelism, specialization, isolation, or independent verification justifies the coordination cost.
- Choose from the targets and model slugs advertised by the task tool according to task fit and required capabilities.
- Prefer a suitable subscription-backed or included harness over a separately metered API model unless the user specifies otherwise.
- Treat that preference as cost routing, not a quality ranking: explicit user choices and required capabilities always win.
- Use stronger reasoning or implementation options for complex or high-risk work and faster options for bounded lookup or mechanical work.
- Blend roles, model families, or harnesses when complementary perspectives are useful, especially for independent review.
- Give workers non-overlapping scopes, avoid redundant fan-out, and retain one owner for final synthesis.

The task tool remains the runtime source of truth for available agents, harnesses, model slugs, and parameters. The system prompt deliberately does not duplicate that roster, keeping the Forge addition small and resilient as integrations change.

### Evolving preferences through memory

When memory is enabled, Forge gives the existing LLM-based memory pipeline one additional instruction: retain durable evidence about which models, harnesses, and subagent setups work best for this user for different task types. It considers explicit direction and corrections, repeated choices, the target used, outcome quality, and the user's reactions or feedback. This is semantic interpretation, not keyword matching.

Explicit user direction outweighs inference. Implicit conclusions require clear or repeated evidence, while ambiguous one-off results are not treated as firm preferences. Useful user wording may be preserved as a short attributed quotation. The result remains concise ordinary Markdown; Forge adds no separate calls, timers, database, scores, or schema, and does not change when the normal memory lifecycle runs.

Current user instructions always take precedence over recalled preferences.

### External harness limitations

External adapters currently have different capabilities from native agents:

- Their CLI must already be installed and authenticated.
- They use provider-native tools rather than Forge-hosted tools.
- Explicit model omission uses the adapter's provider-native default.
- External worktree isolation is not currently supported; use an explicit working directory or a native worktree-capable agent.
- Resume depends on provider session support. Native provider session identifiers are not yet durably mapped across a Forge process restart.

---

## External sessions in `/sessions`

Forge sessions are shown by default. To also discover locally available Claude Code and Codex sessions, add this to `~/.grok/config.toml`:

```toml
[sessions]
show_external = true
```

External rows are labeled `Claude Code` or `Codex`. Selecting one creates a fresh Forge session and invokes the corresponding `/resume-claude` or `/resume-codex` context-import flow. It does not reopen the external harness UI directly.

---

## Forge interface

Forge uses its own simplified theme and product name. Fast mode is displayed with the text-presentation lightning symbol `⚡︎`, allowing the terminal theme to control its foreground color instead of forcing a colored emoji glyph.

---

## Installation and updates

The standard Forge installer downloads a prebuilt release archive, verifies its SHA-256 checksum, and atomically installs `grok` under `~/.grok/bin/`. It does not need a source checkout or Rust toolchain.

Update an installed copy with:

```sh
grok update
```

Updates preserve configuration, authentication, and sessions under `~/.grok/`. They download packaged Forge releases and do not compile or synchronize an upstream Git checkout.

Forge release tags use `forge-vMAJOR.MINOR.PATCH`. The synchronized upstream Grok Build version is recorded separately in release notes and build metadata.

---

## Extension architecture

Substantive fork behavior is kept in additive, crate-local `forge/` modules. Upstream-owned source files contain only narrow hooks where Forge must enter an existing prompt, lifecycle, provider, or rendering path.

The main extension areas are:

- Provider and model capability policy.
- External harness adapters and provider-neutral lifecycle handling.
- Primary-agent and subagent prompt extensions.
- Forge-specific session policy and labels.
- Theme, model label, and fast-mode presentation.
- Packaged update and release behavior.

This separation is intended to make future providers and harnesses additive while keeping upstream synchronization reviewable.

For source-level ownership and synchronization details, see the repository's `FORK-MAINTENANCE.md`.
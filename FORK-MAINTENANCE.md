# Forge fork maintenance notes

This repository tracks [xai-org/grok-build](https://github.com/xai-org/grok-build).
The published, installable Forge branch is `main`; integration happens on `dev`;
upstream is tracked through the `upstream/main` remote-tracking branch.

## Installed layout

| Role | Path |
|------|------|
| Canonical executable | `~/.grok/bin/grok` |
| Source checkout | `~/.local/share/grok/source` |
| Update script | `~/bin/grok-update-from-source` |
| Config, auth, and sessions | `~/.grok/` |
| ChatGPT Codex auth | `~/.codex/auth.json` |
| Third-party provider config | `[model_providers.*]` in `~/.grok/config.toml` |

`~/.local/bin/grok`, `~/.local/share/grok/versions/current`,
`~/.grok/local/grok`, and `~/bin/grok` are compatibility symlinks to the
canonical executable.

## Branches and remotes

- `main`: published Forge source and the repository default branch.
- `dev`: integration branch; publish it to `main` only after validation.
- `forge-vMAJOR.MINOR.PATCH`: immutable Forge SemVer release tags. Record the
  synchronized upstream Grok version separately in release notes/build metadata.
- `refactor/*`: temporary local worktrees only.
- `upstream/main`: upstream source from `https://github.com/xai-org/grok-build.git`.
- `origin`: Forge fork.

Do not force-push published Forge history. Rebase `dev` onto `upstream/main`
only when intentionally updating the fork; the end-user updater never rebases.
Publish validated commits with `scripts/forge-publish main`, which refuses
non-fast-forward updates.

## Extension architecture

Bulk fork logic belongs in additive, crate-local modules:

| Crate | Forge modules | Responsibility |
|------|-------------------|----------------|
| `xai-grok-sampler` | `src/forge/` | Codex Responses request policy, endpoint trust, unknown-event compatibility, and streamed terminal recovery |
| `xai-grok-shell` | `src/agent/forge/` | Provider identity, narrow Codex credentials, catalog policy, request profiles, usage, Fast Mode, and portable history |
| `xai-grok-pager` | `src/forge/` | Provider usage, Fast Mode and effort controls, layout, welcome branding, and focused UI tests |
| `xai-grok-pager-render` | `src/forge/` | Claude palette/package policy and shortcut-footer state |

Use `// Forge:` or `/// Forge:` for residual hooks in upstream-owned
files. Prefer a small call into an Forge module over inline fork logic.

Some coupling should remain inline because ordering or exhaustive matching is
part of the behavior:

- Running-turn `Esc` cancellation in `agent_view/prompt.rs`.
- Theme enum registration/cache decoding and syntax selection.
- Provider model-switch and sampler request-boundary hooks.
- Welcome/dashboard geometry where it participates in upstream layout flow.

## Main integration points

### Sampler

- `xai-grok-sampler/src/client.rs`: enforce the final credential/header boundary for every request shape.
- `xai-grok-sampler/src/forge/endpoint_policy.rs`: positively identify official Codex and xAI HTTPS endpoints.
- `xai-grok-sampler/src/stream/responses.rs`: observe and apply terminal recovery.
- `xai-grok-sampler/tests/forge_codex_responses.rs`: cross-module Codex integration coverage.

### Shell

- `agent/model_providers.rs`: upstream shared provider endpoint/credential/header configuration.
- `agent/config.rs`: apply model-provider defaults and expose provider-aware model metadata.
- `agent/models.rs`: refine the stock model list with provider catalog policy.
- `session/acp_session_impl/model_switch.rs`: remove or flatten provider-bound history when required.
- `session/acp_session_impl/sampler_turn.rs`: apply the narrow Codex request profile and dynamic account header.

Legacy `agent/provider_auth.rs` and `agent/provider_history.rs` remain thin
compatibility facades; implementation belongs in `agent/forge/`.

### Pager

- `app/dispatch/status.rs` and `app/dispatch/task_result.rs`: provider-aware `/usage` routing and output.
- `app/agent_view/{interactions,render}.rs`: Fast Mode and reasoning-effort controls.
- `app/agent_view/prompt.rs`: ordering-sensitive `Esc` cancellation.
- `views/welcome/` and dashboard files: small branding/layout hooks.

`/login` remains the stock SpaceXAI login path. Codex and OpenRouter credentials
are configured out of band; there is no fork-owned provider-login UI.

### Pager render

- `appearance/cache.rs`: public shortcut-state facade and priming hook.
- `theme/mod.rs`: stable Forge registration and policy delegation.
- `theme/cache.rs` and `syntax.rs`: cache/syntax exhaustive-match hooks.

Forge palette implementation: `src/forge/forge_theme.rs`.

## Preserved behavior

- Stock SpaceXAI welcome login remains on the upstream `/login` path.
- Generic third-party providers use upstream `[model_providers.*]` plus each
  model's `model_provider`; the removed `[provider.*]` syntax is inert and warns
  as unrecognized configuration.
- ChatGPT Codex reads OAuth access/account data from `~/.codex/auth.json`; run
  `codex login` to populate or refresh that file.
- OpenRouter uses upstream `api_key`/`env_key` configuration, normally
  `env_key = "OPENROUTER_API_KEY"`; Forge does not own a provider key store.
- Provider catalogs support include/exclude filtering.
- Provider-family switches remove nonportable Responses reasoning and flatten
  backend-only tool calls while retaining portable user/assistant/tool context.
- Fast Mode is session-scoped and sends `service_tier = "priority"` only for
  models that explicitly set `supports_fast_mode = true`.
- Codex requests suppress endpoint-rejected body fields, and all non-xAI
  endpoints reject xAI bearer/private metadata at the final request boundary.
- Unknown additive Responses events and liveness events are tolerated.
- Streamed text/function calls are recovered when terminal Responses output is
  incomplete.
- Forge theme Shift+Tab cycles reasoning effort; other themes retain stock
  permission-mode cycling.
- `/usage` remains provider-aware: stock SpaceXAI billing stays upstream while
  ChatGPT Codex uses its account usage endpoint.
- Running-turn `Esc` cancels generation like `Ctrl+C` after overlays and
  selections receive their normal priority.

## Focused verification

Use crate-specific checks instead of full-workspace test runs:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p xai-grok-sampler --lib forge::
cargo test -p xai-grok-sampler --test forge_codex_responses
cargo test -p xai-grok-shell --lib agent::forge
cargo test -p xai-grok-pager --lib forge
cargo test -p xai-grok-pager-render
```

The pager test filter still compiles its complete library test binary before
running the focused tests. Avoid repeatedly restarting it during compilation.

## Updating from upstream

```bash
git status --short             # must be clean
git fetch upstream --tags
scripts/forge-sync-upstream
```

When resolving conflicts:

1. Preserve additive `forge/` modules.
2. Reapply or adapt the small `// Forge:` hooks to the new upstream flow.
3. Re-check ordering-sensitive behavior rather than moving it mechanically.
4. Run the focused checks above.
5. Publish the validated integration commit:

```bash
scripts/forge-publish main
```

6. Build and install:

```bash
cargo build -p xai-grok-pager-bin --release
install -m 755 target/release/xai-grok-pager ~/.grok/bin/grok
```

The updater automates fetch, rebase, release build, atomic installation, and
compatibility symlink refresh:

```bash
~/bin/grok-update-from-source
```

## Deferred hardening

Keep these as separate behavior-changing commits rather than folding them into
provider cleanup:

- Extend provider-aware `/usage` only when a provider exposes a stable account
  usage API.
- Consider finer per-provider capability objects if additional endpoint-specific
  behavior appears; unknown endpoints must continue to fail closed.
- Consider a transactional history rewrite only if the current in-memory model
  switch flow gains fallible mutation after normalization.

The detailed provider parity background remains in
[`docs/CHATGPT-HANDOFF-PI-PARITY.md`](docs/CHATGPT-HANDOFF-PI-PARITY.md), but
this file is the current source of truth for paths and rebase procedure.

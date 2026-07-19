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
| OpenRouter key store | `~/.grok/provider_keys.json` |

`~/.local/bin/grok`, `~/.local/share/grok/versions/current`,
`~/.grok/local/grok`, and `~/bin/grok` are compatibility symlinks to the
canonical executable.

## Branches and remotes

- `main`: published Forge source and the repository default branch.
- `dev`: integration branch; publish it to `main` only after validation.
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
| `xai-grok-sampler` | `src/forge/` | Codex Responses request policy, unknown-event compatibility, streamed terminal recovery |
| `xai-grok-shell` | `src/agent/forge/` | Provider identity, configuration, credentials, status, catalog policy, profiles, and portable history |
| `xai-grok-pager` | `src/forge/` | Provider login, effort controls, layout, welcome branding, and focused UI tests |
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

- `xai-grok-sampler/src/client.rs`: select and apply Responses backend policy.
- `xai-grok-sampler/src/stream/responses.rs`: observe and apply terminal recovery.
- `xai-grok-sampler/tests/forge_codex_responses.rs`: cross-module Codex integration coverage.

### Shell

- `agent/config.rs`: parse provider packs and apply credentials/request metadata.
- `agent/models.rs`: refine the stock model list with provider catalog policy.
- `session/acp_session_impl/model_switch.rs`: remove provider-bound reasoning when required.
- `session/acp_session_impl/sampler_turn.rs`: apply the provider request profile.

Legacy `agent/provider_auth.rs` and `agent/provider_history.rs` remain thin
compatibility facades; implementation belongs in `agent/forge/`.

### Pager

- `app/dispatch/router.rs`: route provider login and Claude effort cycling.
- `app/agent_view/{interactions,render}.rs`: provider input hooks.
- `app/agent_view/prompt.rs`: ordering-sensitive `Esc` cancellation.
- `views/welcome/` and dashboard files: small branding/layout hooks.

Provider login implementation: `src/forge/provider_login/`.

### Pager render

- `appearance/cache.rs`: public shortcut-state facade and priming hook.
- `theme/mod.rs`: stable Forge registration and policy delegation.
- `theme/cache.rs` and `syntax.rs`: cache/syntax exhaustive-match hooks.

Forge palette implementation: `src/forge/forge_theme.rs`.

## Preserved behavior

- Stock SpaceXAI welcome login remains on the upstream path.
- `/login` supports SpaceXAI, ChatGPT Codex, and OpenRouter.
- ChatGPT Codex reads `~/.codex/auth.json`.
- OpenRouter reads the environment or `~/.grok/provider_keys.json`.
- Provider catalogs support include/exclude filtering.
- Provider-family switches remove nonportable Responses reasoning while
  retaining user, assistant, and tool context.
- Codex requests suppress xAI-only body fields and headers.
- Unknown additive Responses events and liveness events are tolerated.
- Streamed text/function calls are recovered when terminal Responses output is
  incomplete.
- Forge theme Shift+Tab cycles reasoning effort; other themes retain stock
  permission-mode cycling.
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
module extraction:

- Replace substring backend detection with parsed URL/host matching.
- Make provider-key writes atomic.
- Refresh OpenRouter catalog state immediately after saving a key.
- Make provider-switch history conversion explicitly transactional.
- Narrow broad third-party capability checks into provider capabilities.

The detailed provider parity background remains in
[`docs/CHATGPT-HANDOFF-PI-PARITY.md`](docs/CHATGPT-HANDOFF-PI-PARITY.md), but
this file is the current source of truth for paths and rebase procedure.

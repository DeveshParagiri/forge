# Personal grok-build fork notes

This tree tracks [xai-org/grok-build](https://github.com/xai-org/grok-build) with local patches on branch **`dev`**.

## ChatGPT / implementer handoff

**Full Pi-parity + multi-provider + include/exclude catalog spec:**

→ [`docs/CHATGPT-HANDOFF-PI-PARITY.md`](docs/CHATGPT-HANDOFF-PI-PARITY.md)

Paste that file into ChatGPT/Codex for remaining work. Includes upstream rebase rules, stock docs paths, Pi feature depth, and acceptance criteria.

## Install layout

| Role | Path |
|------|------|
| Canonical command and binary | `~/.grok/bin/grok` (the personal extended build) |
| Compatibility launchers | `~/.local/bin/grok`, `~/.local/share/grok/versions/current`, `~/.grok/local/grok`, and `~/bin/grok` → canonical binary |
| Config / state | `~/.grok/` |
| Stock documentation | **`~/.grok/docs/user-guide/`** (auth, config, **custom models**) |
| Source | `~/.local/share/grok/source` |

There is exactly one installed Grok executable. Every alternate launcher is a direct symlink to it.

## Remotes

| Remote | URL |
|--------|-----|
| `upstream` | `https://github.com/xai-org/grok-build.git` |

## Upstream-safe personalization

Keep patches **small, additive, and marked** so `git rebase upstream/main` stays easy.

| Rule | Why |
|------|-----|
| Prefer **new files** for bulk logic | Drop/re-apply without fighting upstream diffs |
| Mark hooks with `// Personal:` or `/// Personal:` | Grep after rebase |
| **Do not** rewrite SpaceXAI `Action::Login` welcome flow | Upstream cold-start auth stays stock |
| Config-only model packs in `~/.grok/config.toml` | No binary required for catalog tweaks |

### Personal touch points (grep `Personal:`)

| Area | Path |
|------|------|
| Provider auth store | `crates/.../xai-grok-shell/src/agent/provider_auth.rs` **(new)** |
| Provider-switch history transform | `crates/.../xai-grok-shell/src/agent/provider_history.rs` **(new)** |
| Provider packs + no OIDC on 3p bases | `agent/config.rs` (`ProviderConfig`, `first_own_credential`, `resolve_credentials`, headers) |
| Pi-style `/login` picker | `pager/.../dispatch/provider_login.rs` **(new)** |
| Direct key input + one-press cancel | `agent_view/interactions.rs`, `agent_view/render.rs` (thin `is_direct_input` branches) |
| Slash `/login` | `pager/.../slash/commands/login.rs` (thin) |
| Actions / LocalQuestionKind | `actions.rs`, `question_view.rs` (enum variants only) |
| Codex body + streamed-text fallback | `xai-grok-sampler/src/client.rs`, `stream/responses.rs` |
| Provider catalog filters | `agent/provider_auth.rs`, `agent/models.rs`, `slash/commands/model.rs` |
| Claude theme / Shift+Tab effort | existing personal commits |

### After upstream rebase

```bash
# 1. Rebase
git fetch upstream
git rebase upstream/main   # or: ~/bin/grok-update-from-source

# 2. If conflicts, restore personal modules first (usually conflict-free):
#    - agent/provider_auth.rs
#    - app/dispatch/provider_login.rs

# 3. Re-apply thin hooks if lost (search PERSONAL.md tables)

# 4. Smoke Codex token path (no secrets printed)
python3 scripts/codex-smoke.py

# 5. Rebuild
~/bin/grok-update-from-source   # or cargo build --release -p xai-grok-pager ...
```

---

## Architecture: themes + providers

### UI packages (themes)

| Theme | Colors | Footer shortcuts bar | Shift+Tab |
|-------|--------|----------------------|-----------|
| `claude` | Claude Code dark chatbox | Hidden (unless `show_shortcuts_bar` set) | Cycle **reasoning effort** |
| `groknight` / others | Stock | Shown by default | Cycle **permission mode** |

```toml
[ui]
theme = "claude"
```

### Providers (model auth packs)

```toml
[provider.codex]
base_url = "https://chatgpt.com/backend-api/codex"
api_backend = "responses"
auth = "codex"   # ~/.codex/auth.json

[provider.openrouter]
base_url = "https://openrouter.ai/api/v1"
api_backend = "chat_completions"
auth = "openrouter"  # OPENROUTER_API_KEY or ~/.grok/provider_keys.json

[model."gpt-5.6-sol"]
provider = "codex"
model = "gpt-5.6-sol"
```

### Provider catalogs

`[catalog.*]` filters the existing model catalog without creating a second
picker. Empty `include` means all configured models for that provider;
`exclude` wins when both match. Values accept the same glob syntax as stock
`[models].allowed_models` and match either the local catalog key or provider
model id.

```toml
[catalog.spacexai]
include = []
exclude = []

[catalog.openai_codex]
include = ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]
exclude = []

[catalog.openrouter]
include = ["google/gemini-3.5-flash", "moonshotai/kimi-k3", "anthropic/claude-opus-4.8"]
exclude = []
```

The existing `/model` picker labels each entry as `SpaceX · model`,
`OpenAI · model`, or `OpenRouter · model`. OpenRouter entries stay hidden until
its API key is configured. On a provider-family switch, the shell keeps user,
assistant, and tool context but removes provider-bound Responses reasoning
items before the next request. This prevents foreign `encrypted_content` from
being replayed to Codex. Codex text deltas are also retained in the terminal
response so already-visible text cannot trigger a false empty-response retry.

Stock custom model behavior remains documented in
`crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md`.

### `/login` (Pi-style)

| Command | Behavior |
|---------|----------|
| `/login` | Interactive picker with only `configured` / `not configured` status |
| `/login spacexai` | Upstream SpaceXAI OAuth (same as welcome login) |
| `/login codex` | Reuse `~/.codex/auth.json`; if missing, spawn `codex login` |
| `/login openrouter` | Direct API-key input → `~/.grok/provider_keys.json` |

The provider picker and OpenRouter input both cancel with one press of `Esc`.

Welcome-screen **Log in** still only does SpaceXAI (upstream path).

Verified Codex wire format (before harness): `stream: true`, `input` as list, Bearer from `~/.codex/auth.json`, header `ChatGPT-Account-Id`. See `scripts/codex-smoke.py`.

---

## Update + rebuild

```bash
~/bin/grok-update-from-source
```

## Install layout

| Path | Role |
|------|------|
| `~/.grok/bin/grok` | Canonical personal extended binary |
| `~/.local/bin/grok`, `~/.local/share/grok/versions/current`, `~/.grok/local/grok`, `~/bin/grok` | Compatibility symlinks → canonical binary |
| `~/bin/grok-update-from-source` | Fetch + rebase + rebuild |
| `~/.grok/auth.json` | Grok / SpaceXAI subscription |
| `~/.codex/auth.json` | Codex ChatGPT OAuth |
| `~/.grok/provider_keys.json` | OpenRouter (and future) API keys |

## Auth summary

| Provider | How to configure | Models |
|----------|------------------|--------|
| SpaceXAI | `/login spacexai` or welcome login | `grok-4.5`, etc. |
| OpenAI Codex | `codex login` then `/login codex` | Sol / Terra / Luna |
| OpenRouter | `/login openrouter` | latest Gemini Flash, Kimi, Claude Opus |

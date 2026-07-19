# Personal grok-build fork notes

This tree tracks [xai-org/grok-build](https://github.com/xai-org/grok-build) with local patches on branch **`personal`**.

## Remotes

| Remote | URL |
|--------|-----|
| `upstream` | `https://github.com/xai-org/grok-build.git` |

## Architecture: themes + providers

Extend the harness with named packages instead of one-off hacks.

### UI packages (themes)

| Theme | Colors | Footer shortcuts bar | Shift+Tab |
|-------|--------|----------------------|-----------|
| `claude` | Claude Code dark chatbox | Hidden (unless `show_shortcuts_bar` set) | Cycle **reasoning effort** |
| `groknight` / others | Stock | Shown by default | Cycle **permission mode** |

```toml
[ui]
theme = "claude"
# show_shortcuts_bar = true   # optional force-on under any theme
```

### Providers (model auth packs)

```toml
[provider.codex]
base_url = "https://chatgpt.com/backend-api/codex"
api_backend = "responses"
auth = "codex"   # ~/.codex/auth.json + ChatGPT-Account-ID

[model."gpt-5.4"]
provider = "codex"
model = "gpt-5.4"
name = "GPT-5.4 (Codex)"
```

Then `/model` lists them. `auth = "codex"` falls back to `~/.codex/auth.json` when `CODEX_ACCESS_TOKEN` is unset.

`[provider.openai]` + `env_key = "OPENAI_API_KEY"` is the plain API-key path from the official custom-models guide.

## Update + rebuild

```bash
~/bin/grok-update-from-source
```

## Install layout

| Path | Role |
|------|------|
| `~/.grok/local/grok` | Built binary |
| `~/.grok/bin/grok` | Symlink → local build |
| `~/bin/grok-update-from-source` | Fetch + rebase + rebuild |
| `~/.grok/auth.json` | Grok subscription (OAuth) |
| `~/.codex/auth.json` | Codex ChatGPT OAuth (provider.codex) |

## Auth / subscription

Grok sub stays on `~/.grok/auth.json`. Codex models use Codex login separately.

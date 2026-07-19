# Personal grok-build fork notes

This tree tracks [xai-org/grok-build](https://github.com/xai-org/grok-build) with local patches on branch **`personal`**.

## Remotes

| Remote | URL |
|--------|-----|
| `upstream` | `https://github.com/xai-org/grok-build.git` |

## Patches on `personal`

1. **`[ui] show_shortcuts_bar`** — hide/show the bottom contextual shortcuts bar. Default `true`; set `false` in `~/.grok/config.toml`.
2. **Claude-like prompt chrome (GrokNight)** — `prompt_border` / user-message fill match Claude Code dark (`rgb(136,136,136)` border, `rgb(55,55,55)` fill).
3. **Shift+Tab → cycle reasoning effort** (not permission mode). Permission mode still via `Ctrl+O` / `/always-approve` / `/plan`.
4. **OpenAI via Codex auth** — models with `env_key = "CODEX_ACCESS_TOKEN"` read `~/.codex/auth.json` when the env var is unset; ChatGPT-Account-ID is injected for `chatgpt.com` bases. Configure `[model.*]` entries in `~/.grok/config.toml`.

## Update + rebuild

```bash
~/bin/grok-update-from-source
```

That script: `git fetch upstream` → rebase `personal` onto `upstream/main` → release build → install to `~/.grok/local/grok` and symlink `~/.grok/bin/grok`.

On conflicts: fix in this repo, `git rebase --continue`, re-run the script.

## Install layout

| Path | Role |
|------|------|
| `~/.grok/local/grok` | Your built binary |
| `~/.grok/bin/grok` | Symlink → local build (PATH) |
| `~/bin/grok` | Convenience copy |
| `~/.grok/local/stock-backup/grok-stock` | One-time stock backup |
| `~/.grok/auth.json` | OAuth session (Grok subscription) |

## Auth / subscription

Do **not** switch to API-key-only. Browser OAuth credentials in `~/.grok/auth.json` are shared with this binary. Same sub, models, sessions.

## Stock binary

With `[cli] auto_update = false`, the official installer will not overwrite your link. To temporarily run stock (if backup exists):

```bash
~/.grok/local/stock-backup/grok-stock
```

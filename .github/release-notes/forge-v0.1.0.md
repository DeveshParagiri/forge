# Forge 0.1.0

Forge 0.1.0 is the first release under Forge's independent semantic-versioning
scheme. It is synchronized with **Grok Build 0.2.105**.

## Highlights

- **More models:** use SpaceXAI, ChatGPT Codex, and OpenRouter models from one
  terminal interface.
- **Multi-harness subagents:** delegate work across native roles, Claude Code,
  and Codex CLI while retaining Forge's streaming, cancellation, metadata, and
  resume experience.
- **Simpler Forge interface:** consistent Forge branding and a cleaner default
  theme.
- **Packaged updates:** `grok update` downloads a checksummed release binary and
  replaces the installed executable atomically without touching `~/.grok/`
  configuration, authentication, or sessions.

## Fast mode indicator

Codex `/fast` remains capability-driven and provider-aware. When enabled, the
model label now uses the text-presentation lightning symbol **`⚡︎`** rather than
a colored emoji, so it follows the active terminal theme.

## External sessions

External harness sessions are hidden by default. To show locally available
Claude Code and Codex sessions under `/sessions`, add:

```toml
[sessions]
show_external = true
```

Rows are labeled `Claude Code` or `Codex`. Selecting one starts a fresh Forge
session and invokes `/resume-claude <id>` or `/resume-codex <id>` to import the
useful continuation context.

## Supported release platforms

- macOS Apple Silicon (`aarch64-apple-darwin`)
- Linux x86_64 (`x86_64-unknown-linux-gnu`)
- Linux AArch64 (`aarch64-unknown-linux-gnu`)

Intel macOS is intentionally not published.

## Install or update

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

Existing Forge installations can update with:

```sh
grok update
```

Checksums are published beside every archive. On macOS, the installer ad-hoc
signs and verifies the installed executable.

## Architecture and maintenance

Substantive fork behavior lives in additive, crate-local `forge/` modules.
Upstream-owned files contain only narrow integration hooks. Forge development is
integrated on `dev`, validated and fast-forwarded to `main`, and synchronized
explicitly with `upstream/main` outside the end-user update path.

See [`CHANGELOG.md`](../../CHANGELOG.md) for the complete Forge change history.

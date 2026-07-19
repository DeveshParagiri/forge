# Forge 0.2.0

Forge 0.2.0 is synchronized with **Grok Build 0.2.106** and expands Forge's
multi-harness workflow, shortcut architecture, and adaptive orchestration
memory.

## External sessions in `/sessions`

Saved Claude Code and Codex CLI sessions can now appear directly in the
`/sessions` dashboard. External sessions remain opt-in:

```toml
[sessions]
show_external = true
```

The existing `[compat.claude].sessions` and `[compat.codex].sessions` settings
remain per-harness kill switches. Cursor sessions are not included in this
release.

Each external row identifies its harness once on the secondary line as
`Claude Code` or `Codex CLI`. Duplicate title badges and foreign `HEAD` branch
markers have been removed. Selecting an external row creates a fresh Forge
session and invokes `/resume-claude <id>` or `/resume-codex <id>`; Forge never
passes a foreign native ID through ACP `session/load`.

## Extensible prompt shortcuts

Prompt shortcuts now resolve through an extensible action registry.
`Shift+Tab` consistently cycles the active model's supported reasoning-effort
levels across color themes, and the help/footer labels use the same binding
metadata as dispatch.

## Adaptive orchestration memory

When memory is enabled, the existing flush and consolidation flow can learn
durable preferences about which models, external harnesses, and orchestration
patterns fit different tasks. This extends the existing semantic memory flow;
it does not add a separate preference database.

## Maintenance and release reliability

- Forge's embedded user guide now documents capability-aware, cost-conscious
  orchestration and the Forge-specific shortcut behavior.
- A scheduled workflow can prepare daily upstream synchronization pull requests
  without rewriting published history.
- Release publication checks out the tagged source before downloading build
  artifacts, preventing checkout cleanup from deleting release archives.

## Supported platforms

- macOS Apple Silicon (`aarch64-apple-darwin`)
- Linux x86_64 (`x86_64-unknown-linux-gnu`)
- Linux AArch64 (`aarch64-unknown-linux-gnu`)

## Install or update

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

Existing installations can update after the release finishes publishing:

```sh
grok update
```

Checksums are published beside every archive. See
[`CHANGELOG.md`](../../CHANGELOG.md) for the complete Forge release history.

# Forge Changelog

This changelog tracks Forge releases independently from the synchronized upstream
Grok Build version. Forge release tags use `forge-vMAJOR.MINOR.PATCH`; the
upstream base is recorded in each release entry.

## [Unreleased]

**Development upstream base:** Grok Build 1.0.0

### Changed

- Synchronized `dev` with Grok Build 1.0.0, preserving Forge's provider,
  credential-isolation, Fast Mode, external-harness, session, branding, and
  upstream-sync extensions.
- Integrated upstream's nonce-protected usage modal with Forge's provider-aware
  account usage. SpaceXAI sessions use native billing, while ChatGPT Codex
  sessions fetch Codex quota without calling xAI billing APIs.

### Fixed

- The dashboard header no longer displays the remote xAI subscription upgrade
  CTA for ChatGPT Codex, OpenRouter, custom, or not-yet-resolved providers.
  SpaceXAI sessions retain the upstream upgrade surface.

## [0.3.2] - 2026-08-09

**Upstream base:** Grok Build 0.2.121

### Changed

- Forge is synchronized with Grok Build 0.2.121, including upstream session and
  dashboard lifecycle improvements, background-task and compaction reliability,
  prompt queue controls, authentication fixes, safer workspace permissions,
  improved error handling, and terminal rendering updates.
- The synchronization retains Forge’s narrow ChatGPT Codex compatibility,
  fail-closed endpoint and credential isolation, session-scoped Fast Mode,
  provider-aware controls, external Claude Code and Codex CLI sessions, Forge
  branding, and the local and scheduled upstream-sync tooling.

## [0.3.1] - 2026-07-31

**Upstream base:** Grok Build 0.2.116

### Changed

- Forge is synchronized with Grok Build 0.2.116, bringing upstream improvements
  to streamed JSON tool results and usage, `/undo`, mode-aware slash-command
  visibility, OAuth refresh and login reliability, MCP credentials and OAuth,
  LSP stability, worktree lifecycle, and terminal handling.
- The synchronization retains Forge’s narrow subscription-backed ChatGPT Codex
  compatibility, session-scoped Fast Mode, provider-aware controls, external
  Claude Code and Codex CLI harnesses, and portable cross-provider history.

### Fixed

- xAI-only bearer resolution, private request headers, and backend recovery
  metadata remain positively gated to trusted xAI endpoints during the upstream
  request-path refactor. ChatGPT Codex fallback remains limited to its canonical
  endpoint, and Responses requests continue to use `store = false`.

## [0.3.0] - 2026-07-27

**Upstream base:** Grok Build 0.2.112

### Added

- A fail-closed endpoint policy positively identifies official xAI and ChatGPT
  Codex HTTPS endpoints before enabling provider-specific credentials, headers,
  or response metadata.
- Portable cross-provider history normalization removes opaque reasoning and
  flattens backend-only tool calls when switching provider families.

### Changed

- Generic third-party providers now use upstream `[model_providers.*]`
  configuration and credentials instead of Forge-owned provider machinery.
- ChatGPT subscription support is reduced to a narrow Codex OAuth and Responses
  compatibility layer. Codex requests always send `store = false`, and Fast
  Mode remains session-scoped through `service_tier = "priority"`.
- Streamed Codex reasoning summaries preserve indexed part boundaries so
  adjacent Markdown sections do not render as glued text.
- Forge is synchronized with Grok Build 0.2.112, including the latest startup,
  workflow, session replay, and background-task improvements.

### Removed

- The legacy Forge `[provider.*]` configuration, provider key store, provider
  status machinery, and provider-login UI. Existing third-party providers must
  be configured through upstream `[model_providers.*]`.
- OpenRouter-specific credential storage; OpenRouter now uses ordinary upstream
  provider API-key or environment-key configuration.

### Fixed

- xAI bearer tokens and private request metadata can no longer reach unknown,
  cleartext, proxy, or third-party endpoints.
- Codex OAuth-file fallback is limited to the canonical ChatGPT Codex endpoint
  and cannot be activated merely by reusing the expected environment key name.
- Provider-aware credential reload now resolves the complete active model,
  including provider defaults and overrides.

## [0.2.0] - 2026-07-19

**Upstream base:** Grok Build 0.2.106

### Added

- Saved Claude Code and Codex CLI sessions now appear directly in the
  `/sessions` dashboard when `[sessions].show_external = true`. External rows
  use the existing compatibility switches for per-harness control, and
  selecting one imports its context into a fresh Forge session rather than
  loading a foreign ID through ACP.
- Prompt shortcuts now use an extensible action registry. `Shift+Tab` cycles
  supported reasoning-effort levels consistently across color themes, while
  help and footer hints are derived from the same binding metadata.
- The existing memory flush and consolidation flow can learn durable model,
  harness, and orchestration preferences from explicit feedback and clear task
  outcomes.
- A scheduled upstream-sync workflow can prepare daily synchronization pull
  requests without rewriting published Forge history.

### Changed

- External session rows show the harness once on the secondary line (`Claude
  Code` or `Codex CLI`) and omit duplicate title badges and foreign `HEAD`
  branch markers.
- Forge orchestration and shortcut behavior is documented in the embedded user
  guide, including capability-aware delegation and provider-cost preferences.

### Fixed

- Release publication now preserves downloaded artifacts by checking out the
  tagged source before the artifact download step.

## [0.1.0] - 2026-07-19

**Upstream base:** Grok Build 0.2.105

### Added

- Multiple provider choices in one TUI, including SpaceXAI, ChatGPT Codex, and
  OpenRouter models.
- Provider-aware `/usage` and Codex OAuth `/fast` support.
- External subagent harness adapters for Claude Code and Codex CLI, with native
  Forge streaming, cancellation, metadata, and resume integration.
- Opt-in Claude Code and Codex entries in `/sessions`. Enable them with
  `[sessions].show_external = true`; selecting an external entry starts a fresh
  Forge session through the matching `/resume-*` skill.
- Checksummed prebuilt release artifacts and an atomic `grok update` flow that
  preserves configuration, authentication, and sessions.
- Maintainer workflows for upstream synchronization and fast-forward-only
  publication from `dev` to `main`.

### Changed

- Fast mode now uses the text-presentation lightning symbol `⚡︎`, allowing the
  terminal theme to control its color instead of forcing emoji presentation.
- External `/sessions` rows display the full harness name, such as `Claude Code`
  or `Codex`.
- Forge-specific behavior is isolated in additive, crate-local `forge/` modules
  with narrow hooks into upstream code.
- macOS release artifacts target Apple Silicon. Linux artifacts target x86_64
  and AArch64.
- The product name and default theme are consistently `Forge`.

### Removed

- Former branding aliases and compatibility behavior.
- Intel macOS release builds, which could indefinitely block Apple Silicon and
  Linux publication.

## Pipeline trial: forge-v0.2.105.1 - 2026-07-19

The initial pipeline-validation tag is retained for provenance. It predates
Forge SemVer and is not part of the `0.x` release sequence.

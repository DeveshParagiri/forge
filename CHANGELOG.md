# Forge Changelog

This changelog tracks Forge releases independently from the synchronized upstream
Grok Build version. Forge release tags use `forge-vMAJOR.MINOR.PATCH`; the
upstream base is recorded in each release entry.

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

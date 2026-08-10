# Forge 0.4.0

Forge 0.4.0 is synchronized with **Grok Build 1.0.0** while preserving Forge's provider isolation, provider-aware controls, Fast Mode, external harnesses, session behavior, branding, and upstream synchronization tooling.

## Provider-aware usage

Forge now integrates Grok Build's nonce-protected usage modal with provider-specific account usage:

- SpaceXAI sessions use native xAI billing.
- ChatGPT Codex sessions fetch Codex quota directly without calling xAI billing APIs.
- OpenRouter, custom, and unresolved providers fail closed with provider-appropriate availability text.
- Stale asynchronous responses cannot overwrite a newer or reopened usage modal.

## Subscription upgrade surface

The dashboard-header xAI subscription upgrade CTA is now shown only for SpaceXAI sessions. ChatGPT Codex, OpenRouter, custom, and not-yet-resolved providers no longer receive an irrelevant xAI upgrade prompt.

## External harness lifecycle

External Claude Code and Codex CLI process groups are enrolled in the owning session's process scope. Session teardown now reaps those children, and late launches are rejected if their owning session is already closing.

## Upstream synchronization

This release incorporates Grok Build 1.0.0's session, usage, MCP, workspace, prompt queue, replay, terminal, and reliability improvements while retaining Forge's narrow provider and orchestration extensions.

## Supported platforms

- macOS Apple Silicon (`aarch64-apple-darwin`)
- Linux x86_64 (`x86_64-unknown-linux-gnu`)
- Linux AArch64 (`aarch64-unknown-linux-gnu`)

## Install or update

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

Existing installations can update after release artifacts finish publishing:

```sh
grok update
```

Checksums are published beside every archive. See [`CHANGELOG.md`](../../CHANGELOG.md) for the complete Forge release history.

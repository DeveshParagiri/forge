# Forge 0.3.0

Forge 0.3.0 is synchronized with **Grok Build 0.2.112**. This release simplifies Forge's provider architecture while preserving ChatGPT subscription-backed Codex models, provider-aware controls, and external Claude Code and Codex CLI harnesses.

## Provider architecture

Generic third-party providers now use upstream `[model_providers.*]` configuration and credentials. Forge no longer maintains a parallel `[provider.*]` configuration format, provider key store, status layer, or provider-login interface.

ChatGPT subscription support remains available through a narrow compatibility layer. Forge reads Codex OAuth data from `~/.codex/auth.json` only for the canonical ChatGPT Codex endpoint. Fast Mode remains session-scoped and sends `service_tier = "priority"` only for models that explicitly support it.

OpenRouter and other generic providers use ordinary upstream API-key or environment-key configuration. Forge does not store their credentials.

## Endpoint and data isolation

Provider-specific behavior now fails closed:

- xAI bearer tokens and private request metadata are stripped from unknown, cleartext, proxy, and third-party endpoints.
- Codex OAuth-file fallback cannot be activated by merely reusing the expected environment-key name on another endpoint.
- ChatGPT Codex Responses requests always send `store = false`.
- Cross-provider history drops opaque reasoning and flattens backend-only tool calls while retaining portable conversation context.

## Codex compatibility

- Streamed reasoning summaries preserve indexed part boundaries, preventing adjacent Markdown sections from rendering as glued text.
- Credential reload resolves the complete active model, including provider defaults and model overrides.
- Provider-aware `/usage`, reasoning-effort controls, and Claude Code/Codex CLI harness integrations remain supported.

## Upstream synchronization

This release includes Grok Build 0.2.112 improvements such as faster nonblocking startup, bounded memory during large session replay, corrected plan-mode ordering, improved `/loop` termination prompts, configurable subagent nesting, leader-process sandboxing, and cleaner background-task snapshots.

## Migration

Replace legacy Forge provider configuration with upstream provider entries:

```toml
[model_providers.openrouter]
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
```

Model entries select the provider with `model_provider`. Run `codex login` to populate or refresh ChatGPT Codex OAuth credentials.

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

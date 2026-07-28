# Forge multi-provider handoff

**Audience:** Maintainers working on the personal Forge fork.

**Updated:** 2026-07-26.
**Status:** The Pi-inspired provider-switch correctness work is implemented. Generic provider configuration is owned by upstream Grok Build; Forge retains only narrow behavior that upstream does not provide.

This document records the current architecture. Do not restore the old `[provider.*]` compatibility layer, provider-login UI, provider status UI, or `~/.grok/provider_keys.json` key store.

---

## 1. Product requirements

Forge must preserve:

- SpaceXAI as the default first-party provider and the stock `/login` flow.
- ChatGPT Plus/Pro-backed GPT models through the Codex CLI's OAuth credentials.
- OpenRouter and arbitrary custom endpoints through upstream model-provider configuration.
- Session-scoped Fast Mode for supported ChatGPT models.
- Provider-aware `/usage`.
- Claude Code and Codex CLI external subagent harnesses.
- Shift+Tab reasoning-effort cycling in the Forge theme.
- Safe mid-session provider switching without replaying foreign opaque reasoning.
- Fail-closed credential and private-header isolation for every non-xAI endpoint.

The maintenance goal is to keep fork-specific behavior additive and small. Generic endpoint, credential, header, and model inheritance logic belongs upstream.

---

## 2. Configuration ownership

### 2.1 Generic providers

Use upstream `[model_providers.<id>]` and point models at the provider with `model_provider = "<id>"`.

```toml
[model_providers.example]
base_url = "https://api.example.com/v1"
api_backend = "chat_completions"
env_key = "EXAMPLE_API_KEY"
extra_headers = { "X-Client" = "grok-build" }

[model.example-model]
model = "provider/model-id"
model_provider = "example"
name = "Example Model"
context_window = 200000
```

Provider blocks can supply `base_url`, `api_base_url`, `api_backend`, `api_key`, `env_key`, `extra_headers`, `query_params`, `env_http_headers`, `auth_provider`/`auth`, and `context_window`. A model inherits those values by naming the provider. Model-local values take precedence according to upstream model-provider rules.

The old `[provider.*]` syntax is removed. It is not translated or activated; config loading reports it as an unrecognized key.

### 2.2 ChatGPT subscription through Codex

Authenticate outside Forge:

```bash
codex login
```

The narrow Forge credential shim reads `access_token` and `account_id` from `~/.codex/auth.json`. File fallback is enabled only when the configured provider explicitly names `CODEX_ACCESS_TOKEN` or `OPENAI_CODEX_TOKEN`; unrelated providers cannot acquire the Codex token accidentally.

```toml
[model_providers.codex]
base_url = "https://chatgpt.com/backend-api/codex"
api_backend = "responses"
env_key = "CODEX_ACCESS_TOKEN"

[model_providers.codex.extra_headers]
OpenAI-Beta = "responses=experimental"
originator = "codex_cli_rs"

[model.gpt-5.6-sol]
model = "gpt-5.6-sol"
model_provider = "codex"
name = "Sol"
context_window = 400000
supports_fast_mode = true
```

The static `OpenAI-Beta` and `originator` headers belong in configuration. Forge injects only the dynamic `ChatGPT-Account-Id` header from the Codex-owned auth file.

### 2.3 OpenRouter

Prefer an environment variable so the API key is not written to disk:

```toml
[model_providers.openrouter]
base_url = "https://openrouter.ai/api/v1"
api_backend = "chat_completions"
env_key = "OPENROUTER_API_KEY"

[model_providers.openrouter.extra_headers]
HTTP-Referer = "https://github.com/xai-org/grok-build"
X-Title = "Grok Build (personal)"

[model.openrouter-auto]
model = "openrouter/auto"
model_provider = "openrouter"
name = "OpenRouter Auto"
context_window = 200000
```

A static `api_key` remains available because upstream supports it, but environment-backed configuration is preferred. Forge does not read or write `~/.grok/provider_keys.json`, and `/login` does not collect OpenRouter keys.

### 2.4 Provider-scoped catalog filtering

Forge keeps a small, optional catalog filter:

```toml
[catalog.spacexai]
exclude = ["unused-model"]

[catalog.openai_codex]
include = ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]

[catalog.openrouter]
include = ["openrouter-auto", "claude-*", "gemini-*"]
```

An absent provider section leaves the upstream catalog untouched. In a present section, an empty `include` means all models for that provider, and `exclude` always wins. Patterns use the same `*`/`?` glob behavior as upstream model filters.

---

## 3. Fork-owned behavior

### 3.1 Codex Responses adapter

Only the official HTTPS endpoint family is recognized as ChatGPT Codex:

```text
https://chatgpt.com/backend-api/codex[/...]
```

Matching is based on a parsed URL, exact host, and path-prefix boundary. Substring lookalikes, userinfo tricks, proxies with a similar path, malformed URLs, and cleartext HTTP are treated as ordinary untrusted endpoints.

For recognized Codex requests, Forge:

- lifts system/developer input into top-level `instructions`;
- removes fields rejected by the ChatGPT Codex endpoint;
- applies Fast Mode as `service_tier = "priority"` only while enabled;
- repairs unsupported hosted-tool/history items where needed;
- clamps response item IDs to the endpoint's 64-byte limit;
- tolerates known additive/liveness SSE events;
- recovers streamed terminal text or function calls when the terminal payload is incomplete.

It intentionally does not invent generic Responses defaults for unrelated providers.

### 3.2 Fast Mode

Fast Mode is a session flag, not a different model. A model must declare:

```toml
supports_fast_mode = true
```

When enabled on a supported ChatGPT model, the request contains:

```json
{ "service_tier": "priority" }
```

Switching to a model that does not advertise support clears the effective Fast Mode state for that sampling configuration. The normal model ID remains unchanged.

### 3.3 Provider-switch history normalization

When the provider scope changes, Forge normalizes existing history before the next request:

| Conversation item | Cross-provider behavior |
|---|---|
| User text | Keep |
| Assistant text | Keep |
| Portable frontend tool calls/results | Keep |
| Opaque `Reasoning` | Remove |
| Provider-specific `BackendToolCall` | Convert to assistant text using its portable summary |

This avoids replaying foreign encrypted/opaque reasoning while retaining useful conversational and tool context. Switching models within the same provider scope does not run this transform.

### 3.4 Provider-aware usage

- SpaceXAI usage remains on the stock billing-extension path.
- ChatGPT Codex usage is fetched from the ChatGPT account usage endpoint with the Codex OAuth token and account ID.
- Providers without a stable account usage API return an explicit unsupported result rather than an authentication error.

Missing or stale ChatGPT credentials direct the user to `codex login`.

### 3.5 External harnesses

Claude Code and Codex CLI remain external subagent harnesses. Their process/session composition is separate from inference-provider configuration and must not be folded into provider credentials or the pager login flow.

---

## 4. Security invariants

These rules are mandatory:

1. **xAI-private request metadata uses a positive HTTPS allowlist.** Only exact `x.ai`/`*.x.ai` and `grok.com`/`*.grok.com` hosts are trusted.
2. **Unknown means third party.** Custom, malformed, proxied, and cleartext endpoints fail closed.
3. **Every request shape is filtered at the final client boundary.** Chat Completions, Responses, and Messages; streaming and non-streaming paths all share the same policy.
4. **Never send xAI bearer/private metadata to third parties.** `x-grok-*` and `x-xai-token-auth` headers are stripped, and xAI-specific request headers are not applied.
5. **Preserve provider-owned data.** The provider's `Authorization` or `x-api-key`, explicitly configured headers, standard tracing headers, and user agent remain intact.
6. **Do not log credential-derived prefixes.** Diagnostics may report whether an auth header is present, never token contents or prefixes.
7. **Codex file fallback is explicit.** Only Codex-named environment keys enable reading `~/.codex/auth.json`.

Adversarial tests cover suffix domains, userinfo attacks, path lookalikes, HTTP URLs, and unknown custom endpoints.

---

## 5. Important files

| Area | Path |
|---|---|
| Endpoint trust policy | `crates/codegen/xai-grok-sampler/src/forge/endpoint_policy.rs` |
| Codex request adapter | `crates/codegen/xai-grok-sampler/src/forge/codex_responses.rs` |
| Final request/header boundary | `crates/codegen/xai-grok-sampler/src/client.rs` |
| Codex integration regression | `crates/codegen/xai-grok-sampler/tests/forge_codex_responses.rs` |
| Upstream provider parser/inheritance | `crates/codegen/xai-grok-shell/src/agent/model_providers.rs` |
| Provider identity | `crates/codegen/xai-grok-shell/src/agent/forge/identity.rs` |
| Narrow Codex credentials | `crates/codegen/xai-grok-shell/src/agent/forge/credentials.rs` |
| Dynamic Codex request profile | `crates/codegen/xai-grok-shell/src/agent/forge/profile.rs` |
| Portable history transform | `crates/codegen/xai-grok-shell/src/agent/forge/history.rs` |
| Provider usage | `crates/codegen/xai-grok-shell/src/agent/forge/usage.rs` |
| Model-switch hook | `crates/codegen/xai-grok-shell/src/session/acp_session_impl/model_switch.rs` |
| Sampler construction hook | `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` |
| Pager usage rendering | `crates/codegen/xai-grok-pager/src/forge/provider_usage.rs` |
| Maintenance source of truth | `FORK-MAINTENANCE.md` |

`agent/provider_auth.rs` and `agent/provider_history.rs` are compatibility facades only. New implementation belongs under `agent/forge/`.

---

## 6. Verification

Run focused checks after provider or upstream-sync changes:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p xai-grok-sampler --lib forge::
cargo test -p xai-grok-sampler --lib all_third_party_request_shapes_strip_xai_headers_after_injection
cargo test -p xai-grok-sampler --test forge_codex_responses
cargo test -p xai-grok-shell --lib agent::forge
cargo test -p xai-grok-shell --lib legacy_provider_section_is_unrecognized_and_inert
cargo test -p xai-grok-shell --lib fast_mode
cargo test -p xai-grok-pager --lib forge
cargo check -p xai-grok-sampler -p xai-grok-shell -p xai-grok-pager-bin
```

Before installing a test binary, build it separately and obtain explicit approval before replacing `~/.grok/bin/grok`. Do not push, tag, publish, or remove old credential files as part of local verification.

---

## 7. Non-goals

- Reimplementing upstream generic provider configuration.
- A fork-owned provider login/status UI.
- A Forge OpenRouter key store or live model catalog.
- Treating Fast Mode as a model alias.
- Sending xAI-private metadata based on a substring or a "not Codex" check.
- Replaying opaque provider reasoning across provider boundaries.
- Replacing SpaceXAI as the default provider.

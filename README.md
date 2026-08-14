<div align="center">

<h1>Forge</h1>

**Forge** is an independent, upstream-friendly extension of
[Grok Build](https://github.com/xai-org/grok-build), the terminal coding agent.
It keeps the native Grok workflow while adding provider choice, a focused
interface, and first-class orchestration across native and external coding
harnesses.

[Install](#install-forge) ·
[Configure](#authentication-and-models) ·
[Features](#extended-features) ·
[Build](#requirements-and-source-builds) ·
[Develop](#maintainer-workflow)

![Forge TUI](docs/assets/forge-tui.jpg)

**Multi-model and multi-harness orchestration in the native Grok terminal workflow.**

Forge is not an official SpaceXAI distribution. The `main` branch is the stable,
installable Forge channel; development is integrated on `dev` and periodically
synchronized with upstream Grok Build.

</div>

---

## Install Forge

Install the latest checksummed release:

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

The installer selects the archive for macOS Apple Silicon or Linux ARM64;
verifies its SHA-256 checksum; and installs the canonical executable
at `~/.grok/bin/grok`. It also refreshes compatibility links without changing
your shell configuration.

On Windows x86_64, use PowerShell:

```powershell
irm https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install.ps1 | iex
```

That installs `%USERPROFILE%\.grok\bin\grok.exe` and adds it to your user PATH.

Launch Forge:

```sh
grok
```

Update to the latest Forge release at any time:

```sh
grok update
```

Configuration, authentication, sessions, and memory remain under `~/.grok/`.
`grok update` installs another packaged release. It does not need Rust or alter
a source checkout.

### Install a specific release

Pass a Forge version with or without the `forge-v` prefix:

```sh
FORGE_VERSION=0.5.0 \
  curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

```powershell
$env:FORGE_VERSION = "0.5.0"
irm https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install.ps1 | iex
```

## Extended features

| Feature | What Forge adds |
|---|---|
| **Models and providers** | Use SpaceXAI, ChatGPT Codex, and upstream-compatible third-party providers in one model picker. Provider capabilities, credentials, and request policy remain isolated. |
| **Fast Mode and usage** | Toggle session-scoped Codex Fast Mode with `/fast` on models that advertise support. `/usage` follows the active provider's supported accounting path. |
| **Multi-model subagents** | Delegate independent implementation, research, planning, and verification work to native roles with explicit model and reasoning-effort choices. |
| **External harnesses** | Run Claude Code and Codex CLI as subagent harnesses through their own installations, authentication, model defaults, and tools. |
| **Unified sessions** | Browse Forge, Claude Code, and Codex sessions together in `/sessions`. Importing an external row creates a new Forge session with its context. |
| **Private phone remote** | Open the current Forge session on a phone with `/rc`. Use the bundled browser client or a locally built iOS app over Tailscale Serve. `/rc stop` revokes that session's pairing. |
| **Adaptive memory** | When enabled, the existing memory lifecycle tracks explicitly stated current work plus durable preferences and corrections, while excluding execution noise. Current instructions always win. |

## Authentication and models

The exact model roster depends on the providers configured and authenticated for
the current installation. Forge uses the stock SpaceXAI login flow, the Codex
CLI's OAuth file for ChatGPT subscription models, and upstream Grok Build's
generic model-provider configuration for other endpoints.

### SpaceXAI

Start Forge and use `/login`, or run:

```sh
grok login
```

This remains the stock first-party authentication path.

### ChatGPT Codex

Install and authenticate the official Codex CLI separately:

```sh
codex login
```

Forge can read the resulting `~/.codex/auth.json` only for an explicitly
configured Codex provider at the canonical ChatGPT Codex HTTPS endpoint. A
minimal provider block looks like this:

```toml
# ~/.grok/config.toml
[model_providers.codex]
base_url = "https://chatgpt.com/backend-api/codex"
api_backend = "responses"
env_key = "CODEX_ACCESS_TOKEN"

[model_providers.codex.extra_headers]
OpenAI-Beta = "responses=experimental"
originator = "codex_cli_rs"

[model."gpt-5.6-sol"]
model_provider = "codex"
model = "gpt-5.6-sol"
name = "GPT-5.6 Sol"
context_window = 200000
supports_reasoning_effort = true
supports_fast_mode = true
```

Select the model from the picker or set it as the default:

```toml
[models]
default = "gpt-5.6-sol"
default_reasoning_effort = "high"
```

`/fast` is session-scoped and sends priority service-tier metadata only when the
active model declares `supports_fast_mode = true`. `/usage` uses the ChatGPT
account usage endpoint for this provider. Missing or expired Codex credentials
are repaired with `codex login`, not Forge's `/login` flow.

## Private phone remote

Forge Remote opens the current live session in the bundled browser client or
the optional Forge iOS app. It does not start another agent session and does
not require T3 Code or a T3 account.

1. Install Tailscale and sign in to the same tailnet on the laptop and phone.
   MagicDNS and Tailscale HTTPS must be available for the laptop.
2. In the active Forge session, run `/rc`. Forge checks Tailscale, creates a
   short-lived local pairing gateway, and enables one tailnet-private HTTPS
   route. `/rc enable` is a compatibility alias; no second command is needed.
   Forge never uses Funnel or public sharing.
3. Scan the QR code or open the displayed URL on the phone. The page offers the
   pairing to the Forge iOS app, if installed, and retains a browser option.
   Either client attaches to that session and shows its transcript and current
   interactions. Available controls include prompts, stop, `/btw`, model and
   reasoning changes, usage refresh, and interaction responses when the active
   session advertises those capabilities.
4. Use the laptop and phone together. Inputs from either are sent to the same
   live session and updates appear on both. Run `/rc stop` in that session to
   revoke only its URL and remove only its Forge route.

The QR/link contains a fresh 256-bit pairing secret and expires after eight
hours. Forge then revokes the pairing and removes its own route. Running `/rc`
in other live sessions creates independent links, and `/rc status` reports the
pairing for the current terminal session.

Each pairing has one active phone surface. The visible browser or visible iOS
app owns the phone connection, and opening one transfers ownership from the
other. The terminal remains connected and usable. Both clients keep a local
list of scanned pairings so you can switch between remote sessions.

The composer has one trailing action. It sends while idle, changes in place to
Stop while the current turn can be cancelled, and recognizes `/btw <question>`
as a side question without exposing a separate BTW mode or button.

The browser source is under `crates/codegen/xai-grok-shell/remote-ui`. The
native client under [`clients/forge-mobile`](clients/forge-mobile/README.md)
uses the pinned MIT-licensed T3 Code React Native presentation source with a
Forge protocol controller. The iOS client is a local source build. There is no
public App Store build, universal IPA, or free TestFlight link; its README has
the Xcode installation command and exact license provenance.

### Other OpenAI-compatible providers

Forge retains upstream `[model_providers.*]` and `[model.*]` configuration. For
example:

```toml
# ~/.grok/config.toml
[model_providers.example]
base_url = "https://api.example.com/v1"
api_backend = "chat_completions"
env_key = "EXAMPLE_API_KEY"

[model.example-model]
model_provider = "example"
model = "provider/model-id"
name = "Example Model"
context_window = 200000
```

Prefer `env_key` over writing a static `api_key` to disk. Forge does not maintain
a separate third-party key store or provider-login UI. Generic endpoint,
credential, header, and model inheritance behavior comes from upstream Grok
Build. Forge enables xAI session credentials only for trusted xAI HTTPS
endpoints and strips xAI-private headers at the final request boundary for
unknown, cleartext, custom-proxy, and third-party endpoints.

For the complete schema, including query parameters, environment-backed headers,
and model overrides, see the
[custom-model guide](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md).

## External harnesses and sessions

External subagents require the corresponding official CLI to be installed and
authenticated separately:

```sh
claude --version
codex --version
```

Forge discovers available harnesses at runtime. Neither CLI is required for
normal Forge use, and installing only one enables only that adapter. External
harnesses use provider-native tools rather than Forge-hosted tools; omitting an
explicit model uses the harness's native default.

External sessions are hidden from `/sessions` by default. Enable local discovery
in `~/.grok/config.toml`:

```toml
[sessions]
show_external = true
```

Rows are labeled `Claude Code` or `Codex`. Selecting one creates a fresh Forge
session and invokes the existing `/resume-claude` or `/resume-codex` context
import; it does not reopen the external harness UI or treat a foreign session ID
as a native Forge session.

## Privacy and credential boundaries

- Forge configuration, sessions, and memory are stored under `~/.grok/`.
- Codex OAuth data remains owned by the Codex CLI in `~/.codex/auth.json`.
- External session stores are read only when external-session discovery is
  enabled, and imports create a new Forge session.
- Forge does not create an OpenRouter- or third-party-specific credential store.
  Environment-backed API keys are preferred for generic providers.
- Requests and prompts go to the provider selected for that session. Provider
  switches remove opaque provider reasoning and flatten nonportable backend tool
  history before the next request.
- xAI session credentials are resolved only for trusted xAI HTTPS endpoints;
  xAI-private headers are also stripped from untrusted requests at the final
  sampler boundary.
- Forge's adaptive preference behavior uses the existing memory lifecycle; it
  adds no separate analytics service or credential-bearing record format.

The shared Grok telemetry controls and data categories are documented in the
[monitoring and usage guide](crates/codegen/xai-grok-pager/docs/user-guide/24-monitoring-usage.md).

## Requirements and source builds

The release installer requires `curl`, `tar`, and either `shasum` or
`sha256sum`. Ordinary users do not need Rust, Cargo, Git, DotSlash, or `protoc`.

Building from source requires Git, [Rust](https://rustup.rs/), Cargo, and either
[`dotslash`](https://dotslash-cli.com/) or `protoc` on `PATH`. The repository's
[`rust-toolchain.toml`](rust-toolchain.toml) pins Rust **1.94.0**.

Clone the stable branch and build manually:

```sh
git clone --branch main https://github.com/DeveshParagiri/forge.git
cd forge
cargo build --locked -p xai-grok-pager-bin --release
mkdir -p ~/.grok/bin
install -m 755 target/release/xai-grok-pager ~/.grok/bin/grok
```

On macOS, ad-hoc sign a manually installed binary:

```sh
codesign --force --sign - ~/.grok/bin/grok
codesign --verify ~/.grok/bin/grok
```

The installer also supports a source mode. It checks prerequisites but does not
install system packages or alter shell configuration:

```sh
FORGE_INSTALL_MODE=source GROK_BRANCH=main \
  curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

## Maintainer workflow

Forge uses three principal refs:

| Ref | Purpose |
|---|---|
| `upstream/main` | Official source from `xai-org/grok-build` |
| `dev` | Forge integration and upstream synchronization |
| `main` | Validated, published, installable Forge |

Synchronize a clean local `dev` branch:

```sh
scripts/forge-sync-upstream
```

That helper fetches `upstream/main` and rebases local `dev`; it does not publish.

After resolving conflicts and running the focused checks, publish the exact
validated integration commit without rewriting history:

```sh
scripts/forge-publish main
```

The publisher runs formatting, compilation, and focused Forge tests by default.
It refuses dirty trees, the wrong source branch, and non-fast-forward updates.
Create immutable Forge release tags only after `dev` and `main` point to the same
validated commit.

## License

First-party source is licensed under the [Apache License 2.0](LICENSE).
Third-party and vendored source remains under its original licenses; see
[`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES) and
[`third_party/NOTICE`](third_party/NOTICE).

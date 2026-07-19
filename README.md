# Forge (`grok`)

Forge extends the open-source
[Grok Build](https://github.com/xai-org/grok-build) terminal coding agent with
multi-provider models, provider-safe session switching, and a streamlined
interface.

This is an independent fork, not an official SpaceXAI distribution. Its
`main` branch contains the installable Forge source and tracks upstream
`main`.

## Features

| Area | Added behavior |
|------|----------------|
| Authentication | Interactive `/login` setup for SpaceXAI, ChatGPT Codex, and OpenRouter |
| Models | Provider-aware catalog with configurable include/exclude patterns |
| Sessions | Safe model switching across provider families |
| Interface | Forge theme, reasoning-effort controls, optional shortcut footer, and `Esc` cancellation |
| Branding | Forge welcome screen and build version |

## Architecture

Fork-specific behavior is isolated in per-crate `forge/` modules, with
small integration hooks in upstream code. See
[`FORK-MAINTENANCE.md`](FORK-MAINTENANCE.md) for implementation and rebase notes.

Existing configurations that set `theme = "exaforge"` remain supported as a
legacy read alias. Forge canonicalizes that value to `forge`, and settings UI
selections and subsequent writes use `theme = "forge"`.

## Install

Requires [Rust](https://rustup.rs/) (`cargo`) and either `dotslash` or `protoc`
on `PATH`.

Install directly from GitHub:

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

Or clone first and run the same installer from the checkout:

```sh
git clone https://github.com/DeveshParagiri/forge.git
cd grok-build
./scripts/install
```

The curl pathway clones `main` into `~/.local/share/grok/source`; the checkout
pathway builds the checkout you invoked it from. Both install `grok` at
`~/.grok/bin/grok`. They do not replace configuration, authentication, or
sessions under `~/.grok/`.

Verify the installation:

```sh
grok --version
```

## Update

```sh
~/bin/grok-update-from-source
```

The updater fetches the published `main` branch, fast-forwards the local
checkout, rebuilds, and installs the new binary. It stops without changing
local work when the checkout is dirty or branches have diverged. Rebasing onto
upstream remains an explicit maintainer operation.

## Build manually

```sh
git clone https://github.com/DeveshParagiri/forge.git
cd grok-build
cargo build -p xai-grok-pager-bin --release
mkdir -p ~/.grok/bin
install -m 755 target/release/xai-grok-pager ~/.grok/bin/grok
```

## Development

Target individual crates; full-workspace builds are slow.

```sh
cargo fmt --all
cargo check -p <crate>
cargo test -p <crate>
cargo clippy -p <crate>
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the fork contribution policy.

## License

First-party source is licensed under the [Apache License 2.0](LICENSE).
Third-party and vendored source remains under its original licenses; see
[`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES) and [`third_party/NOTICE`](third_party/NOTICE).

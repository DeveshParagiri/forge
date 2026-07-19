# Exaforge (`grok`)

Exaforge extends the open-source
[Grok Build](https://github.com/xai-org/grok-build) terminal coding agent with
multi-provider models, provider-safe session switching, and a streamlined
interface.

This is an independent fork, not an official SpaceXAI distribution. Its `dev`
branch tracks upstream `main`.

## Features

| Area | Added behavior |
|------|----------------|
| Authentication | Interactive `/login` setup for SpaceXAI, ChatGPT Codex, and OpenRouter |
| Models | Provider-aware catalog with configurable include/exclude patterns |
| Sessions | Safe model switching across provider families |
| Interface | Exaforge theme, reasoning-effort controls, optional shortcut footer, and `Esc` cancellation |
| Branding | Exaforge welcome screen and build version |

## Architecture

Fork-specific behavior is isolated in per-crate `exaforge/` modules, with
small integration hooks in upstream code. See
[`FORK-MAINTENANCE.md`](FORK-MAINTENANCE.md) for implementation and rebase notes.

## Install

Requires [Rust](https://rustup.rs/) (`cargo`) and either `dotslash` or `protoc`
on `PATH`.

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/grok-build/dev/scripts/install | sh
```

The installer clones the `dev` branch, builds the release binary, and installs
`grok` at `~/.grok/bin/grok`. It does not replace configuration, authentication,
or sessions under `~/.grok/`.

Verify the installation:

```sh
grok --version
```

## Update

```sh
~/bin/grok-update-from-source
```

The updater fetches the fork and upstream, fast-forwards local `dev`, rebases it
onto `upstream/main`, rebuilds, and installs the new binary. It stops without
changing local work when the checkout is dirty or branches have diverged.

## Build manually

```sh
cargo build -p xai-grok-pager-bin --release
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

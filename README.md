# Exaforge (`grok`)

Exaforge is Dev Paragiri's personal extension of SpaceXAI's open-source
[Grok Build](https://github.com/xai-org/grok-build) terminal coding agent. It
preserves the upstream Rust CLI, TUI, and ACP runtime while adding
multi-provider models, provider-safe session switching, and a Claude-inspired
interface.

This is an independent fork, not an official SpaceXAI distribution. The
`dev` branch carries the Exaforge extension layer and tracks upstream `main`.

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

## Install or update

Requires Rust (`cargo`) and either `dotslash` or `protoc` on `PATH`.

```sh
git clone --branch dev https://github.com/DeveshParagiri/grok-build.git ~/.local/share/grok/source
mkdir -p ~/bin
install -m 755 ~/.local/share/grok/source/scripts/grok-update-from-source ~/bin/grok-update-from-source
~/bin/grok-update-from-source
grok --version
```

Run `~/bin/grok-update-from-source` again for future updates. It rebases `dev`
onto `upstream/main`, builds the release binary, installs it at
`~/.grok/bin/grok`, and preserves data under `~/.grok/`.

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

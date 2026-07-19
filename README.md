<div align="center">

<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://media.x.ai/v1/website/spacexai-symbol-white-transparent-0c31957f.png">
    <source media="(prefers-color-scheme: light)" srcset="https://media.x.ai/v1/website/spacexai-symbol-black-transparent-6435cf42.png">
    <img alt="SpaceXAI logo" src="https://media.x.ai/v1/website/spacexai-symbol-black-transparent-6435cf42.png" width="96">
  </picture>
  <br>
  Exaforge (<code>grok</code>)
</h1>

**Exaforge** is Dev Paragiri's personal extension of SpaceXAI's open-source
Grok Build terminal coding agent. It preserves the upstream Rust CLI, TUI, and
ACP runtime while adding multi-provider model support, a Claude-inspired UI,
provider-safe session switching, and a small rebase-friendly personalization
layer.

[Fork features](#fork-features) ·
[Installing the fork](#installing-the-fork) ·
[Building from source](#building-from-source) ·
[Documentation](#documentation) ·
[Repository layout](#repository-layout) ·
[Development](#development) ·
[Contributing](#contributing) ·
[License](#license)

![Grok Build TUI](https://media.x.ai/v1/website/universe-tui-screenshot-6f7a0837.png)

**Upstream project: [xai-org/grok-build](https://github.com/xai-org/grok-build)**

This is an independent personal fork, not an official SpaceXAI distribution.
The `dev` branch carries the extension layer and is intended to stay easy to
rebase onto upstream `main`.

A small `SOURCE_REV` file at the root records the full monorepo commit SHA
for the version of the code present in this tree.

</div>

---

## Fork features

| Area | Added behavior |
|------|----------------|
| Multi-provider authentication | Interactive `/login` setup for SpaceXAI, ChatGPT Codex, and OpenRouter |
| Model catalog | Provider-aware models with configurable include/exclude patterns and authentication status in `/model` |
| Provider switching | Safe mid-session model changes across provider families |
| Interface | Claude-inspired theme, reasoning-effort controls, an optional shortcut footer, and polished dashboard and modal layouts |
| Branding | A clean, rebase-friendly Exaforge welcome screen that displays the build version |

The larger architecture and rebase notes live in [`PERSONAL.md`](PERSONAL.md).

### Rebase-friendly extension layout

Most fork-specific implementation now lives in additive, per-crate `exaforge/`
modules:

- `xai-grok-sampler/src/exaforge/` — Codex request policy, Responses API compatibility, and stream recovery.
- `xai-grok-shell/src/agent/exaforge/` — provider identity, credentials, catalogs, profiles, status, and portable history.
- `xai-grok-pager/src/exaforge/` — provider login, effort controls, layout policy, welcome branding, and focused UI tests.
- `xai-grok-pager-render/src/exaforge/` — Claude theme policy and shortcut-footer behavior.

Upstream-owned files retain only small `// Exaforge:` integration hooks where
possible. Ordering-sensitive behavior, such as running-turn `Esc` cancellation,
stays close to the upstream event flow. This keeps features intact while making
future upstream rebases smaller and easier to review.

## Installing the fork

Requires Rust (`cargo`) and either `dotslash` or `protoc` on `PATH`.

```sh
git clone --branch dev https://github.com/DeveshParagiri/grok-build.git ~/.local/share/grok/source
mkdir -p ~/bin
install -m 755 ~/.local/share/grok/source/scripts/grok-update-from-source ~/bin/grok-update-from-source
~/bin/grok-update-from-source
grok --version
```

The updater rebases `dev` onto `upstream/main`, builds the release binary, and
installs it at `~/.grok/bin/grok`. It preserves existing authentication,
configuration, and sessions under `~/.grok/`. Run the same updater command for
future rebuilds.

## Building from source

```sh
cargo build -p xai-grok-pager-bin --release
```

The resulting binary is `target/release/xai-grok-pager`. To install it manually:

```sh
install -m 755 target/release/xai-grok-pager ~/.grok/bin/grok
```

## Documentation

Full online documentation is available at
[docs.x.ai/build/overview](https://docs.x.ai/build/overview).

The user guide ships with the pager crate:
[`crates/codegen/xai-grok-pager/docs/user-guide/`](crates/codegen/xai-grok-pager/docs/user-guide/)
— getting started, keyboard shortcuts, slash commands, configuration, theming,
MCP servers, skills, plugins, hooks, headless mode, sandboxing, and more.

## Repository layout

| Path | Contents |
|------|----------|
| `crates/codegen/xai-grok-pager-bin` | Composition-root package; builds the `xai-grok-pager` binary |
| `crates/codegen/xai-grok-pager` | The TUI: scrollback, prompt, modals, rendering |
| `crates/codegen/xai-grok-shell` | Agent runtime + leader/stdio/headless entry points |
| `crates/codegen/xai-grok-tools` | Tool implementations (terminal, file edit, search, ...) |
| `crates/codegen/xai-grok-workspace` | Host filesystem, VCS, execution, checkpoints |
| `crates/codegen/...` | The rest of the CLI crate closure (config, MCP, markdown, sandbox, ...) |
| `crates/common/`, `crates/build/`, `prod/mc/` | Small shared leaf crates pulled in by the closure |
| `third_party/` | Vendored upstream source (Mermaid diagram stack) — see below |

> [!IMPORTANT]
> The root `Cargo.toml` (workspace members, dependency versions, lints,
> profiles) is **generated** — treat it as read-only. Prefer editing per-crate
> `Cargo.toml` files.

## Development

```sh
cargo check -p <crate>        # always target specific crates; full-workspace builds are slow
cargo test -p xai-grok-config # per-crate tests
cargo clippy -p <crate>       # lint config: clippy.toml at the repo root
cargo fmt --all               # rustfmt.toml at the repo root
```

## Contributing

> [!NOTE]
> External contributions are not accepted. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

First-party code in this repository is licensed under the **Apache License,
Version 2.0** — see [`LICENSE`](LICENSE).

Third-party and vendored code remains under its original licenses. See:

- [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES) — crates.io / git dependencies,
  bundled UI themes, and **in-tree source ports** (including openai/codex and
  sst/opencode tool implementations)
- [`crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)
  — crate-local notice for the codex and opencode ports (license texts +
  Apache §4(b) change notice)
- [`third_party/NOTICE`](third_party/NOTICE) — vendored Mermaid-stack index

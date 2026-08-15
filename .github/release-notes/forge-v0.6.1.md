# Forge 0.6.1

Forge 0.6.1 completes the Forge Remote phone experience, aligns the bundled
browser with the native client, and makes `forge` the canonical executable
without breaking existing `grok` installations.

## Forge Remote parity

The browser and native clients now share the same typed remote protocol for
streamed transcript updates, compact work disclosures, queued-message control,
model and reasoning changes, Fast Mode, usage, attachments, and same-directory
new sessions. The browser presentation mirrors the native thread and grouped
project views, while capability and session checks keep every action bound to
the exact terminal session.

The iOS client includes multiline composer sizing, separate Photos and Files
pickers, rotated-bearer deduplication, immediate scanner dismissal after a
validated pairing, compact work rows, and the updated Forge visual system.

## Installation and updates

The packaged executable is now named `forge`. Installers, release archives,
source builds, test harnesses, and documentation use that canonical name while
retaining `grok` as a compatibility alias. A plain `forge update` or
`grok update` downloads the latest checksummed release and preserves existing
configuration, authentication, sessions, and memory under `~/.grok/`.

This release also makes protobuf dependency discovery portable on Windows so
all advertised release targets can be built by the release workflow.

## Supported platforms

- macOS Apple Silicon (`aarch64-apple-darwin`)
- Linux ARM64 (`aarch64-unknown-linux-gnu`)
- Windows x86-64 (`x86_64-pc-windows-msvc`)

## Install or update

```sh
curl -fsSL https://raw.githubusercontent.com/exaforge/forge/main/scripts/install | sh
```

On Windows:

```powershell
irm https://raw.githubusercontent.com/exaforge/forge/main/scripts/install.ps1 | iex
```

Existing installations can update after the release artifacts finish
publishing:

```sh
forge update
```

Checksums are published beside every release archive. Native iOS source-build
instructions remain in
[`clients/forge-mobile/README.md`](../../clients/forge-mobile/README.md).

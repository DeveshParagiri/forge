# Forge Mobile

Forge Mobile is the optional native iOS client for Forge Remote. It opens an existing Forge TUI session over the private URL created by `/rc`. The same URL also has a bundled browser client, so the native app is not required.

This directory is a source workspace. There is no public App Store build, universal IPA, or free TestFlight link. You can build and install the app on your own iPhone with Xcode and an Apple account. Apple controls the signing lifetime and device limits for free personal teams.

## What works

Forge Mobile stores each scanned pairing in iOS SecureStore and lists the paired sessions on its Home screen. A thread shows the existing transcript and live work from the terminal. The app can send a prompt, stop a cancellable turn, send `/btw <question>`, change the model and reasoning effort when the session supports it, refresh usage, and answer the approval or input cards exposed by the Forge Remote protocol.

Running `/rc` in several TUI sessions creates separate pairings. You can switch between them in the app. Each pairing still has one active phone surface: the visible Forge app or visible browser owns its WebSocket. Opening the app takes ownership from Safari; returning to the browser takes it back. The terminal remains active in both cases.

Forge Mobile does not expose the upstream T3 project, file, Git, terminal, attachment, runtime-mode, cloud-auth, share, widget, or relay features. It does not connect to T3 services and does not require a T3 account.

## Requirements

The private session flow requires Tailscale on the Mac and iPhone, both devices signed into the same tailnet, MagicDNS, and Tailscale HTTPS. The local iOS build requires macOS, Xcode, Node.js 24 or newer, pnpm 11, CocoaPods, an Apple account added to Xcode, and a wired or paired iPhone with Developer Mode enabled.

An iPhone build signed with a free personal team is for local development. It is not a redistributable IPA. TestFlight and App Store distribution require Apple Developer Program membership and release administration outside this repository.

## Install on your own iPhone

Open Xcode once, finish its component installation, and add your Apple account under Xcode > Settings > Accounts. Connect and unlock the iPhone, trust the Mac, then enable Developer Mode in Settings > Privacy & Security.

From this directory, install dependencies and inspect the device and signing values:

```sh
pnpm install
xcrun devicectl list devices
security find-identity -v -p codesigning
```

Choose a bundle ID that is unique to your Apple account. Run the local installer with your team ID and either the device's CoreDevice identifier or hardware UDID:

```sh
pnpm install:ios:local -- \
  --team YOUR_TEAM_ID \
  --device YOUR_DEVICE_ID \
  --bundle-id com.yourname.forge.dev
```

The script regenerates the ignored iOS project from `app.config.ts`, builds the Release configuration with automatic development signing, verifies the Hermes bytecode header and app signature, installs the `.app`, and launches it. Release builds do not need Metro.

Use `--dry-run` to validate arguments and print the commands without changing the generated project or device. Use `--skip-prebuild` only when you intentionally want to reuse the current generated `apps/mobile/ios` project. Full options are available through:

```sh
pnpm install:ios:local -- --help
```

After the app opens, run `/rc` in a Forge TUI session and scan the QR code inside Forge Mobile. Scanning the same code with the standard Camera app opens Safari first; choosing Open in Forge transfers the active phone connection to the app.

## Verify the source workspace

Run these commands from `clients/forge-mobile`:

```sh
pnpm typecheck
pnpm test
pnpm verify:active-graph
pnpm verify:provenance
```

The active app entry is `apps/mobile/src/forge/ForgeApp.tsx`. `apps/mobile/ios` and `apps/mobile/android` are generated and ignored. Persistent native changes belong in `app.config.ts` or a config plugin under `apps/mobile/plugins`.

## Troubleshooting

If Xcode cannot create a provisioning profile, confirm that the Apple account appears in Xcode, use a bundle ID that no other account owns, and pass the team ID associated with that account. If the phone is missing, unlock it, reconnect or pair it in Xcode's Devices and Simulators window, and rerun `xcrun devicectl list devices`. If installation succeeds but launch fails, confirm Developer Mode is enabled and accept any trust prompt on the phone.

If the app shows a missing JavaScript bundle, the installed artifact was a development build. Reinstall with `scripts/install-ios-local.sh`, which builds Release and checks for the embedded bundle. If pairing cannot connect, confirm Tailscale is active on both devices, close stale pages, and run `/rc status` in the matching terminal session. `/rc stop` revokes only that session's URL.

## T3 Code provenance and licenses

The presentation source is derived from `pingdotgg/t3code` at commit `b73232bdd31e83914a8a943960c7dc4b6390b39b`, under the MIT License. [`UPSTREAM.md`](UPSTREAM.md) records the adaptation boundary. [`provenance/t3-mobile-files.json`](provenance/t3-mobile-files.json) records copied files and pinned hashes, while [`provenance/t3-mobile-adaptations.patch`](provenance/t3-mobile-adaptations.patch) records the source changes. The retained MIT text is [`third_party/T3CODE-LICENSE.txt`](third_party/T3CODE-LICENSE.txt), and other notices are in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

Forge's first-party source remains under the repository's [Apache License 2.0](../../LICENSE). Exaforge brand assets and Basier Square fonts are separately supplied brand material; the T3 MIT License does not cover them.

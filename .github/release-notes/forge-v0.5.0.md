# Forge 0.5.0

Forge 0.5.0 adds a private phone client for an exact live Forge terminal
session. It keeps the terminal authoritative while making the session usable
from the bundled browser or the optional native iOS app over Tailscale.

## Forge Remote

Running `/rc` creates a short-lived, tailnet-private HTTPS route and opens a
compact QR modal. The phone receives the existing transcript and live state and
can send prompts, stop cancellable work, ask `/btw` questions, change the model
or reasoning effort, inspect usage, manage queued work and plans, and answer
interactive approval or input cards when the session advertises those
capabilities. `/rc stop` revokes only the current session's pairing.

Several TUI sessions can expose separate pairings at once. Commands are bound
to the exact agent, canonical session, binding epoch, and gateway generation.
The latest authenticated foreground client owns each pairing, so moving between
Safari and Forge Mobile transfers control without command duplication or stale
socket mutation.

## Browser and iOS clients

The bundled browser uses the pinned T3 Code visual source as a documented web
adaptation, with Basier Square typography, mobile-safe keyboard geometry, a
single Send/Stop action, `/btw` recognition, typed interaction cards, and
authoritative model, reasoning, and usage controls.

Forge Mobile retains the pinned MIT-licensed T3 Code native presentation layer
and replaces its cloud controller with Forge's private protocol. It includes
SecureStore pairings, native QR scanning, session switching, foreground
ownership transfer, native safe-area and keyboard behavior, and an embedded
Release bundle that does not require Metro. Its source workspace includes a
verified local installer for free Apple personal-team signing; no paid
TestFlight or App Store account is required for a developer to install their
own build on their own iPhone.

## Reliability and security

Capability URLs contain a fresh 256-bit secret, remain private to Tailscale,
expire after eight hours, and are never accepted from the phone as a session
selector. Commands, asynchronous usage results, interaction responses, and
reconnects are validated against server-owned session and generation state.
Bounded channels, monotonic revisions, full-snapshot recovery, lifecycle
serialization, exact Tailscale route cleanup, and last-client ownership prevent
stale or cross-session control.

This release also makes local billing and free-usage cards dismissible with
Escape, preserves prompt drafts, fixes iOS scanner linkage and dark-mode text
contrast, and keeps the phone composer visible above the iOS keyboard.

## Install or update

```sh
curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh
```

Existing installations can update after release artifacts finish publishing:

```sh
grok update
```

Checksums are published beside the macOS Apple Silicon, Linux x86-64, and Linux
AArch64 archives. Native iOS source-build instructions are in
[`clients/forge-mobile/README.md`](../../clients/forge-mobile/README.md).

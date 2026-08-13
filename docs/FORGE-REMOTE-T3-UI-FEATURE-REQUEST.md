# Feature Request: Forge Remote for the Exact Live TUI Session, with T3 Browser and Native iOS Clients

> **Historical implementation specification.** This file records the original scope and acceptance target. It is not the current setup guide or a completed-test report. Use the root [`README.md`](../README.md#private-phone-remote) for current Forge Remote behavior and [`clients/forge-mobile/README.md`](../clients/forge-mobile/README.md) for the native iOS build, limitations, and T3 provenance. Unchecked items below remain acceptance criteria unless a separate test report proves them.

**Status:** Original implementation handoff / acceptance specification  
**Audience:** An engineer implementing this feature in Forge  
**Repository:** `DeveshParagiri/forge`  
**Do not push, tag, publish, or discard unrelated local work as part of this request.**

---

## 1. One-sentence objective

Add a private phone remote to Forge such that a user can type `/rc` inside **any already-running Forge TUI session**, scan a QR code, and use either a Forge-branded native iOS client based on the **actual MIT-licensed T3 Code mobile source** or a bundled browser client adapted from T3's real web and mobile presentation; the phone and terminal must concurrently control and display the **same exact live session** with its existing context.

The terminal TUI remains the source of truth and must remain usable while the phone is connected.

---

## 2. Non-negotiable user experience

### Desired flow

```text
1. User is already working in any Forge TUI session.
2. User types /rc in that session.
3. Forge arms a private remote bound to that exact active session and enables one scoped, tailnet-private route.
4. Forge prints a tailnet-private HTTPS URL and compact terminal QR code.
5. User scans it from an iPhone (or opens the link in a phone browser).
6. Phone hands the pairing to the native Forge app when installed, otherwise it continues in the Forge browser client.
7. Terminal and phone share one live conversation concurrently.
8. User may use either device at any time; the other device reflects the change.
9. User types /rc stop, or the link expires, and only that remote route is revoked.
```

### Required behavior

- The session can be **new, resumed, direct/default, leader-backed, or otherwise active in Forge**. No special startup flag, configuration switch, restart, separate session, or separate T3-owned thread is acceptable.
- `/rc` binds to the session in the currently active Forge tab/view, not merely a “last session” or any session found through a leader socket.
- Running `/rc` in multiple live sessions creates independent pairings. Each session keeps its own token, loopback listener, Serve path, expiry, phone-client connection, and exact-session bridge; enabling or stopping one never replaces another.
- Opening the phone after work has already happened shows the current session **history and state immediately**, not an empty transcript that only receives future events.
- The terminal TUI and phone are **shared controls**, not an exclusive remote lease:
  - Prompt from phone → appears and runs in terminal session; phone also reflects it.
  - Prompt from terminal → appears on phone.
  - Tool activity, streaming output, background task state, errors, model state, plan state, approvals, and questions appear on both.
  - Closing/reloading the phone never stops or detaches the terminal session.
- The browser UI must be a source-mapped adaptation of real T3 Code **web** UI and a faithful web port of the native T3 mobile geometry, not a hand-made “T3-inspired” static page.
- The native iOS client must reuse the pinned T3 React Native presentation source directly. It keeps the real Home, Thread, feed, composer, interaction cards, navigation, keyboard, safe-area, gesture, and native rendering layers while replacing T3 auth/server/relay controllers with a Forge protocol adapter.
- The UI is named and branded **Forge**, not T3 Code. Retain required MIT attribution and license notices for copied/adapted T3 code.
- No one needs to install the T3 Code app, create a T3 account, run a T3 server, use a relay, or create a second provider session. The Forge iOS app is optional because the same pairing always has a browser fallback.
- The native app and browser are alternative phone surfaces for one pairing. The latest visible phone surface owns the connection; opening the app or returning to the browser transfers ownership. The terminal remains concurrently usable.

---

## 3. Important distinction: do not solve the wrong problem

### This is **not** stock T3 remote access

T3 Code supports Grok Build, but its Grok provider starts and owns a separate subprocess:

```text
T3 server → grok agent stdio
```

The stock T3 mobile app also expects a complete T3 server environment protocol (`/.well-known/t3/environment`, pairing/auth endpoints, and T3 WebSocket RPC). It cannot be pointed at Forge’s current ACP connection or a Forge `/rc` browser URL unchanged.

Do **not** replace this feature with “run T3 and use its iPhone app.” That produces a T3-owned session and does not attach the phone to an arbitrary existing Forge TUI session.

### Correct architecture

```text
Terminal Forge TUI ───────┐
                          │  same live app/session/ACP transport
Forge pager application ──┼── Forge Remote bridge ── Forge gateway ── Tailscale Serve ── Forge iOS app or browser
                          │
ACP agent process ────────┘
```

The gateway is only a phone-client transport for the native app or browser. It must never create a second ACP client, spawn a leader, launch `grok agent stdio`, or attach a different session behind the user’s back.

---

## 4. Scope

### In scope

1. Session-pinned phone remote for the active Forge TUI session.
2. Private tailnet-only URL/QR pairing using Tailscale Serve.
3. Shared terminal + phone state and controls.
4. Snapshot + delta remote protocol.
5. Source-mapped adaptation of the needed T3 **web** UI plus direct reuse of the pinned T3 native mobile presentation source.
6. Chat transcript, streaming, tools, plans, approvals, questions, composer, cancel, model/reasoning, and BTW.
7. Reconnections, revocation, expiry, focused tests, E2E validation, README documentation, and safe binary installation only after the feature works.

### Explicitly out of scope

- Implementing the full T3 server environment protocol merely to make stock T3 mobile connect.
- Requiring the stock T3 iOS app or any T3-owned runtime/account.
- A public URL, Tailscale Funnel, cloud relay, tunnel service, or multi-user public sharing.
- Multiple simultaneous phone clients for one pairing (one native app or browser connection per independently paired session is adequate, provided each terminal remains concurrently usable).
- Replacing Forge’s TUI with T3.
- Rewriting unrelated Forge/upstream architecture.
- Git push, release publication, tag creation, destructive resets, or deleting unrelated uncommitted changes.

---

## 5. Security and network requirements

### Trust model

The pairing URL is a bearer secret, but it must also only be reachable by devices in the user’s Tailscale tailnet. It is not an Internet endpoint.

### Requirements

- Bind the HTTP gateway to loopback only: `127.0.0.1:<ephemeral-port>`.
- Use **Tailscale Serve**, not Funnel, to proxy the loopback gateway over tailnet-private HTTPS.
- Require both laptop and phone to be signed into the same tailnet.
- Require Tailscale CLI, running/signed-in state, MagicDNS, and Tailscale HTTPS to be available. Report clear actionable errors; do not install Tailscale or alter its global configuration.
- Generate a fresh cryptographically random **256-bit** pairing secret for every new arm.
- Keep the secret out of logs and out of ACP payloads. It may appear only in the QR/link and active in-memory pairing state.
- URL path should contain the random secret, for example:

  ```text
  https://mac.tailnet.ts.net/forge/<64-hex-character-secret>
  ```

- Pairing lifetime is eight hours unless stopped sooner.
- On expiry and `/rc stop`, shut down the gateway and remove **only Forge’s exact random Serve path**.
- Never run destructive shared-state Tailscale commands such as `tailscale serve reset` or `tailscale serve clear`.
- Never invoke `tailscale funnel`.
- Use constant-time comparison for the path secret.
- Return `404` for missing/wrong/expired secrets.
- Use `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, and a CSP appropriate to the embedded static app.
- Define strict WebSocket message size limits and schema validation.
- One phone surface owns a pairing at a time. A newly active app or browser supersedes the former owner without presenting a conflict as an error. The terminal is never counted as a phone client and must always remain usable.

### Tailscale lifecycle commands

The existing intended scoped form is acceptable in principle:

```sh
tailscale serve --bg --https=443 --set-path /forge/<token> http://127.0.0.1:<port>
tailscale serve --https=443 --set-path /forge/<token> off
```

Validate behavior against installed Tailscale before calling it “E2E tested.” Do not overwrite unrelated routes.

---

## 6. Slash-command UX

Keep `/rc` session-scoped and retain `/remote-control` as an alias.

| Command | Required result |
|---|---|
| `/rc` | Verify prerequisites, arm a loopback gateway **for the active session**, create Forge’s one scoped tailnet-private Serve path, and show the private URL plus compact terminal QR. |
| `/rc enable` | Compatibility alias for `/rc`; it performs the same idempotent activation. |
| `/rc status` | Show whether the current active session is armed/live/expired, its expiry, and live URL only when appropriate. |
| `/rc stop` | Revoke the current exact session’s pairing, close its phone-client connection, stop its gateway, and remove only its exact Forge route. Other paired sessions remain live. Must be idempotent. |

If no live Forge session is selected, produce an explicit user error. Do not silently attach to another session.

### Expected messages

Messages should be short but must accurately state:

- exactly which session is armed (not the raw secret); 
- that the route is tailnet-private, not public;
- that no Funnel was enabled;
- when the route expires;
- that terminal and phone can be used simultaneously;
- what prerequisite is missing when Tailscale is unavailable.

---

## 7. Current local implementation status

The original `/rc` prototype has been replaced by the architecture in this handoff. The work remains uncommitted in a dirty checkout, so unrelated user-owned modifications must still be preserved.

| Path | Implemented role |
|---|---|
| `crates/codegen/xai-grok-pager/src/forge/remote_control.rs` | Session-scoped `/rc`, `/rc status`, `/rc stop`, and compatibility alias parsing. Bare `/rc` performs activation and route enablement. |
| `crates/codegen/xai-grok-pager/src/forge/remote_bridge.rs` | Exact-target multi-pair registry, bounded wakeable commands, canonical reducers, authoritative projection, revisions, interaction resolution, and lifecycle isolation. |
| `crates/codegen/xai-grok-shell/src/remote_control.rs` | Per-pairing loopback gateway, typed WebSocket protocol, authenticated embedded assets, compact QR, scoped Tailscale Serve lifecycle, expiry/revocation, and one phone-client connection per pairing. |
| `crates/codegen/xai-grok-shell/remote-ui` | Forge/Basier browser fallback with pinned T3 provenance, a multi-pair session list, native-shaped thread view, revision recovery, and the shared-control composer. |
| `clients/forge-mobile` | Forge iOS source workspace forked from the pinned T3 mobile presentation, with Forge protocol/pairing controllers replacing T3 auth/server/relay runtime paths. |
| `crates/codegen/xai-grok-pager/src/app/event_loop.rs` and dispatch/effect modules | Direct event-loop wakeup plus canonical prompt, cancel, model/reasoning, BTW, and interaction paths pinned to the armed target. |

Focused gateway, real loopback WebSocket, bridge, browser, and native protocol/UI tests cover the defects recorded in the original audit. Remaining release evidence is the physical iPhone/Tailscale manual matrix in section 12; automated checks do not substitute for that device test.

---

## 8. Forge Remote protocol

Implement a small **Forge-owned** WebSocket protocol. Do not attempt to tunnel raw ACP blindly to the browser and do not make the UI depend on T3 server RPC.

The schema may vary, but it must have the following semantic contract.

### 8.1 Connection and versioning

- The browser connects at the secret-protected endpoint, e.g.:

  ```text
  GET /forge/<token>/events
  ```

- The server sends a `connected` message and then a complete `snapshot` before normal deltas.
- Include a protocol version. If browser/server versions are incompatible, show a safe “update/reopen remote” message rather than processing ambiguous commands.
- A reconnect receives a fresh snapshot and should not need a stale local replay buffer to reconstruct state.
- Sequence every snapshot/delta or provide a monotonic revision. Client must ignore stale/out-of-order messages and detect a gap that requires resync.

### 8.2 Proposed message model

Names are illustrative; use Rust types and frontend validation rather than untyped strings.

#### Server → browser

```ts
type ServerMessage =
  | { type: "connected"; protocolVersion: 1; sessionId: string; expiresAt: string }
  | { type: "snapshot"; protocolVersion: 1; revision: number; session: RemoteSessionSnapshot }
  | { type: "delta"; protocolVersion: 1; revision: number; event: RemoteSessionEvent }
  | { type: "command_result"; commandId: string; ok: true }
  | { type: "command_result"; commandId: string; ok: false; error: RemoteError }
  | { type: "resync_required"; reason: string }
  | { type: "revoked"; reason: "stopped" | "expired" | "session_closed" }
  | { type: "error"; error: RemoteError };
```

#### Browser → server

```ts
type ClientMessage =
  | { type: "hello"; protocolVersion: 1 }
  | { type: "prompt"; commandId: string; text: string; attachments?: [] }
  | { type: "cancel"; commandId: string }
  | { type: "set_model"; commandId: string; modelId: string; reasoningEffort?: string | null }
  | { type: "btw"; commandId: string; question: string }
  | { type: "resolve_interaction"; commandId: string; interactionId: string; response: InteractionResponse }
  | { type: "resync" }
  | { type: "ping" };
```

### 8.3 Snapshot requirements

The snapshot must describe enough state for the real T3-derived UI to render the session without guessing:

```ts
interface RemoteSessionSnapshot {
  sessionId: string;
  title?: string;
  cwd?: string; // only if disclosure is acceptable; otherwise omit/redact
  status: "idle" | "running" | "waiting_for_input" | "error" | "closed";
  transcript: RemoteTimelineItem[];
  currentModel?: { id: string; label: string };
  availableModels: RemoteModel[];
  reasoningEffort?: { current?: string; options: RemoteReasoningOption[] };
  planMode?: { active: boolean; plan?: RemotePlan };
  activeInteractions: RemoteInteraction[];
  queue?: RemoteQueueItem[];
  taskState?: RemoteTaskState;
  capabilities: RemoteCapabilities;
}
```

The exact fields can align with existing Forge state, but do not omit data needed to render an accurate initial phone view.

### 8.4 Delta requirements

Deltas must cover at least:

- user message appended/updated;
- assistant streaming chunks and finalization;
- reasoning/thought display only to the extent Forge TUI displays it and policy allows;
- tool call started/updated/completed/failed;
- plan created/updated;
- model and reasoning selection changed;
- current run busy/idle/cancelled/failed;
- BTW question and response flow;
- pending interaction opened, updated, resolved/cancelled/timed-out;
- queue/background task updates if presented in remote UI;
- system/error notices;
- session closed/revoked.

Do not merely serialize arbitrary `AcpClientMessage` structs to JSON and expect the frontend to infer business state. Convert at a Forge app boundary into a stable view model.

### 8.5 Command semantics

- Validate command schema, command ID, fields, payload size, and session binding before dispatch.
- Command target is implicitly the gateway’s armed session. Never trust a browser-supplied session ID.
- Execute through the active pager’s actual session and app action/effect mechanisms.
- Acknowledge or return a typed error for each command.
- Browser optimistic rendering is allowed only if reconciled by authoritative deltas/snapshot.
- Enforce one active interaction response. If terminal resolves it first, phone gets resolution delta; a later phone response returns a harmless “already resolved” error.
- If phone resolves first, terminal UI must update immediately and its native interaction card must no longer offer stale controls.

---

## 9. Shared control and interaction semantics

This section is critical. Do not implement a phone-exclusive lease.

### 9.1 Prompts

- Terminal prompt and remote prompt are both valid while the remote is live.
- They follow the existing Forge queue/turn semantics. If Forge supports queuing while busy, remote prompts must use the same behavior; if not, return the same kind of user-visible error/disabled state as terminal.
- The originating side may optimistically show a user message, but the canonical transcript delta confirms it for both sides.

### 9.2 Cancel

- A cancel on either device cancels the same active turn and updates both surfaces.
- Preserve existing cancel metadata/semantics such as subagent cancellation and rewind behavior where that is part of normal TUI behavior.

### 9.3 Model and reasoning effort

- Phone model picker and reasoning selector must be derived from the session’s actual available models/capabilities.
- Terminal model change must update phone. Phone model change must update terminal’s picker/status.
- Do not hard-code model lists or effort levels in the web client.

### 9.4 BTW

- Support Forge’s existing `x.ai/btw` behavior from the phone.
- Display side question/request and response using the session’s existing semantics. The terminal and phone should both reflect it.

### 9.5 Approvals, questions, and plans

All interaction cards must appear on both devices.

#### Permissions

- Render the actual permission request information and available options.
- Phone option selection must resolve the exact original request using its pager-owned response channel / equivalent coordination layer.
- Terminal selection resolves phone UI, and vice versa.

#### `x.ai/ask_user_question`

- Render questions and offered options faithfully; support the expected answer shape, cancellation, and any free-form input required by Forge’s existing tool contract.
- Both surfaces must converge when one answers.

#### `x.ai/exit_plan_mode`

- Render the real plan and approve/request-changes feedback flow supported by Forge.
- Phone response must reach the live pending reverse RPC rather than being merely decorative.

#### Other ACP interaction methods

- Preserve safe handling for known/current methods and show a safe unsupported card for methods that cannot yet be rendered.
- Never drop a reverse request’s response sender in a way that hangs or fails a turn. Existing headless code has explicit safeguards for this class of bug.

### 9.6 Disconnect/reconnect

- A phone disconnect never auto-cancels the current turn, stops Forge, resolves an interaction, or causes a lost terminal state.
- Reconnect gets a snapshot that includes currently pending interactions and in-flight state.
- If an interaction is resolved while disconnected, snapshot reflects its final state.

---

## 10. UI requirement: adapt actual T3 Code web UI and reuse its native mobile source

### Source and license

T3 Code source inspected for this request:

```text
Repository: https://github.com/pingdotgg/t3code.git
Revision: b73232bdd31e83914a8a943960c7dc4b6390b39b
License: MIT License, Copyright (c) 2026 T3 Tools Inc.
```

The MIT license permits copying, modifying, publishing, distributing, sublicensing, and selling, provided the copyright and permission notice are included in copies/substantial portions.

The native implementation is isolated under `clients/forge-mobile` and is forked from `apps/mobile` at that same revision. Its active Forge entry keeps the upstream `HomeScreen`, `ThreadDetailScreen`, `ThreadFeed`, `ThreadComposer`, `PendingApprovalCard`, `PendingUserInputCard`, native Markdown/editor modules, navigation, gestures, haptics, safe areas, and keyboard behavior. A Forge-owned controller maps the remote snapshot/delta protocol into those views. T3 Connect, Clerk, relay, T3 server discovery, T3-owned update endpoints, and T3 bundle/deep-link identifiers must not be reachable from the active Forge entry.

The native identity is Forge: app name and wordmark `Forge`, URL scheme `forge`, development bundle identifier `com.exaforge.forge.dev`, and Basier Square typography sourced from the Exaforge font assets. The browser uses the same Forge branding and Basier family. T3 copyright and applicable third-party license notices remain in both distributions, while T3 trademarks, app identifiers, team IDs, legal URLs, and service configuration do not.

### Required approach

- For the browser artifact, copy/adapt the relevant **web** source and port the mobile information hierarchy without pretending React Native can be served by the Rust gateway. For the actual iOS artifact, retain the native Expo/React Native presentation source and replace only its controller/runtime boundary.
- Preserve the applicable MIT copyright/license text in the vendored source and in a third-party notices file included with the generated static assets/distribution.
- Document the upstream repository and pinned revision in a small `UPSTREAM.md` / provenance file colocated with vendored code.
- Keep this adaptation isolated so future upstream Forge syncs do not become difficult:
  - no broad formatting of upstream code;
  - no unrelated changes to shared Forge internals;
  - a clear vendor/adaptor boundary;
  - minimal, intentional integration points;
  - update instructions for future T3 source refreshes.

### Recommended source starting points

The exact dependency graph needs pruning, but start by evaluating and extracting the actual conversation surface from:

| T3 source path | Why it matters |
|---|---|
| `apps/web/src/components/ChatView.tsx` | Main chat/session composition and conversation orchestration. |
| `apps/web/src/components/chat/ChatComposer.tsx` | Actual T3 composer UI and controls. |
| `apps/web/src/components/chat/MessagesTimeline.tsx` | Actual conversation timeline/message rendering. |
| `apps/web/src/components/ComposerPromptEditor.tsx` | Composer editing behavior; may need a simplified extraction for phone. |
| related `apps/web/src/components/chat/*` | Tool cards, plan/interaction presentation, scrolling, composer banners. |
| T3 web styles/tokens/base UI primitives referenced by the extracted components | Visual fidelity and real behavior. |

The full T3 web app is large and assumes T3’s own router, Effect state runtime, environment model, auth, projects, terminals, files, PRs, and server RPC. Do **not** embed the entire T3 app unchanged. Extract the smallest coherent set of real components/styles necessary for the mobile remote conversation surface and replace its runtime dependencies with a Forge adapter.

### UI acceptance criteria

The remote page must visibly and structurally be the T3 web conversation experience adapted to Forge:

- actual T3-derived layout, message rendering, composer, typography, spacing, interactions, controls, icons, and styling—not a generic chat screen recreated from memory;
- Forge name, colors/icon, page title, and private-remote indicators;
- no visible T3 account, project picker, environment, relay, or unrelated desktop/server UI;
- mobile Safari first, responsive tablet/desktop usable;
- Markdown/code/tool/plan cards must render correctly and safely;
- accessible labels, focus management, keyboard behavior, reduced-motion consideration, safe-area support, and readable contrast;
- no external CDN or runtime network dependency for the UI;
- static assets served by Forge with cache policy compatible with secret path/revocation.
- a single composer action occupies one fixed position: Send while idle, Stop in the same position while cancellable, and Send again for a valid `/btw <question>` command;
- no separate Prompt/BTW selector and no second Stop control;
- typing bare `/btw` is invalid, and matching follows the TUI's case-sensitive slash-command boundary;
- opening a pairing URL first attempts `forge://pair?url=<encoded pairing URL>` without starting a competing browser WebSocket, then offers an explicit browser fallback;
- both phone surfaces provide a T3-shaped list of independently scanned pairings and switch by exact immutable capability rather than a client-supplied session ID.

### Build/distribution approach

Preferred approach:

1. Add an isolated frontend workspace/directory, for example `crates/codegen/xai-grok-shell/remote-ui/` or a top-level `web/forge-remote/` with clear ownership.
2. Vendor only the T3 web source/assets needed for remote conversation view plus a thin Forge adapter.
3. Build with a pinned, reproducible Node toolchain already available in the developer environment; do not auto-install packages/toolchains. If missing, fail with explicit instructions.
4. Produce static JS/CSS/assets in a generated directory excluded from source hand edits or embedded at Rust build time with `include_dir`/equivalent.
5. Serve index, assets, manifest, and WebSocket under the secret path. Ensure relative asset paths work under `/forge/<token>/`.
6. Define CSP based on generated assets. Prefer external hashed static JS/CSS and avoid `unsafe-inline` if practical.

The browser may use a constrained source-mapped extraction that omits T3-only features such as project navigation, local files, and terminals. It is not acceptable to leave the old handwritten static page as the final client. The native artifact has a stronger source-reuse requirement: preserve the real upstream presentation files wherever the Forge protocol can drive them through narrow props/adapters.

---

## 11. Proposed implementation plan

### Phase A — establish correct Forge-side session bridge first

1. **Trace all relevant pager state and input pathways.**
   - Identify canonical session snapshot sources: transcript/scrollback, task state, models, reasoning selection, plans, interactions, queue, app state.
   - Identify the normal TUI dispatch/effect paths for prompt, cancel, set model, reasoning effort, BTW, and interaction response.
   - Identify the existing response-channel ownership for `RequestPermission`, `ExtMethod`, `ask_user_question`, and `exit_plan_mode`.

2. **Replace global prototype bridge design as needed.**
   - Use a lifecycle-owned registry keyed by an opaque bridge incarnation, with a separate exact target, transport, snapshot revision, interaction map, gateway, token, listener, and Serve path per pairing. Canonical session ID alone must never select, replace, or revoke an entry.
   - Ensure re-arming the same exact target is idempotent; arming a different active session creates an independent pairing and never stops or replaces existing routes.

3. **Create typed remote state.**
   - Define protocol/view-model Rust types separate from raw ACP message types.
   - Build initial snapshot synchronously/atomically enough that the client does not see a broken empty state between connection and deltas.
   - Publish sequenced deltas after snapshot.

4. **Integrate remote receiver into event loop.**
   - Remote command arrival must wake an otherwise idle TUI.
   - Process commands serially through app actions/effects; do not create competing receivers for ACP.
   - Keep laptop terminal input and ACP responsiveness intact.

5. **Implement session filtering.**
   - Every snapshot and delta is for the armed session only.
   - Do not assume all `AcpClientMessage` variants have the same session field; extract/validate safely and write tests for each relevant event type.

6. **Implement interaction coordinator.**
   - Introduce per-session, opaque interaction IDs and a pending map owned by the app/bridge.
   - Capture enough typed request data to render and validate a phone response.
   - Ensure a terminal resolution and phone resolution race has deterministic one-winner behavior.
   - Send a resolution delta to both UIs and return a safe “already resolved” result to the loser.

### Phase B — replace temporary client with T3-derived UI

7. **Vendor/adapt the real T3 web conversation UI.**
   - Start with timeline + composer + model controls + tool/plan/interactions.
   - Build Forge-specific state adapter/hooks that consume the Forge Remote snapshot/delta protocol and emit commands.
   - Preserve T3 source provenance/license.

8. **Remove the temporary giant inline page.**
   - Delete or retire `forge_remote_html()` only when the static asset client is complete and tested.
   - The Rust gateway should serve the frontend index/assets instead.

9. **Make reconnect and rendering authoritative.**
   - Snapshot drives initial and reconnect rendering.
   - Deltas update local state deterministically.
   - UI should gracefully show gateway expiry, session closed, connection errors, server/client version mismatch, resync state, and unavailable capabilities.

### Phase C — validation and docs

10. **Testing.** See the exhaustive test matrix below.
11. **Update README.** Only claim what is actually implemented and validated. Mention the tailnet/private behavior, Tailscale requirements, `/rc` flow, shared control, and how to stop access.
12. **Build and install only after all feature blockers are resolved.** Build release; atomically replace local `~/.grok/bin/grok` only after explicit authorization already given for this task context. Do not kill a currently running Forge process; it keeps using its old image until the user restarts it.

---

## 12. Acceptance test matrix

### Unit tests

#### Security / gateway

- token is random, 256-bit, fixed encoding length, and not derived from session ID;
- constant-time token comparison;
- wrong/missing/expired secret returns 404;
- secret is not leaked by status/error/log strings;
- HTML/index/assets and WebSocket require the secret path;
- maximum one phone-client connection per pairing;
- command payload size limit;
- malformed, unknown, empty, overlong, and invalid-version messages are rejected cleanly;
- CSP/no-store/referrer/nosniff headers;
- Tailscale command argument generation includes only Forge’s exact path and never `funnel`, `reset`, or `clear`;
- stop/expiry/new-session-arm remove only the active Forge route;
- cancellation shuts down gateway reader/fanout tasks.

#### Bridge/protocol

- phone command wakes idle event loop promptly;
- remote request is delivered only to armed session;
- commands cannot carry/override session target;
- snapshot contains preexisting transcript/model/status/interactions;
- snapshot+delta revisions are monotonic;
- non-armed session ACP messages never appear in remote stream;
- direct/default mode works without leader socket;
- leader-backed mode also works without duplicating/stealing ACP receiver;
- remote and terminal prompt take the same canonical path/produce matching app state;
- model, reasoning, cancel, BTW actions use expected ACP metadata/effects;
- interaction resolution works from phone;
- interaction resolution works from terminal and produces remote resolved update;
- double resolution race resolves exactly once;
- reconnect after dropped client gets current snapshot;
- gateway expiry / stop yields clean revoked state.

#### UI

- frontend protocol reducer handles snapshot/delta/resync/revoked/error;
- copied/adapted T3 components render core states;
- model/reasoning choices come from server snapshot;
- markdown and code content are sanitized/rendered safely;
- permissions/questions/plans render actual options and cannot dispatch malformed reply;
- UI shows a clear distinction between reconnecting, revoked, and session closed;
- tests/provenance enforce MIT notice is present.

### Integration tests

Use a fake/in-memory ACP layer or existing test harness where possible to prove:

1. Start direct/default session; arm remote; connect browser test client; snapshot matches TUI state.
2. Send phone prompt; terminal-side app receives expected state + ACP request; simulated response streams to browser.
3. Send terminal prompt; browser receives user/assistant/tool deltas.
4. Phone cancel cancels terminal turn.
5. Phone model/reasoning update changes terminal state and ACP request.
6. Terminal model/reasoning update changes browser state.
7. Phone BTW works and updates both.
8. Permission/question/plan cards: terminal-first and phone-first response races.
9. Browser disconnect/reconnect during a streamed turn.
10. Arm a second session and ensure both exact routes remain live; stop either session and ensure only its route is removed.

### Manual E2E validation

Before saying the feature works:

1. Start a normal direct Forge session in terminal—not leader-only mode.
2. Have an existing multi-turn conversation with tool activity before invoking `/rc`.
3. Run `/rc`.
4. Confirm no unrelated Tailscale Serve route changes.
5. Scan QR from tailnet-connected iPhone Safari.
6. Confirm transcript and current model/status appear immediately.
7. Prompt from phone and watch terminal update/execute.
8. Prompt from terminal and watch phone update.
9. Test cancel, model, reasoning, BTW, permission, `ask_user_question`, and plan approval/change response from both sides.
10. Reload/reconnect browser during an active turn; confirm accurate state recovers.
11. `/rc stop`; confirm URL fails and only Forge’s path disappeared.
12. Repeat with a resumed session and, if supported by test setup, leader-backed session.

Capture concise evidence (commands, results, screenshots/logs without pairing secret) in the implementation notes/PR description.

---

## 13. Build and environment constraints

- Before using Node/Vite/pnpm/bun/etc., check whether the necessary tool is already available. Do **not** auto-install packages, system dependencies, or change shell configuration.
- If required tooling is missing, report exactly what is missing and the command the developer should run; wait rather than auto-installing.
- The Rust workspace currently builds with Cargo. Run focused tests first, then relevant frontend tests/build, then workspace/package checks appropriate to changed crates.
- Existing focused command that passed before the unfinished runtime gap was found:

  ```sh
  cargo test -p xai-grok-shell remote_control --lib && \
  cargo test -p xai-grok-pager forge::remote --lib && \
  cargo build --release -p xai-grok-pager-bin
  ```

  This is not sufficient final validation; add real bridge/protocol/UI coverage.

- Do not claim an installed binary was replaced unless it was actually atomically replaced and verified.
- Do not kill an active user Forge process when installing a new binary.

---

## 14. Documentation requirements after implementation

Update `README.md` only once behavior is real. It should concisely explain:

1. Tailscale requirements on laptop/phone (same tailnet; MagicDNS/HTTPS).
2. `/rc` → scan QR/open URL → `/rc stop`.
3. The URL is private to the tailnet, expires, and never uses Funnel/public exposure.
4. Phone and terminal are simultaneous shared controls for the exact live session.
5. Both phone clients are Forge-branded: the browser is a source-mapped T3 web/native-geometry port, while the iOS app directly reuses the pinned T3 React Native presentation source. Neither requires T3 Code installation or a T3 account.
6. What the feature supports: transcript, prompts, tools, cancel, model/reasoning, BTW, approvals, questions, plans.
7. Clear troubleshooting for missing Tailscale/MagicDNS/HTTPS.

Do not overpromise unsupported behavior, and do not say “PWA” unless installability/assets/manifest are actually complete.

---

## 15. Upstream-friendliness requirements

Forge syncs upstream Grok Build periodically. Keep the feature maintainable:

- confine new code to clearly named Forge modules where possible (`forge/remote_*`, dedicated remote UI directory);
- keep invasive event-loop changes minimal and well-commented;
- avoid renaming/reformatting upstream areas unrelated to the feature;
- prefer adapters at boundaries over modifying generic ACP internals;
- write tests describing the exact-session invariant;
- isolate T3-derived code and document provenance/pinned revision;
- include a short update note explaining how to refresh vendored T3 components deliberately rather than accidentally merging the full T3 app.

---

## 16. Definition of done

These were the original acceptance criteria. The unchecked boxes do not claim current completion or physical-device validation.

- [ ] A normal existing Forge TUI session can run `/rc` without leader mode or special flags.
- [ ] Phone Safari opens a private, tailnet-only Forge URL and immediately displays the existing session state/history.
- [ ] The browser UI has source-level T3 provenance and a reproducible pinned-source verifier; the native Forge app directly reuses the pinned T3 mobile presentation source with MIT and third-party notices.
- [ ] When the native app is installed, the pairing page hands off before opening its browser socket; otherwise the browser remains a complete fallback.
- [ ] Independently paired sessions coexist and can be selected from the phone session list; stopping one does not revoke another, even when canonical session IDs match.
- [ ] Terminal and phone can be used simultaneously against the same exact live session.
- [ ] Both sides synchronize prompts, assistant output, tools, status, cancel, model/reasoning, BTW, permissions, questions, and plan interactions.
- [ ] Interactions resolve exactly once and update both surfaces.
- [ ] Reconnect/reload recovers accurate state by snapshot.
- [ ] `/rc stop` and expiry revoke access and remove only Forge’s own Tailscale Serve path.
- [ ] No Funnel/public exposure, no destructive Serve reset/clear, no stock T3 app/server requirement.
- [ ] Security, protocol, bridge, UI, integration, and manual E2E tests pass.
- [ ] README is accurate and concise.
- [ ] Release build succeeds; local binary replacement happens only after final validation and without disrupting an existing terminal process.

---

## 17. Explicit anti-goals / common failure modes

Reject an implementation if it does any of the following:

- Uses the existing T3 iOS app unchanged and asks the user to create a T3 environment/thread.
- Launches a second `grok agent stdio` process or a new T3-owned session.
- Requires leader mode or attaches only through `~/.grok/leader.sock`.
- Shows only future events and no existing transcript.
- Makes laptop input stop working while phone is connected.
- Makes phone input wait indefinitely when TUI is idle.
- Sends raw ACP blobs to the UI with no stable snapshot/view model.
- Lets browser choose a session ID or receive data from another session.
- Leaves `Respond` as a no-op or fails to resolve actual pending ACP requests.
- Keeps the large handwritten inline HTML/CSS/JS page as the shipped UI.
- Uses Tailscale Funnel, `serve reset`, `serve clear`, public tunnel services, or changes unrelated Serve routes.
- Claims success based solely on Rust compilation/unit tests without a live direct-session browser lifecycle test.

---

## 18. Deliverable

Deliver implementation changes, tests, and concise docs. In the final report, state:

- the exact T3 source revision and components adapted;
- protocol and bridge design;
- what shared-control behaviors were tested;
- exact Rust/frontend build and test commands/results;
- manual E2E result for direct/default session and Tailscale lifecycle;
- files changed;
- whether the binary was installed/replaced (only if it actually was);
- that nothing was pushed/published unless separately explicitly authorized.

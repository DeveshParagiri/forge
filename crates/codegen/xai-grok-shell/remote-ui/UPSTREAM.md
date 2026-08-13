# T3 Code web UI provenance

This browser client is a constrained adaptation of the T3 Code web conversation UI and a faithful web port of the T3 Code mobile home/thread presentation. The browser home is newly written React DOM code; it does not claim native source reuse or pixel identity across UIKit and the web platform.

The upstream repository is `https://github.com/pingdotgg/t3code.git`. The exact revision used is `b73232bdd31e83914a8a943960c7dc4b6390b39b`, committed as `feat(web): reset sidebar width on double click (#6320)`. The upstream code is licensed under the MIT License, Copyright (c) 2026 T3 Tools Inc. The full license is preserved in `LICENSE.t3code.txt` and the generated distribution in `public/THIRD_PARTY_NOTICES.txt`.

The full-file SHA-256 values of every inspected upstream source file are recorded in `provenance/upstream-files.sha256`. Given a checkout of T3 Code at the pinned revision, `pnpm verify:upstream -- /path/to/t3code` mechanically verifies the revision and each source file before a review or refresh.

The adaptation deliberately does not import T3's router, Effect state runtime, environment model, authentication, projects, terminals, files, pull requests, or server RPC. Forge's versioned snapshot and delta protocol replaces those runtime dependencies.

## Adapted source

`src/t3-adapted/ChatView.tsx` adapts the conversation composition, constrained `max-w-3xl` content column, timeline/composer ownership, and live-edge scrolling behavior from `apps/web/src/components/ChatView.tsx` and its use of `MessagesTimeline` and `ChatComposer`.

`src/t3-adapted/MessagesTimeline.tsx` adapts the upstream timeline root, `mx-auto w-full max-w-3xl` geometry, right-aligned `max-w-[80%]` rounded user-message surface, assistant content treatment, working indicator, compact collapsible work entries, and plan/card hierarchy from `apps/web/src/components/chat/MessagesTimeline.tsx`, especially the upstream sections defining `MessagesTimeline`, `UserMessageRow`, `AssistantMessageRow`, `AssistantWorkingIndicator`, `WorkLogGroup`, `WorkLogEntryRow`, and plan rendering.

`src/t3-adapted/ChatComposer.tsx` adapts the upstream bottom-docked form, safe-area horizontal inset, 22px composer shell, prompt/editor/control hierarchy, compact primary action behavior, and server-derived model controls from `apps/web/src/components/chat/ChatComposer.tsx` and its composer shell styles in `apps/web/src/index.css`.

`src/t3-adapted/UsageSheet.tsx` adapts the segmented Context/Session/Account hierarchy, usage headline cards, provider window rows, refresh treatment, and partial-data language from `apps/mobile/src/features/usage/UsageRouteScreen.tsx`. Forge does not import T3's environment discovery or account RPC. The sheet renders only the selected Forge session's optional versioned `usage` snapshot and schedules refresh through the scoped remote command channel.

`src/t3-adapted/ComposerPromptEditor.tsx` adapts the upstream editor contract and focus/submit behavior from `apps/web/src/components/ComposerPromptEditor.tsx`. The upstream Lexical editor was intentionally reduced to a controlled textarea because Forge Remote phase one supports plain text only and omits mentions, skills, attachments, terminal context, and the slash palette. The component retains multiline editing, mobile-safe explicit submission, desktop Enter-to-send behavior, focus restoration, and automatic height management.

`src/styles.css` adapts the upstream font stacks, dark neutral surface hierarchy, semantic message/tool/code tokens, timeline spacing, 22px composer radius, compact control sizing, focus rings, safe-area behavior, and reduced-motion treatment. Its colors are remapped to Forge's canonical palette in `crates/codegen/xai-grok-pager-render/src/forge/forge_theme.rs` rather than T3's stock primary palette.

`src/t3-adapted/PairingsHome.tsx` is newly written React DOM code that ports the information hierarchy and geometry of `apps/mobile/src/features/home/HomeHeader.tsx`, `HomeScreen.tsx`, and `apps/mobile/src/features/threads/thread-list-v2-items.tsx`: compact brand header, flat edge-to-edge session rows, colored attention labels, title/project/status hierarchy, inset hairlines, and the floating rounded search dock. It stores only exact scanned pairing capabilities and sanitized last-known presentation metadata; it is not a T3 runtime or catalog implementation.

The mobile-like thread header and responsive CSS map the native back/title symmetry and safe-area ownership from `apps/mobile/src/features/threads/ThreadDetailScreen.tsx`. The existing transcript and composer port additionally use `ThreadFeed.tsx` and `ThreadComposer.tsx` as visual references for phone-scale feed spacing and the bottom glass composer. T3's native keyboard controller, navigation stack, Expo APIs, haptics, file surfaces, git controls, and environment RPC are not copied into the browser.

Pairing registration and URL parsing boundaries are informed by `apps/mobile/src/features/connection/pairing.ts`, but Forge keeps each scanned bearer URL immutable and uses its own existing per-session snapshot/delta WebSocket protocol. No arbitrary session target or cross-gateway catalog endpoint is introduced.

## Forge typography provenance

The browser uses Basier Square Regular and SemiBold imported from
`exaforge/website` at commit
`38706ebf060ad4379ff9365c7aaa4276fb866322`. Their SHA-256 values at import are
`03ba9f4f16e19439a8fcfd45baaa82c5c119ff0610931cba166d3f5a8fea2706`
and `e8822db64f02e851302a7539c21e15eb48673795d6d754bd21e8540af31252e2`.
These are separately supplied Forge/Exaforge brand assets; the T3 MIT license
does not cover them and this repository does not invent or assert new rights.

## Updating

To refresh the source, inspect every upstream file recorded in `provenance/upstream-files.sha256` at the new pinned revision, compare only the cited conversation and mobile-presentation sections, port intentional changes into `src/t3-adapted`, update this document and both license notices if necessary, then run `pnpm check`, `pnpm test`, and `pnpm build`. Do not replace this isolated client with the whole T3 runtime.

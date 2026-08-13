# Upstream and rebrand boundary

Forge Mobile is derived from `pingdotgg/t3code`, commit `b73232bdd31e83914a8a943960c7dc4b6390b39b`. The upstream project is MIT licensed; its full license is retained at `third_party/T3CODE-LICENSE.txt`. This repository intentionally preserves upstream internal `@t3tools/*` package names and many source filenames because those names are implementation provenance, not shipped Forge product branding.

The copied workspace consists of upstream `apps/mobile`, `packages/client-runtime`, `packages/contracts`, `packages/shared`, the native Ghostty terminal source needed by upstream native modules, the workspace patches, and representative upstream brand assets kept only under `third_party/t3-assets`. The active Forge app starts at `apps/mobile/index.ts`, imports `apps/mobile/src/forge/ForgeApp.tsx`, and renders the retained upstream `HomeScreen`, `ThreadDetailScreen`, `ThreadFeed`, `ThreadComposer`, `PendingApprovalCard`, and `PendingUserInputCard` presentation boundaries. The manifest in `provenance/t3-mobile-files.json` records every copied file's pinned hash and whether it remains identical or was adapted.

The upstream-only `apps/mobile/clerk-theme.json` is deliberately excluded because Forge Mobile has no Clerk runtime. The exclusion and its rationale are recorded mechanically in the provenance manifest. The retained `eas.json` contains only a Forge local-simulator development profile; it has no upstream App Store identifier, submit target, update channel, or production profile.

Forge-owned code supplies the Stack, secure multi-pair registry, QR and `forge://pair` handoff, Forge protocol decoder/socket/controller, snapshot-to-T3 presentation adapter, and capability-aware command mapping. Small upstream presentation edits hide or disable unsupported project, file, git, terminal, runtime-mode, attachment, search, and lifecycle actions. Those controls are not simulated.

No active Forge entry imports the upstream CloudAuth provider, Clerk SDK, relay runtime, Expo Updates, T3 URL schemes, T3 legal URLs, T3 Apple team configuration, share/widget extensions, or T3 product marks. `scripts/verify-active-graph.mjs` validates that executable Forge source boundary. It deliberately does not classify transitive type-only barrel exports as executable code; `tsc` separately validates types.

Forge branding assets and Basier Square source fonts were imported from
`exaforge/website` at commit
`38706ebf060ad4379ff9365c7aaa4276fb866322`. Native OTF/TTF derivatives were
generated with fontTools 4.59.1 and brotli 1.2.0 by loading each WOFF2,
clearing its WOFF2 flavor, and saving its native sfnt. Their source paths and
output hashes are recorded in `provenance/forge-brand-files.json`. The font
assets are separately supplied brand material and are not relicensed by the
upstream T3 MIT license.

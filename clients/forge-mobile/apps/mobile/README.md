# Forge Mobile app package

The workspace-level [`README.md`](../../README.md) contains the supported pairing flow, local iPhone installer, limitations, troubleshooting, and license references.

This package contains the Expo and React Native app. Its active entry is [`src/forge/ForgeApp.tsx`](src/forge/ForgeApp.tsx). Run package checks from the workspace root with `pnpm typecheck` and `pnpm test`. Regenerate the ignored native iOS project with `pnpm prebuild:ios`; keep persistent native changes in [`app.config.ts`](app.config.ts) or [`plugins`](plugins).

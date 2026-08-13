import { collectActiveGraph, relativeToRoot } from "./active-graph.mjs";

const graph = collectActiveGraph();
const failures = [];
const forbiddenPaths = [
  "apps/mobile/src/App.tsx",
  "/features/cloud/",
  "/features/sharing/",
  "/features/auth/",
  "/state/relay",
  "/client-runtime/src/relay/",
  "/CompactBrandTitle.tsx",
  "/BrandMark.tsx",
];
const forbiddenText = [
  [/@clerk\//i, "Clerk SDK"],
  [/ClerkProvider|EXPO_PUBLIC_CLERK/i, "Clerk runtime config"],
  [/expo-updates/i, "Expo Updates"],
  [/t3code(?:-dev|-preview)?:\/\//i, "T3 deep link"],
  [/https?:\/\/[^\s"']*t3(?:code)?\.(?:chat|app|dev)/i, "T3-owned URL"],
  [/\bT3 Code\b/, "T3 product mark"],
  [/DMSans-|@expo-google-fonts\/dm-sans/, "DM Sans runtime"],
];

for (const [file, entry] of graph) {
  const relative = relativeToRoot(file);
  if (forbiddenPaths.some((part) => relative === part || relative.includes(part))) {
    failures.push(`${relative}: forbidden runtime module`);
  }
  for (const [pattern, label] of forbiddenText) {
    if (pattern.test(entry.text)) failures.push(`${relative}: ${label}`);
  }
}

if (failures.length > 0) {
  process.stderr.write(`Forge active graph verification failed (${failures.length}):\n${failures.join("\n")}\n`);
  process.exit(1);
}

const files = [...graph.keys()].map(relativeToRoot).sort();
process.stdout.write(
  `PASS active Forge iOS graph: ${files.length} local runtime files; 0 CloudAuth, Clerk, relay, Expo Updates, T3 schemes/marks/URLs, or DM Sans runtime paths.\n`,
);

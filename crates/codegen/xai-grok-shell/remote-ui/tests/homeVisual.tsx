import { createRoot } from "react-dom/client";
import type { StoredPairing } from "../src/pairingRegistry";
import { PairingsHome } from "../src/t3-adapted/PairingsHome";
import "../src/styles.css";

const pairings: StoredPairing[] = [
  { id: "one", baseUrl: `https://forge.tail.example/forge/${"a".repeat(64)}/`, addedAt: new Date().toISOString(), lastSeenAt: new Date(Date.now() - 11 * 60_000).toISOString(), title: "Make remote coding feel local ✦", cwd: "/Users/dev/Projects/forge", status: "running", modelLabel: "GPT-5.6 Sol" },
  { id: "two", baseUrl: `https://forge.tail.example/forge/${"b".repeat(64)}/`, addedAt: new Date().toISOString(), lastSeenAt: new Date(Date.now() - 24 * 60_000).toISOString(), title: "Make Suspense transitions buttery", cwd: "/Users/dev/Projects/react", status: "running", modelLabel: "GPT-5.6 Terra" },
  { id: "three", baseUrl: `https://forge.tail.example/forge/${"c".repeat(64)}/`, addedAt: new Date().toISOString(), lastSeenAt: new Date(Date.now() - 31 * 60_000).toISOString(), title: "Put the command center in your pocket", cwd: "/Users/dev/Projects/forge", status: "waiting_for_input", attention: "approval", modelLabel: "GPT-5.6 Sol" },
  { id: "four", baseUrl: `https://forge.tail.example/forge/${"d".repeat(64)}/`, addedAt: new Date().toISOString(), lastSeenAt: new Date(Date.now() - 52 * 60_000).toISOString(), title: "Turn hydration warnings into haikus", cwd: "/Users/dev/Projects/react", status: "idle", modelLabel: "GPT-5.6 Terra" },
];

const root = document.getElementById("root");
if (!root) throw new Error("Visual fixture root was not found");
createRoot(root).render(
  <PairingsHome
    pairings={pairings}
    onSelect={() => {}}
    onRemove={() => {}}
    onCreateSession={() => {}}
    onCreateSessionInProject={() => {}}
  />,
);

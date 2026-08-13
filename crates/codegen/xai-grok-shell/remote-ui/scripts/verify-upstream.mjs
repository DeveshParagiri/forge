import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PINNED_REVISION = "b73232bdd31e83914a8a943960c7dc4b6390b39b";
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const uiDirectory = resolve(scriptDirectory, "..");
const upstreamArgument = process.argv.slice(2).find((argument) => argument !== "--");
const upstreamDirectory = upstreamArgument ? resolve(upstreamArgument) : null;

if (!upstreamDirectory) {
  console.error("Usage: node scripts/verify-upstream.mjs /path/to/t3code");
  process.exitCode = 2;
} else {
  const revision = execFileSync("git", ["-C", upstreamDirectory, "rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim();
  if (revision !== PINNED_REVISION) {
    throw new Error(`Expected T3 Code ${PINNED_REVISION}, received ${revision}`);
  }

  const manifest = await readFile(join(uiDirectory, "provenance/upstream-files.sha256"), "utf8");
  for (const line of manifest.trim().split("\n")) {
    const [expected, sourcePath] = line.trim().split(/\s+/, 2);
    const contents = await readFile(join(upstreamDirectory, sourcePath));
    const actual = createHash("sha256").update(contents).digest("hex");
    if (actual !== expected) throw new Error(`${sourcePath} does not match the pinned source`);
    console.log(`verified ${sourcePath}`);
  }
}

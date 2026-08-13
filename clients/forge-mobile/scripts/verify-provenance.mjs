import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sha = (file) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const read = (relative) => JSON.parse(fs.readFileSync(path.join(ROOT, relative), "utf8"));
const failures = [];
const manifest = read("provenance/t3-mobile-files.json");
const brand = read("provenance/forge-brand-files.json");

if (manifest.upstream.revision !== "b73232bdd31e83914a8a943960c7dc4b6390b39b") {
  failures.push("unexpected pinned T3 revision");
}
if (sha(path.join(ROOT, manifest.adaptationPatch)) !== manifest.adaptationPatchSha256) {
  failures.push("adaptation patch hash mismatch");
}
for (const entry of manifest.copiedFiles) {
  const file = path.join(ROOT, entry.currentPath);
  if (!fs.existsSync(file)) failures.push(`${entry.currentPath}: missing`);
  else if (sha(file) !== entry.currentSha256) failures.push(`${entry.currentPath}: current hash mismatch`);
  if (entry.status === "identical" && entry.currentSha256 !== entry.upstreamSha256) {
    failures.push(`${entry.currentPath}: identical status does not match upstream hash`);
  }
}
for (const entry of manifest.excludedUpstreamFiles ?? []) {
  if (fs.existsSync(path.join(ROOT, entry.upstreamPath))) {
    failures.push(`${entry.upstreamPath}: declared excluded but present`);
  }
}
for (const entry of brand.brandFiles) {
  const file = path.join(ROOT, entry.currentPath);
  if (!fs.existsSync(file)) failures.push(`${entry.currentPath}: missing brand asset`);
  else if (sha(file) !== entry.currentSha256) failures.push(`${entry.currentPath}: brand hash mismatch`);
}
if (sha(path.join(ROOT, "third_party/T3CODE-LICENSE.txt")) !== "935d8f2af0c703f9c39517ee57cc4930b19d02d533be930b63f0e82f93614b43") {
  failures.push("T3 MIT license hash mismatch");
}

if (failures.length > 0) {
  process.stderr.write(`FAIL provenance (${failures.length})\n${failures.join("\n")}\n`);
  process.exit(1);
}
process.stdout.write(`PASS provenance: T3 ${manifest.upstream.revision}; ${manifest.identicalFileCount} identical, ${manifest.adaptedFileCount} adapted, ${brand.brandFiles.length} Forge brand assets verified.\n`);

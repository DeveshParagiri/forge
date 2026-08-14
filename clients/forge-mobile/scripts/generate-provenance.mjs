import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const UPSTREAM = path.resolve(process.argv[2] ?? process.env.T3CODE_UPSTREAM_DIR ?? "");
const PINNED_REVISION = "b73232bdd31e83914a8a943960c7dc4b6390b39b";
const SAME_PATH_ROOTS = [
  "apps/mobile",
  "packages/client-runtime",
  "packages/contracts",
  "packages/shared",
  "native/libghostty-vt",
  "patches",
];
const ROOT_FILES = [
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "tsconfig.base.json",
  "vite.config.ts",
];
const ASSET_ROOTS = ["assets/dev", "assets/nightly", "assets/prod"];
const EXCLUDED_UPSTREAM_FILES = [
  {
    upstreamPath: "apps/mobile/clerk-theme.json",
    reason:
      "Unused Clerk presentation theme excluded from the Forge deliverable; the active app has no Clerk integration.",
  },
];
const excludedUpstreamPaths = new Set(EXCLUDED_UPSTREAM_FILES.map((entry) => entry.upstreamPath));

if (!UPSTREAM || !fs.existsSync(path.join(UPSTREAM, ".git"))) {
  throw new Error("Usage: node scripts/generate-provenance.mjs /path/to/clean/pinned/t3code");
}
const revision = execFileSync("git", ["-C", UPSTREAM, "rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
if (revision !== PINNED_REVISION)
  throw new Error(`Expected ${PINNED_REVISION}, received ${revision}`);
if (execFileSync("git", ["-C", UPSTREAM, "status", "--porcelain"], { encoding: "utf8" }).trim()) {
  throw new Error("The upstream checkout must be clean.");
}

function sha(file) {
  return createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function filesBelow(root) {
  if (!fs.existsSync(root)) return [];
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(root, entry.name);
    if (entry.isDirectory()) return filesBelow(file);
    return entry.isFile() ? [file] : [];
  });
}

const copiedMappings = [
  ...ROOT_FILES.map((relative) => ({ upstreamPath: relative, currentPath: relative })),
  ...SAME_PATH_ROOTS.flatMap((relativeRoot) =>
    filesBelow(path.join(UPSTREAM, relativeRoot)).map((file) => ({
      upstreamPath: path.relative(UPSTREAM, file).replaceAll(path.sep, "/"),
      currentPath: path.relative(UPSTREAM, file).replaceAll(path.sep, "/"),
    })),
  ),
  ...ASSET_ROOTS.flatMap((relativeRoot) =>
    filesBelow(path.join(UPSTREAM, relativeRoot)).map((file) => {
      const upstreamPath = path.relative(UPSTREAM, file).replaceAll(path.sep, "/");
      return {
        upstreamPath,
        currentPath: `third_party/t3-assets/${upstreamPath.slice("assets/".length)}`,
      };
    }),
  ),
]
  .filter((entry) => !excludedUpstreamPaths.has(entry.upstreamPath))
  .sort((left, right) => left.currentPath.localeCompare(right.currentPath));

const copiedFiles = copiedMappings.map(({ upstreamPath, currentPath }) => {
  const source = path.join(UPSTREAM, upstreamPath);
  const current = path.join(ROOT, currentPath);
  const upstreamSha256 = sha(source);
  const currentSha256 = fs.existsSync(current) ? sha(current) : null;
  return {
    upstreamPath,
    currentPath,
    upstreamSha256,
    currentSha256,
    status:
      currentSha256 === null
        ? "missing"
        : currentSha256 === upstreamSha256
          ? "identical"
          : "adapted",
  };
});

const adapted = copiedFiles.filter((entry) => entry.status === "adapted");
let adaptationPatch = `# T3 Code ${PINNED_REVISION} to Forge Mobile adaptation patch\n`;
for (const entry of adapted) {
  const result = spawnSync(
    "git",
    [
      "diff",
      "--no-index",
      "--no-ext-diff",
      "--no-color",
      "--unified=3",
      "--",
      path.join(UPSTREAM, entry.upstreamPath),
      path.join(ROOT, entry.currentPath),
    ],
    { encoding: "utf8", maxBuffer: 128 * 1024 * 1024 },
  );
  if (result.status !== 0 && result.status !== 1)
    throw new Error(result.stderr || `Could not diff ${entry.currentPath}`);
  adaptationPatch += result.stdout
    .replaceAll(path.join(UPSTREAM, entry.upstreamPath), `a/${entry.upstreamPath}`)
    .replaceAll(path.join(ROOT, entry.currentPath), `b/${entry.currentPath}`);
}

const provenanceDir = path.join(ROOT, "provenance");
fs.mkdirSync(provenanceDir, { recursive: true });
const patchPath = path.join(provenanceDir, "t3-mobile-adaptations.patch");
fs.writeFileSync(patchPath, adaptationPatch);

const manifest = {
  schemaVersion: 1,
  upstream: {
    repository: "https://github.com/pingdotgg/t3code.git",
    revision: PINNED_REVISION,
    license: "MIT",
  },
  generatedAt: new Date().toISOString(),
  copiedFileCount: copiedFiles.length,
  identicalFileCount: copiedFiles.filter((entry) => entry.status === "identical").length,
  adaptedFileCount: adapted.length,
  missingFileCount: copiedFiles.filter((entry) => entry.status === "missing").length,
  adaptationPatch: "provenance/t3-mobile-adaptations.patch",
  adaptationPatchSha256: sha(patchPath),
  excludedUpstreamFiles: EXCLUDED_UPSTREAM_FILES,
  copiedFiles,
};
fs.writeFileSync(
  path.join(provenanceDir, "t3-mobile-files.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);

const EXAFORGE_REPOSITORY = "https://github.com/exaforge/website.git";
const EXAFORGE_REVISION = "38706ebf060ad4379ff9365c7aaa4276fb866322";
const brandMappings = [
  ["src/assets/fonts/Basier Square Regular.woff2", "brand/Basier Square Regular.woff2", "source"],
  ["src/assets/fonts/Basier Square SemiBold.woff2", "brand/Basier Square SemiBold.woff2", "source"],
  [
    "src/assets/fonts/basiersquaremono-regular-webfont.woff2",
    "brand/basiersquaremono-regular-webfont.woff2",
    "source",
  ],
  [null, "brand/mobile-logo-source.png", "exact-copy-of-user-supplied-Untitled-design-1.png"],
  [null, "brand/apple-icon.png", "180x180-derived-from-brand/mobile-logo-source.png"],
  ["public/icon.svg", "brand/icon.svg", "source"],
  [null, "brand/logo.png", "272x244-transparent-derived-from-brand/mobile-logo-source.png"],
  [null, "brand/native/BasierSquare-Regular.otf", "fontTools-4.59.1/brotli-1.2.0-derived"],
  [null, "brand/native/BasierSquare-Semibold.otf", "fontTools-4.59.1/brotli-1.2.0-derived"],
  [null, "brand/native/BasierSquareMono-Regular.ttf", "fontTools-4.59.1/brotli-1.2.0-derived"],
  [
    null,
    "apps/mobile/assets/forge/icon.png",
    "1024x1024-derived-from-brand/mobile-logo-source.png",
  ],
  [
    null,
    "apps/mobile/assets/forge/mark.png",
    "1024x1024-transparent-derived-from-brand/mobile-logo-source.png",
  ],
  [
    null,
    "apps/mobile/assets/forge/adaptive-icon-foreground.png",
    "432x432-safe-zone-derived-from-brand/mobile-logo-source.png",
  ],
  [
    null,
    "apps/mobile/assets/forge/adaptive-icon-monochrome.png",
    "copy-of-apps/mobile/assets/forge/adaptive-icon-foreground.png",
  ],
  [null, "apps/mobile/assets/forge/wordmark.png", "copy-of-brand/logo.png"],
  [null, "apps/mobile/assets/forge/BasierSquare-Regular.otf", "copy-of-brand/native"],
  [null, "apps/mobile/assets/forge/BasierSquare-Semibold.otf", "copy-of-brand/native"],
  [null, "apps/mobile/assets/forge/BasierSquareMono-Regular.ttf", "copy-of-brand/native"],
];
const brandFiles = brandMappings.map(([sourcePath, currentPath, relation]) => {
  const currentFile = path.join(ROOT, currentPath);
  return {
    sourcePath,
    currentPath,
    relation,
    ...(sourcePath ? { sourceSha256: sha(currentFile) } : {}),
    currentSha256: sha(currentFile),
  };
});
fs.writeFileSync(
  path.join(provenanceDir, "forge-brand-files.json"),
  `${JSON.stringify({ schemaVersion: 1, sourceRepository: EXAFORGE_REPOSITORY, sourceRevision: EXAFORGE_REVISION, brandFiles }, null, 2)}\n`,
);

process.stdout.write(
  `Generated ${copiedFiles.length} copied mappings: ${manifest.identicalFileCount} identical, ${manifest.adaptedFileCount} adapted, ${manifest.missingFileCount} missing.\n`,
);

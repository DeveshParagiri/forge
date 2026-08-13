import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SOURCE_EXTENSIONS = [
  ".ios.tsx",
  ".ios.ts",
  ".native.tsx",
  ".native.ts",
  ".tsx",
  ".ts",
  ".mjs",
  ".js",
  ".json",
];

function resolveFile(base) {
  const candidates = path.extname(base)
    ? [base]
    : [
        ...SOURCE_EXTENSIONS.map((extension) => `${base}${extension}`),
        ...SOURCE_EXTENSIONS.map((extension) => path.join(base, `index${extension}`)),
      ];
  return candidates.find((candidate) => fs.existsSync(candidate) && fs.statSync(candidate).isFile()) ?? null;
}

function packageExport(specifier) {
  const packageMappings = [
    ["@t3tools/client-runtime", "packages/client-runtime"],
    ["@t3tools/contracts", "packages/contracts"],
    ["@t3tools/shared", "packages/shared"],
  ];
  for (const [name, relativePackage] of packageMappings) {
    if (specifier !== name && !specifier.startsWith(`${name}/`)) continue;
    const packageRoot = path.join(ROOT, relativePackage);
    const packageJson = JSON.parse(fs.readFileSync(path.join(packageRoot, "package.json"), "utf8"));
    const exportKey = specifier === name ? "." : `.${specifier.slice(name.length)}`;
    const exported = packageJson.exports?.[exportKey];
    const target = typeof exported === "string" ? exported : exported?.import ?? exported?.default ?? exported?.types;
    return typeof target === "string" ? resolveFile(path.resolve(packageRoot, target)) : null;
  }
  return null;
}

function runtimeSpecifiers(sourceFile) {
  const specifiers = [];
  const visit = (node) => {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
      const clause = node.importClause;
      const named = clause?.namedBindings && ts.isNamedImports(clause.namedBindings)
        ? clause.namedBindings.elements
        : [];
      const typeOnly = clause?.isTypeOnly === true ||
        (clause && !clause.name && named.length > 0 && named.every((entry) => entry.isTypeOnly));
      if (!typeOnly) specifiers.push(node.moduleSpecifier.text);
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
      if (!node.isTypeOnly) specifiers.push(node.moduleSpecifier.text);
    } else if (
      ts.isCallExpression(node) &&
      node.arguments.length === 1 &&
      ts.isStringLiteral(node.arguments[0]) &&
      ((ts.isIdentifier(node.expression) && ["require", "import"].includes(node.expression.text)) ||
        node.expression.kind === ts.SyntaxKind.ImportKeyword)
    ) {
      specifiers.push(node.arguments[0].text);
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return specifiers;
}

function resolveSpecifier(specifier, fromFile) {
  if (specifier.startsWith(".")) return resolveFile(path.resolve(path.dirname(fromFile), specifier));
  return packageExport(specifier);
}

export function collectActiveGraph(entry = path.join(ROOT, "apps/mobile/index.ts")) {
  const graph = new Map();
  const pending = [path.resolve(entry)];
  while (pending.length > 0) {
    const file = pending.pop();
    if (!file || graph.has(file)) continue;
    const text = fs.readFileSync(file, "utf8");
    const source = ts.createSourceFile(
      file,
      text,
      ts.ScriptTarget.Latest,
      true,
      file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );
    const imports = runtimeSpecifiers(source);
    const local = imports.map((specifier) => resolveSpecifier(specifier, file)).filter(Boolean);
    graph.set(file, { imports, local, text });
    pending.push(...local);
  }
  return graph;
}

export function relativeToRoot(file) {
  return path.relative(ROOT, file).replaceAll(path.sep, "/");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const graph = collectActiveGraph();
  process.stdout.write(`${[...graph.keys()].map(relativeToRoot).sort().join("\n")}\n`);
}

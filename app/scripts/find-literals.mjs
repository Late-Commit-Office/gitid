// Lists string literals in the app's screens that look like text a person
// reads, so nothing skips the translation catalog. Run: node scripts/find-literals.mjs
// Exits 1 if any are found. Used by CI; see "Localisation" in the README.
//
// It uses TypeScript's own parser, so comments, imports and object keys are
// understood rather than guessed at.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import ts from "typescript";

const ROOTS = ["src/views", "src/main.ts", "src/lib/dom.ts", "src/lib/messages.ts"];

// Literals that are never shown as text: CSS classes, element and event
// names, attribute names, input types, codes compared against.
const CODE_LIKE = /^(|[a-z][a-zA-Z0-9]*(?:[-_.:][a-zA-Z0-9]+)*|[a-z-]+(?: [a-z-]+)*)$/;
/** Attributes whose value is read by people (or screen readers). */
const READ_ATTRS = new Set(["aria-label", "title", "placeholder", "alt"]);

function files(path) {
  if (path.endsWith(".ts")) return [path];
  return readdirSync(path).flatMap((f) => files(join(path, f)));
}

function looksLikeText(s) {
  // Words with spaces and a capital or punctuation, or a single capitalised word.
  if (!/[A-Za-z]/.test(s)) return false;
  if (/^[A-Z][a-z]+[.!?:…]?$/.test(s.trim())) return true;
  if (!/\s/.test(s)) return false;
  return !CODE_LIKE.test(s);
}

let found = 0;
for (const file of ROOTS.flatMap(files)) {
  const text = readFileSync(file, "utf8");
  const src = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const visit = (node) => {
    let value = null;
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) value = node.text;
    if (ts.isTemplateExpression(node)) {
      const parts = [node.head.text, ...node.templateSpans.map((s) => s.literal.text)];
      // `row row-${kind}`, `setting-${key}`: built names, not text.
      const codeOnly = parts.every((p) => CODE_LIKE.test(p.trim()) || /^[-_.:/ ]*$/.test(p));
      if (!codeOnly) value = parts.join("…");
    }
    if (value !== null) {
      const parent = node.parent;
      const isKey = parent && ts.isPropertyAssignment(parent) && parent.name === node;
      const isImport = parent && (ts.isImportDeclaration(parent) || ts.isExportDeclaration(parent));
      // t("key") and friends: the argument is a catalog key, not text.
      // t("key") and querySelector(".css selector"): not text either.
      const isCatalogKey =
        parent && ts.isCallExpression(parent) && /(^(t|tn|tx|hasKey)|querySelector(All)?)$/.test(parent.expression.getText());
      // el("code", {}, "git var GIT_AUTHOR_IDENT"): commands and keys stay as written.
      const isCode =
        parent && ts.isCallExpression(parent) && parent.expression.getText() === "el" &&
        parent.arguments[0] && ts.isStringLiteral(parent.arguments[0]) && parent.arguments[0].text === "code";
      const readAttr = parent && ts.isPropertyAssignment(parent) && READ_ATTRS.has(parent.name.getText().replace(/"/g, ""));
      if (!isKey && !isImport && !isCatalogKey && !isCode && (readAttr ? /[A-Za-z]/.test(value) : looksLikeText(value))) {
        const { line } = src.getLineAndCharacterOfPosition(node.getStart());
        console.log(`${file}:${line + 1}: ${JSON.stringify(value)}`);
        found++;
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(src);
}
console.log(found ? `\n${found} strings to move into src/locales/en.json` : "No untranslated strings.");
process.exit(found ? 1 : 0);

// Checks every language file in src/locales against en.json, the source:
//
// - no entries English doesn't have, and none missing;
// - the same {placeholders} as the English (a translation may reorder them,
//   not drop or invent them);
// - plural entries (`x.one`, `x.other`, ...) use the language's own plural
//   forms: Chinese has only `other`, English needs `one` and `other`;
// - nothing out of date: src/locales/sources/<lang>.json records the English
//   each translation was made from, so an English change flags its
//   translations for review.
//
//   npm run check:locales                         check (CI runs this)
//   npm run check:locales -- --update zh-Hans     after reviewing: record the current
//                                                 English for every zh-Hans entry
//
// Exits 1 on any problem.

import { readFileSync, readdirSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const DIR = "src/locales";
const SOURCE = "en";

const args = process.argv.slice(2);
const updateIndex = args.indexOf("--update");
const update = updateIndex >= 0 ? args[updateIndex + 1] : null;

const read = (file) => {
  const data = JSON.parse(readFileSync(file, "utf8"));
  for (const [k, v] of Object.entries(data)) {
    if (typeof v !== "string") throw new Error(`${file}: "${k}" isn't a string`);
  }
  return data;
};

const placeholders = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
const same = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);
const PLURAL = /^(.*)\.(zero|one|two|few|many|other)$/;

const en = read(join(DIR, `${SOURCE}.json`));
const problems = [];

// The English itself: every plural entry has its `other` form.
const pluralBases = new Set();
for (const key of Object.keys(en)) {
  const m = PLURAL.exec(key);
  if (m) pluralBases.add(m[1]);
}
for (const base of pluralBases) {
  if (!(`${base}.other` in en)) problems.push(`en: "${base}" has plural forms but no "${base}.other"`);
}

const locales = readdirSync(DIR)
  .filter((f) => f.endsWith(".json") && f !== `${SOURCE}.json`)
  .map((f) => f.slice(0, -".json".length));

for (const lang of locales) {
  const file = join(DIR, `${lang}.json`);
  const own = read(file);
  const sourcesFile = join(DIR, "sources", `${lang}.json`);
  const sources = existsSync(sourcesFile) ? read(sourcesFile) : {};
  const categories = new Set(new Intl.PluralRules(lang).resolvedOptions().pluralCategories);

  // What this language must have: every plain entry, and for plurals, the
  // forms the language uses (`other` at least).
  const required = new Set();
  for (const key of Object.keys(en)) {
    const m = PLURAL.exec(key);
    if (!m) required.add(key);
    else for (const c of categories) required.add(`${m[1]}.${c}`);
  }

  const missing = [...required].filter((k) => !(k in own));
  if (missing.length) {
    problems.push(`${lang}: ${missing.length} missing:\n    ${missing.join("\n    ")}`);
  }

  for (const [key, text] of Object.entries(own)) {
    const m = PLURAL.exec(key);
    if (m && pluralBases.has(m[1])) {
      if (!categories.has(m[2])) problems.push(`${lang}: "${key}": ${lang} has no "${m[2]}" plural form`);
      // Compare with the English `other`, which every plural has.
      const english = en[`${m[1]}.other`];
      if (!same(placeholders(text), placeholders(english))) {
        problems.push(`${lang}: "${key}" has {${placeholders(text).join("}, {")}}, the English has {${placeholders(english).join("}, {")}}`);
      }
    } else if (!(key in en)) {
      problems.push(`${lang}: "${key}" isn't in en.json (renamed or removed?)`);
      continue;
    } else if (!same(placeholders(text), placeholders(en[key]))) {
      problems.push(`${lang}: "${key}" has {${placeholders(text).join("}, {")}}, the English has {${placeholders(en[key]).join("}, {")}}`);
    }
  }

  if (update === lang) {
    const recorded = {};
    for (const key of Object.keys(own)) {
      const m = PLURAL.exec(key);
      const english = m && pluralBases.has(m[1]) ? (en[key] ?? en[`${m[1]}.other`]) : en[key];
      if (english !== undefined) recorded[key] = english;
    }
    mkdirSync(join(DIR, "sources"), { recursive: true });
    writeFileSync(sourcesFile, JSON.stringify(recorded, null, 2) + "\n");
    console.log(`Recorded the English for ${Object.keys(recorded).length} ${lang} entries in ${sourcesFile}.`);
    continue;
  }

  // Out of date: the English changed after the translation was made.
  const stale = [];
  for (const key of Object.keys(own)) {
    const m = PLURAL.exec(key);
    const english = m && pluralBases.has(m[1]) ? (en[key] ?? en[`${m[1]}.other`]) : en[key];
    if (english === undefined) continue;
    if (!(key in sources)) stale.push(`${key}  (no English recorded)`);
    else if (sources[key] !== english) stale.push(`${key}\n      was: ${sources[key]}\n      now: ${english}`);
  }
  if (stale.length) {
    problems.push(
      `${lang}: ${stale.length} to review, the English changed since they were translated:\n    ${stale.join("\n    ")}\n` +
        `  Update the translations, then: npm run check:locales -- --update ${lang}`,
    );
  }
}

if (problems.length) {
  console.log(problems.join("\n\n"));
  process.exit(1);
}
console.log(`Locales OK: ${locales.join(", ")} match en.json (${Object.keys(en).length} entries).`);

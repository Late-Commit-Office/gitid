// Translations. Every user-visible string goes through t() (plain text) or
// tx() (text with elements inside it, like a <code> key). Entries live in
// src/locales/<locale>.json, one flat object per language; en.json is the
// source and the fallback for anything another language doesn't have.
//
// Entries use named placeholders, `{key}`, so a translation can put them in
// any order. Plural entries are split by CLDR category: `x.one`, `x.other`
// (English needs both; Chinese only `x.other`), and looked up with tn().
//
// Translations are inserted as text, never as HTML.

import en from "../locales/en.json";
import zhHans from "../locales/zh-Hans.json";

export const LOCALES = ["en", "zh-Hans"] as const;
export type Locale = (typeof LOCALES)[number];

/**
 * A made-up language for development builds only: every entry is English
 * with accents, about a third longer, in ⟦brackets⟧. Text that skipped the
 * catalog stays plain English, and layouts too tight for longer words show.
 */
export const PSEUDO = "en-XA";
export type Pseudo = typeof PSEUDO;
/** Offered only in development builds (`npm run tauri dev`). */
export const PSEUDO_AVAILABLE: boolean = import.meta.env.DEV;
/** Its name in the language picker; not a real language, so not translated. */
export const PSEUDO_NAME = "Pseudo (en-XA)";

/** What the user picked in Settings: a language, or follow the system. */
export type LanguageChoice = Locale | Pseudo | "system";

/** Each language's own name for itself, for the language picker. */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  "zh-Hans": "简体中文",
};

type Catalog = Record<string, string>;
type English = typeof en;

/** Every key in en.json. */
export type MessageKey = keyof English;
/** Keys with plural forms, without the `.one` / `.other` ending. */
export type PluralKey = { [K in MessageKey]: K extends `${infer Base}.other` ? Base : never }[MessageKey];

export type Params = Record<string, string | number>;
/** For tx(): a placeholder can also be an element (a <code> key, a link). */
export type RichParams = Record<string, string | number | Node>;

const CATALOGS: Record<Locale, Catalog> = { en, "zh-Hans": zhHans };

let current: Locale | Pseudo = "en";

/** The language in use. */
export function locale(): Locale | Pseudo {
  return current;
}

/** Switch language. Callers re-render; nothing here touches the page except `<html lang>`. */
export function setLocale(next: Locale | Pseudo): void {
  current = next;
  document.documentElement.lang = next;
}

/** The language to use for a choice; "system" follows the system's preferred languages. */
export function resolveLocale(
  choice: LanguageChoice,
  preferred: readonly string[] = systemLanguages(),
): Locale | Pseudo {
  if (choice === PSEUDO) return PSEUDO_AVAILABLE ? PSEUDO : "en";
  if (choice !== "system") return choice;
  for (const tag of preferred) {
    const found = matchLocale(tag);
    if (found) return found;
  }
  return "en";
}

/**
 * Map a BCP 47 tag to a supported language. Chinese is only matched when
 * it's Simplified: zh, zh-CN, zh-SG and zh-Hans-*; Traditional (zh-TW,
 * zh-HK, zh-MO, zh-Hant-*) moves on to the next preferred language.
 */
export function matchLocale(tag: string): Locale | null {
  const parts = tag.toLowerCase().split(/[-_]/);
  if (parts[0] === "en") return "en";
  if (parts[0] === "zh") {
    if (parts.includes("hant")) return null;
    if (parts.includes("hans")) return "zh-Hans";
    const region = parts[1];
    if (region === "tw" || region === "hk" || region === "mo") return null;
    return "zh-Hans";
  }
  return null;
}

function systemLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  return navigator.languages?.length ? navigator.languages : [navigator.language];
}

/** The text of an entry with its placeholders filled in. */
export function t(key: MessageKey, params?: Params): string {
  return fill(lookup(key), params, key);
}

/** A plural entry, chosen by `count`. `count` is also available as `{count}`. */
export function tn(key: PluralKey, count: number, params?: Params): string {
  return fill(lookup(pluralKey(key, count)), { count: formatNumber(count), ...params }, key);
}

/**
 * An entry whose placeholders may be elements, as a fragment to append.
 * Text pieces become text nodes, so nothing is ever parsed as HTML.
 */
export function tx(key: MessageKey, params: RichParams): DocumentFragment {
  const frag = document.createDocumentFragment();
  const text = lookup(key);
  let last = 0;
  for (const m of text.matchAll(PLACEHOLDER)) {
    const at = m.index ?? 0;
    if (at > last) frag.appendChild(document.createTextNode(text.slice(last, at)));
    const value = params[m[1]];
    if (value === undefined) {
      missingParam(key, m[1]);
      frag.appendChild(document.createTextNode(m[0]));
    } else {
      frag.appendChild(typeof value === "object" ? value : document.createTextNode(String(value)));
    }
    last = at + m[0].length;
  }
  if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
  return frag;
}

/** "a, b and c" in the current language. */
export function formatList(items: readonly string[]): string {
  // Intl.ListFormat arrived in Safari 14.1; macOS 10.15 may have an older WebKit.
  if (typeof Intl.ListFormat !== "function") return items.join(current === "en" ? ", " : "、");
  return new Intl.ListFormat(current, { type: "conjunction" }).format(items);
}

export function formatNumber(n: number): string {
  return new Intl.NumberFormat(current).format(n);
}

/** Whether a key exists in en.json, for codes that arrive from the backend. */
export function hasKey(key: string): key is MessageKey {
  return key in en;
}

// ---------------------------------------------------------------- internals

const PLACEHOLDER = /\{(\w+)\}/g;

function lookup(key: string): string {
  if (current === PSEUDO) {
    const english = (en as Catalog)[key];
    if (english !== undefined) return pseudo(english);
    missingKey(key);
    return key;
  }
  const own = CATALOGS[current][key];
  if (own !== undefined) return own;
  const fallback = (en as Catalog)[key];
  if (fallback !== undefined) {
    if (current !== "en") missingKey(key);
    return fallback;
  }
  // Only reachable with a key built at runtime (pluralKey, backend codes).
  missingKey(key);
  return key;
}

function pluralKey(base: string, count: number): string {
  const category = new Intl.PluralRules(current).select(count);
  const exact = `${base}.${category}`;
  const own = current === PSEUDO ? {} : CATALOGS[current];
  return exact in own || exact in en ? exact : `${base}.other`;
}

const ACCENTED: Record<string, string> = Object.fromEntries(
  [..."abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"].map((c, i) => [
    c,
    [..."åƀçđéƒĝĥîĵķļɱñöþǫŕšţûṽŵẋýžÅƁÇĐÉƑĜĤÎĴĶĻṀÑÖÞǪŔŠŢÛṼŴẊÝŽ"][i],
  ]),
);

/** "Saved {key}." -> "⟦Šåṽéð {key}. ·····⟧": placeholders stay as they are. */
function pseudo(text: string): string {
  const parts = text.split(PLACEHOLDER_SPLIT);
  const accented = parts.map((p, i) => (i % 2 === 1 ? `{${p}}` : [...p].map((c) => ACCENTED[c] ?? c).join(""))).join("");
  const pad = "·".repeat(Math.ceil(text.length * 0.3));
  return `⟦${accented} ${pad}⟧`;
}

/** Splits on placeholders, keeping their names at odd indexes. */
const PLACEHOLDER_SPLIT = /\{(\w+)\}/;

function fill(text: string, params: Params | undefined, key: string): string {
  return text.replace(PLACEHOLDER, (whole, name: string) => {
    const value = params?.[name];
    if (value === undefined) {
      missingParam(key, name);
      return whole;
    }
    return String(value);
  });
}

const warned = new Set<string>();

function missingKey(key: string): void {
  warnOnce(`${current}:${key}`, `i18n: no "${key}" in ${current}`);
}

function missingParam(key: string, name: string): void {
  warnOnce(`${key}{${name}}`, `i18n: "${key}" needs {${name}}`);
}

function warnOnce(id: string, text: string): void {
  if (!import.meta.env.DEV || warned.has(id)) return;
  warned.add(id);
  console.warn(text);
}

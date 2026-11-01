// Phrasing for what the backend sends as data: errors, identity warnings,
// rule lines and managed settings. The backend never sends a sentence the
// app shows (except git's own words, as `detail`); everything is worded here,
// in the current language.

import { AppError } from "./api";
import { el } from "./dom";
import { type MessageKey, formatList, hasKey, t, tx } from "./i18n";
import { tilde } from "./paths";
import type { Described, ErrorParam, Part, Role, Warning } from "./types";

// ---------------------------------------------------------------- errors

/** Any thrown value as an AppError. */
export function asAppError(e: unknown): AppError {
  if (e instanceof AppError) return e;
  const text = e instanceof Error ? e.message : String(e);
  return new AppError({ kind: "internal", code: "internal", params: {}, message: text, detail: text });
}

/** The message for an error, in the current language (English if it has no entry). */
export function errorText(e: unknown): string {
  const err = asAppError(e);
  const key = `error.${err.code}`;
  if (!hasKey(key)) return sentence(err.message);
  const params: Record<string, string> = {};
  for (const [name, value] of Object.entries(err.params)) params[name] = param(name, value);
  if (err.code === "unsupported") {
    const feature = `feature.${err.params.feature}`;
    if (hasKey(feature)) params.feature = t(feature);
  }
  return t(key, params);
}

/** git's or the system's own words about an error, if any; shown as they are. */
export function errorDetail(e: unknown): string | null {
  const err = asAppError(e);
  if (err.detail) return err.detail;
  // git's message for a key it refused travels as a param.
  return typeof err.params.detail === "string" ? err.params.detail : null;
}

function param(name: string, value: ErrorParam): string {
  if (value === null) return "";
  if (Array.isArray(value)) return formatList(name === "searched" ? value.map(tilde) : value);
  if (typeof value === "string" && (name === "path" || name === "git")) return tilde(value);
  return String(value);
}

/** English fallback text from the backend, as a sentence. */
function sentence(s: string): string {
  const first = s.split(/\s/, 1)[0];
  const t = first.includes(".") || first.includes("/") ? s : s.charAt(0).toUpperCase() + s.slice(1);
  return /[.!?]$/.test(t) ? t : t + ".";
}

// ---------------------------------------------------------------- identity

/** "author", "committer" */
export function roleName(role: Role): string {
  return t(`role.${role}`);
}

/** "author name", "committer email", ... */
export function fieldName(role: Role, part: Part): string {
  return t(`field.${role}.${part}`);
}

/** A warning as a sentence; keys and variables are set in code style. */
export function warningText(w: Warning): DocumentFragment {
  const code = (s: string) => el("code", {}, s);
  switch (w.kind) {
    case "missing":
      return tx("warning.missing", { role: roleName(w.role) });
    case "guessed":
      return tx("warning.guessed", { field: fieldName(w.role, w.part) });
    case "fromEnv":
      return tx("warning.fromEnv", { field: fieldName(w.role, w.part), var: code(w.var) });
    case "injected":
      return tx("warning.injected", { field: fieldName(w.role, w.part), key: code(w.key) });
    case "authorCommitterDiffer":
      return tx("warning.authorCommitterDiffer", {});
    case "guessingAllowed":
      return tx("warning.guessingAllowed", { key: code("user.useConfigOnly") });
  }
}

/** git's own message for a warning, if it has one (why git would refuse). */
export function warningDetail(w: Warning): string | null {
  return w.kind === "missing" && w.message ? w.message : null;
}

// ---------------------------------------------------------------- rules

/** One line of a rule's description; hosts and patterns in code style. */
export function ruleLine(d: Described): DocumentFragment {
  const code = (s: string) => el("code", {}, s);
  if (d.kind === "server") {
    return d.sshPort
      ? tx("rule.line.serverPort", { host: code(d.host), port: d.sshPort })
      : tx("rule.line.server", { host: code(d.host) });
  }
  const c = d.condition;
  switch (c.kind) {
    case "folder":
      return tx("rule.line.folder", { pattern: code(c.pattern) });
    case "remoteUrl":
      return tx("rule.line.remoteUrl", { pattern: code(c.pattern) });
    case "other":
      return tx("rule.line.other", { condition: code(c.raw) });
  }
}

// ---------------------------------------------------------------- settings

/** A managed setting's label; the key itself if the catalog doesn't know it. */
export function settingLabel(key: string): string {
  const k = `setting.${key}.label`;
  return hasKey(k) ? t(k) : key;
}

export function settingHelp(key: string): string {
  const k = `setting.${key}.help`;
  return hasKey(k) ? t(k) : "";
}

/** A translated placeholder where the example is words (a name), else the backend's. */
export function settingPlaceholder(key: string, fallback: string): string {
  const k = `setting.${key}.placeholder`;
  return hasKey(k) ? t(k) : fallback;
}

export function choiceLabel(key: string, value: string): string {
  const k = `setting.${key}.choice.${value}`;
  return hasKey(k) ? t(k as MessageKey) : value;
}

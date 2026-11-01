// The identity view: pick a folder, see who commits there will be from,
// where each value comes from, and what to fix.

import { api, AppError } from "../lib/api";
import { type MessageKey, t, tx } from "../lib/i18n";
import { asAppError, errorDetail, errorText, warningDetail, warningText } from "../lib/messages";
import { append, clear, el } from "../lib/dom";
import { basename, tilde } from "../lib/paths";
import { forgetFolder, recentFolders, rememberFolder } from "../lib/recent";
import type {
  Field,
  IdentityView,
  Part,
  RoleIdentity,
  RoleRules,
  Scope,
  Source,
  WarningReport,
} from "../lib/types";

interface State {
  folder: string | null;
  loading: boolean;
  result: IdentityView | null;
  error: AppError | null;
}

export interface IdentityOptions {
  /** Switch to the Settings view. */
  openSettings: () => void;
}

/** What fix buttons can do. */
interface Actions {
  openSettings: () => void;
  turnOnGuard: () => Promise<void>;
}

export function mountIdentityView(root: HTMLElement, options: IdentityOptions): void {
  const state: State = { folder: null, loading: false, result: null, error: null };
  const actions: Actions = {
    openSettings: options.openSettings,
    turnOnGuard: async () => {
      await api.setSetting("user.useConfigOnly", "true");
      if (state.folder) await open(state.folder);
    },
  };
  const bar = el("section", { class: "folder-bar", attrs: { "aria-label": t("common.folder") } });
  const body = el("section", { class: "result", attrs: { "aria-live": "polite" } });
  append(root, bar, body);

  async function open(folder: string): Promise<void> {
    Object.assign(state, { folder, loading: true, error: null });
    render();
    try {
      state.result = await api.identity(folder);
      rememberFolder(folder);
    } catch (e) {
      state.result = null;
      state.error = asAppError(e);
      if (state.error.kind === "notADirectory") forgetFolder(folder);
    } finally {
      state.loading = false;
      render();
    }
  }

  async function choose(): Promise<void> {
    try {
      const picked = await api.pickFolder(t("dialog.chooseFolder"));
      if (picked) await open(picked);
    } catch (e) {
      state.error = asAppError(e);
      render();
    }
  }

  function render(): void {
    renderBar();
    renderBody();
  }

  function renderBar(): void {
    clear(bar);
    const chooseButton = el(
      "button",
      { class: "primary", type: "button", on: { click: () => void choose() } },
      t(state.folder ? "common.chooseAnother" : "common.chooseFolder"),
    );
    const row = el("div", { class: "bar-row" }, chooseButton);
    if (state.folder) {
      append(
        row,
        el("span", { class: "current mono", title: state.folder }, tilde(state.folder)),
        el(
          "button",
          {
            type: "button",
            title: t("identity.refreshTitle"),
            attrs: state.loading ? { disabled: "" } : {},
            on: { click: () => state.folder && void open(state.folder) },
          },
          t("identity.refresh"),
        ),
      );
    }
    append(bar, row);

    const others = recentFolders().filter((p) => p !== state.folder);
    if (others.length > 0) {
      const list = el("ul", { class: "recent", attrs: { "aria-label": t("common.recentFolders") } });
      for (const p of others) {
        append(
          list,
          el(
            "li",
            {},
            el("button", { type: "button", class: "chip", title: tilde(p), on: { click: () => void open(p) } }, basename(p)),
          ),
        );
      }
      append(bar, el("div", { class: "recent-row" }, el("span", { class: "muted" }, t("common.recent")), list));
    }
  }

  function renderBody(): void {
    clear(body);
    body.setAttribute("aria-busy", String(state.loading));
    if (state.loading) {
      append(body, el("p", { class: "muted" }, t("identity.checking")));
      return;
    }
    if (state.error) {
      append(body, errorPanel(state.error));
      return;
    }
    if (!state.result) {
      append(body, emptyState());
      return;
    }
    append(body, ...identityPanel(state.result, actions));
  }

  render();
  const last = recentFolders()[0];
  if (last) void open(last);
}

// ---------------------------------------------------------------- pieces

function emptyState(): HTMLElement {
  return el(
    "div",
    { class: "empty" },
    el("p", { class: "lede" }, t("identity.empty.lede")),
    el("p", { class: "muted" }, t("identity.empty.body")),
  );
}

function errorPanel(e: AppError): HTMLElement {
  // These two say all there is to say; anything else gets the error's own words too.
  const known: Record<string, MessageKey> = {
    notADirectory: "identity.error.gone",
    gitNotFound: "identity.error.noGit",
  };
  const own = known[e.kind];
  const detail = own ? null : errorDetail(e);
  return el(
    "div",
    { class: "verdict problem", attrs: { role: "alert" } },
    el("p", { class: "lede" }, t(own ?? "identity.error.generic")),
    own ? null : el("p", {}, errorText(e)),
    detail ? el("p", { class: "mono detail" }, detail) : null,
  );
}

function identityPanel(v: IdentityView, actions: Actions): HTMLElement[] {
  const out: HTMLElement[] = [verdict(v), roleBlock(t("identity.author"), v.author, v.authorRules)];
  if (sameIdentity(v.author, v.committer)) {
    out.push(el("p", { class: "muted committer-same" }, t("identity.committerSame")));
  } else {
    out.push(roleBlock(t("identity.committer"), v.committer, v.committerRules));
  }
  const problems = v.warnings.filter((w) => w.severity === "problem");
  const notes = v.warnings.filter((w) => w.severity === "note");
  if (problems.length) out.push(issues(t("identity.toFix"), problems, "problem", v, actions));
  if (notes.length) out.push(issues(t("identity.worthKnowing"), notes, "note", v, actions));
  out.push(environment(v));
  return out;
}

function verdict(v: IdentityView): HTMLElement {
  const a = v.author;
  const failing = a.error !== null;
  const tone = v.hasProblems ? "problem" : "ok";
  const where = v.repo
    ? t(failing ? "identity.where.repoFailing" : "identity.where.repo", { repo: tilde(v.repo.workTree ?? v.repo.gitDir) })
    : t("identity.where.noRepo");

  const lede = t(failing ? "identity.lede.failing" : v.hasProblems ? "identity.lede.madeUp" : "identity.lede.ok");

  return el(
    "div",
    { class: `verdict ${tone}` },
    el("p", { class: "lede" }, lede),
    failing ? null : el("p", { class: "signature mono" }, signature(a)),
    el("p", { class: "muted" }, where),
  );
}

function signature(r: RoleIdentity): string {
  return `${r.name?.value ?? "?"} <${r.email?.value ?? "?"}>`;
}

function sameIdentity(a: RoleIdentity, b: RoleIdentity): boolean {
  const key = (r: RoleIdentity) => JSON.stringify([r.name, r.email, r.error]);
  return key(a) === key(b);
}

function roleBlock(title: string, r: RoleIdentity, rules: RoleRules): HTMLElement {
  const rows = el("dl", { class: "trail" });
  for (const part of ["name", "email"] as Part[]) {
    const field = r[part];
    append(
      rows,
      el("dt", {}, t(part === "name" ? "identity.name" : "identity.email")),
      el("dd", { class: "value mono" }, field ? field.value : el("span", { class: "missing" }, t("identity.notSet"))),
      el("dd", { class: "source" }, field ? sourceOf(field, rules[part]) : t("identity.nothingConfigured")),
    );
  }
  return el("section", { class: "role" }, el("h2", {}, title), rows);
}

const scopeText: Record<Scope, MessageKey> = {
  system: "identity.source.system",
  global: "identity.source.global",
  local: "identity.source.local",
  worktree: "identity.source.worktree",
  command: "identity.source.command",
  unknown: "identity.source.unknown",
};

function sourceOf(field: Field, ruleId: string | null): HTMLElement {
  const s: Source = field.source;
  switch (s.kind) {
    case "env":
      return el("span", {}, tx("identity.source.env", { var: el("code", {}, s.var) }));
    case "guessed":
      return el("span", { class: "problem-text" }, t("identity.source.guessed"));
    case "config": {
      const file = s.origin.kind === "file" ? s.origin.value : null;
      return el(
        "span",
        {},
        ruleId ? t("identity.source.viaRule", { scope: t(scopeText[s.scope]), rule: ruleId }) : t(scopeText[s.scope]),
        s.key.startsWith("user.") ? null : el("span", { class: "muted" }, t("identity.source.key", { key: s.key })),
        file ? el("span", { class: "file mono", title: file }, tilde(file)) : null,
      );
    }
  }
}

/** A button that fixes the problem, or opens the place to fix it. */
function fixFor(w: WarningReport, v: IdentityView, actions: Actions): HTMLElement | null {
  const W = w.warning;
  const settingsButton = (label: MessageKey) =>
    el("button", { type: "button", on: { click: actions.openSettings } }, t(label));
  switch (W.kind) {
    case "missing": {
      // Name only what is actually missing for that role.
      const r = W.role === "author" ? v.author : v.committer;
      if (r.name && !r.email) return settingsButton("identity.fix.email");
      if (r.email && !r.name) return settingsButton("identity.fix.name");
      return settingsButton("identity.fix.both");
    }
    case "guessed":
      return settingsButton(W.part === "name" ? "identity.fix.name" : "identity.fix.email");
    case "guessingAllowed": {
      const button = el("button", { type: "button" }, t("identity.fix.turnOn"));
      button.addEventListener("click", async () => {
        button.disabled = true;
        try {
          await actions.turnOnGuard();
        } catch (e) {
          button.disabled = false;
          button.textContent = t("identity.fix.turnOnFailed");
          button.title = errorText(e);
        }
      });
      return el(
        "span",
        {},
        button,
        " ",
        el("span", { class: "muted fix-note" }, tx("identity.fix.turnOnNote", { key: el("code", {}, "user.useConfigOnly") })),
      );
    }
    default:
      return null;
  }
}

function issues(
  title: string,
  list: WarningReport[],
  tone: "problem" | "note",
  v: IdentityView,
  actions: Actions,
): HTMLElement {
  const ul = el("ul", { class: `issues ${tone}` });
  for (const w of list) {
    const fix = fixFor(w, v, actions);
    const detail = warningDetail(w.warning);
    append(
      ul,
      el(
        "li",
        {},
        el("p", {}, warningText(w.warning)),
        detail ? el("p", { class: "mono detail" }, detail) : null,
        fix ? el("div", { class: "fix" }, fix) : null,
      ),
    );
  }
  return el("section", { class: "issues-block" }, el("h2", {}, title), ul);
}

function environment(v: IdentityView): HTMLElement {
  const note = el(
    "p",
    { class: "muted footnote" },
    tx("identity.env.note", {
      example: el("code", {}, "GIT_AUTHOR_NAME"),
      command: el("code", {}, "git var GIT_AUTHOR_IDENT"),
    }),
  );
  if (v.env.length === 0) return note;
  const rows = el("dl", { class: "env" });
  for (const e of v.env) {
    append(rows, el("dt", { class: "mono" }, e.name), el("dd", { class: "mono" }, e.sensitive ? t("identity.env.hidden") : e.value));
  }
  return el(
    "section",
    { class: "env-block" },
    el("h2", {}, t("identity.env.title")),
    rows,
    note,
  );
}

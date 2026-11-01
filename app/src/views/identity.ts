// The identity view: pick a folder, see who commits there will be from,
// where each value comes from, and what to fix.

import { api, AppError } from "../lib/api";
import { t } from "../lib/i18n";
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
  const bar = el("section", { class: "folder-bar", attrs: { "aria-label": "Folder" } });
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
      state.folder ? "Choose another folder" : "Choose a folder",
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
            title: "Check this folder again",
            attrs: state.loading ? { disabled: "" } : {},
            on: { click: () => state.folder && void open(state.folder) },
          },
          "Refresh",
        ),
      );
    }
    append(bar, row);

    const others = recentFolders().filter((p) => p !== state.folder);
    if (others.length > 0) {
      const list = el("ul", { class: "recent", attrs: { "aria-label": "Recent folders" } });
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
      append(bar, el("div", { class: "recent-row" }, el("span", { class: "muted" }, "Recent:"), list));
    }
  }

  function renderBody(): void {
    clear(body);
    body.setAttribute("aria-busy", String(state.loading));
    if (state.loading) {
      append(body, el("p", { class: "muted" }, "Checking…"));
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
    el("p", { class: "lede" }, "Pick a folder to see who your commits there will be from."),
    el(
      "p",
      { class: "muted" },
      "Any folder works: a repository, a folder of repositories, or one where you're about to create a repo.",
    ),
  );
}

function errorPanel(e: AppError): HTMLElement {
  const text: Record<string, string> = {
    notADirectory: "This folder no longer exists. Choose another one.",
    gitNotFound: "git wasn't found on this computer. Install git, then reopen gitid.",
  };
  return el(
    "div",
    { class: "verdict problem", attrs: { role: "alert" } },
    el("p", { class: "lede" }, text[e.kind] ?? "Couldn't check this folder."),
    text[e.kind] ? null : el("p", {}, errorText(e)),
    text[e.kind] || !errorDetail(e) ? null : el("p", { class: "mono detail" }, errorDetail(e)),
  );
}

function identityPanel(v: IdentityView, actions: Actions): HTMLElement[] {
  const out: HTMLElement[] = [verdict(v), roleBlock("Author", v.author, v.authorRules)];
  if (sameIdentity(v.author, v.committer)) {
    out.push(el("p", { class: "muted committer-same" }, "The committer is the same as the author."));
  } else {
    out.push(roleBlock("Committer", v.committer, v.committerRules));
  }
  const problems = v.warnings.filter((w) => w.severity === "problem");
  const notes = v.warnings.filter((w) => w.severity === "note");
  if (problems.length) out.push(issues("To fix", problems, "problem", v, actions));
  if (notes.length) out.push(issues("Worth knowing", notes, "note", v, actions));
  out.push(environment(v));
  return out;
}

function verdict(v: IdentityView): HTMLElement {
  const a = v.author;
  const failing = a.error !== null;
  const tone = v.hasProblems ? "problem" : "ok";
  const where = v.repo
    ? `${failing ? "In" : "Applies in"} the repository ${tilde(v.repo.workTree ?? v.repo.gitDir)}.`
    : "This folder isn't a repository, so this is what a new repo here would use.";

  const lede = failing
    ? "Commits here will fail: git has no identity to use."
    : v.hasProblems
      ? "Commits here will use an identity git made up:"
      : "Commits here will be made as";

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
      el("dt", {}, part === "name" ? "Name" : "Email"),
      el("dd", { class: "value mono" }, field ? field.value : el("span", { class: "missing" }, "not set")),
      el("dd", { class: "source" }, field ? sourceOf(field, rules[part]) : "nothing configured"),
    );
  }
  return el("section", { class: "role" }, el("h2", {}, title), rows);
}

const scopeText: Record<Scope, string> = {
  system: "System config",
  global: "Your global config",
  local: "This repository's config",
  worktree: "This worktree's config",
  command: "Set by git -c or GIT_CONFIG_* variables",
  unknown: "Config file",
};

function sourceOf(field: Field, ruleId: string | null): HTMLElement {
  const s: Source = field.source;
  switch (s.kind) {
    case "env":
      return el("span", {}, "Environment variable ", el("code", {}, s.var));
    case "guessed":
      return el("span", { class: "problem-text" }, "Made up by git from your computer's login. Not configured.");
    case "config": {
      const file = s.origin.kind === "file" ? s.origin.value : null;
      return el(
        "span",
        {},
        ruleId ? `${scopeText[s.scope]}, through the rule “${ruleId}”` : scopeText[s.scope],
        s.key.startsWith("user.") ? null : el("span", { class: "muted" }, ` (${s.key})`),
        file ? el("span", { class: "file mono", title: file }, tilde(file)) : null,
      );
    }
  }
}

/** A button that fixes the problem, or opens the place to fix it. */
function fixFor(w: WarningReport, v: IdentityView, actions: Actions): HTMLElement | null {
  const W = w.warning;
  const settingsButton = (what: string) =>
    el("button", { type: "button", on: { click: actions.openSettings } }, `Set your ${what} in Settings`);
  switch (W.kind) {
    case "missing": {
      // Name only what is actually missing for that role.
      const r = W.role === "author" ? v.author : v.committer;
      const parts = [r.name ? null : "name", r.email ? null : "email"].filter((x): x is string => x !== null);
      return settingsButton(parts.length ? parts.join(" and ") : "name and email");
    }
    case "guessed":
      return settingsButton(W.part);
    case "guessingAllowed": {
      const button = el("button", { type: "button" }, "Turn this on");
      button.addEventListener("click", async () => {
        button.disabled = true;
        try {
          await actions.turnOnGuard();
        } catch (e) {
          button.disabled = false;
          button.textContent = "Couldn't turn it on";
          button.title = errorText(e);
        }
      });
      return el("span", {}, button, el("span", { class: "muted fix-note" }, " Sets user.useConfigOnly in your global config."));
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
    "Variables like GIT_AUTHOR_NAME set in a terminal override all of this, and this app can't see them. To check a terminal, run ",
    el("code", {}, "git var GIT_AUTHOR_IDENT"),
    " in it.",
  );
  if (v.env.length === 0) return note;
  const rows = el("dl", { class: "env" });
  for (const e of v.env) {
    append(rows, el("dt", { class: "mono" }, e.name), el("dd", { class: "mono" }, e.sensitive ? "(set, value hidden)" : e.value));
  }
  return el(
    "section",
    { class: "env-block" },
    el("h2", {}, "Environment this app sees"),
    rows,
    note,
  );
}

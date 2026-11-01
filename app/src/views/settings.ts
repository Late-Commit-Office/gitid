// The settings view: your default identity, the "never guess" guard, a few
// common defaults, commit signing, and which git the app uses.

import { api, AppError } from "../lib/api";
import { append, clear, el } from "../lib/dom";
import { tilde } from "../lib/paths";
import type { SettingGroup, SettingValue, SettingsView } from "../lib/types";

const GROUPS: { id: SettingGroup; title: string; note?: string }[] = [
  {
    id: "identity",
    title: "Your default identity",
    note: "Rules and repository settings can still use a different identity in specific folders.",
  },
  { id: "defaults", title: "Defaults" },
  { id: "signing", title: "Commit signing" },
];

interface Status {
  key: string;
  ok: boolean;
  text: string;
}

export interface SettingsOptions {
  /** Called after the app starts using a different git. */
  onGitChanged: () => void;
}

export function mountSettingsView(root: HTMLElement, options: SettingsOptions): void {
  const body = el("div", { class: "settings" });
  append(root, body);

  let view: SettingsView | null = null;
  let status: Status | null = null;
  let busy = false;
  /** Text typed but not saved yet, kept across re-renders (e.g. after a refused save). */
  const drafts = new Map<string, string>();

  async function load(): Promise<void> {
    try {
      view = await api.settings();
    } catch (e) {
      clear(body);
      append(body, failure(e));
      return;
    }
    render();
  }

  /** Run a change, show its outcome next to the row, and keep focus there. */
  async function change(key: string, action: () => Promise<SettingsView>, done: string): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      view = await action();
      drafts.delete(key);
      status = { key, ok: true, text: done };
    } catch (e) {
      status = { key, ok: false, text: message(e) };
    } finally {
      busy = false;
    }
    render();
    body.querySelector<HTMLElement>(`[data-key="${key}"]`)?.focus();
  }

  function render(): void {
    clear(body);
    if (!view) return;
    const where = view.globalFile
      ? el("p", { class: "muted intro" }, "Saved to your global git config, ", el("code", {}, tilde(view.globalFile)), ".")
      : null;
    append(body, where);

    if (view.settings.length > 0) {
      for (const g of GROUPS) {
        const rows = view.settings.filter((s) => s.group === g.id);
        if (!rows.length) continue;
        append(
          body,
          el(
            "section",
            { class: "group" },
            el("h2", {}, g.title),
            g.note ? el("p", { class: "muted group-note" }, g.note) : null,
            ...rows.map(row),
          ),
        );
      }
    }
    append(body, gitSection(view));
  }

  function rowStatus(key: string): HTMLElement {
    const s = status && status.key === key ? status : null;
    return el(
      "p",
      { class: `status ${s ? (s.ok ? "ok" : "bad") : ""}`, attrs: { "aria-live": "polite" } },
      s ? s.text : "",
    );
  }

  function row(s: SettingValue): HTMLElement {
    const id = `setting-${s.key}`;
    const label = el("label", { attrs: { for: id } }, s.label);
    let control: HTMLElement;

    switch (s.kind.kind) {
      case "bool": {
        const kind = s.kind;
        const on = s.value === null ? kind.default : /^(true|yes|on|1)$/i.test(s.value);
        const box = el("input", {
          type: "checkbox",
          class: "switch",
          attrs: { id, role: "switch", "data-key": s.key, ...(on ? { checked: "" } : {}) },
        });
        box.addEventListener("change", () => {
          const next = box.checked;
          void change(s.key, () => api.setSetting(s.key, String(next)), next ? "Turned on." : "Turned off.");
        });
        control = box;
        break;
      }
      case "choice": {
        const kind = s.kind;
        const current = s.value ?? kind.default;
        const select = el("select", { attrs: { id, "data-key": s.key } });
        for (const c of kind.choices) {
          const label = c.value === kind.default ? `${c.label} (git's default)` : c.label;
          append(select, el("option", { attrs: { value: c.value, ...(c.value === current ? { selected: "" } : {}) } }, label));
        }
        select.addEventListener("change", () => {
          void change(s.key, () => api.setSetting(s.key, select.value), "Saved.");
        });
        control = select;
        break;
      }
      case "text": {
        const kind = s.kind;
        const input = el("input", {
          type: "text",
          class: "mono",
          attrs: {
            id,
            "data-key": s.key,
            value: drafts.get(s.key) ?? s.value ?? "",
            placeholder: kind.placeholder,
            autocomplete: "off",
            autocapitalize: "off",
            spellcheck: "false",
          },
        });
        const save = el("button", { type: "submit" }, "Save");
        const unchanged = () => input.value.trim() === (s.value ?? "");
        save.disabled = unchanged();
        input.addEventListener("input", () => {
          save.disabled = unchanged();
        });
        const form = el("form", { class: "text-row" }, input, save);
        form.addEventListener("submit", (ev) => {
          ev.preventDefault();
          if (save.disabled) return;
          const v = input.value.trim();
          drafts.set(s.key, input.value);
          void change(s.key, () => api.setSetting(s.key, v === "" ? null : v), v === "" ? "Removed." : "Saved.");
        });
        control = form;
        break;
      }
    }

    return el(
      "div",
      { class: `row row-${s.kind.kind}` },
      el("div", { class: "row-text" }, label, el("p", { class: "muted help" }, s.help)),
      el("div", { class: "row-control" }, control, rowStatus(s.key)),
    );
  }

  function gitSection(v: SettingsView): HTMLElement {
    const g = v.git;
    const key = "git-path";
    const summary = g.path
      ? el(
          "p",
          { class: "git-summary" },
          `Using git ${g.version} at `,
          el("code", {}, tilde(g.path)),
          g.chosen ? ", which you chose." : ", found automatically.",
        )
      : el(
          "div",
          { class: "git-summary" },
          el(
            "p",
            { class: "problem-text" },
            g.chosen
              ? "The git you chose doesn't run. Choose another, or let gitid find git automatically."
              : "gitid couldn't find git. Install it from git-scm.com, or enter where it is below.",
          ),
          g.error ? el("p", { class: "mono detail" }, g.error) : null,
        );

    const input = el("input", {
      type: "text",
      class: "mono",
      attrs: {
        "data-key": key,
        "aria-label": "Path to the git program",
        value: drafts.get(key) ?? g.chosen ?? "",
        placeholder: g.path ?? "/usr/local/bin/git",
        autocomplete: "off",
        spellcheck: "false",
      },
    });
    const form = el(
      "form",
      { class: "text-row" },
      input,
      el(
        "button",
        {
          type: "button",
          on: {
            click: async () => {
              try {
                const picked = await api.pickGitExecutable();
                if (picked) input.value = picked;
              } catch (e) {
                status = { key, ok: false, text: message(e) };
                render();
              }
            },
          },
        },
        "Browse",
      ),
      el("button", { type: "submit" }, "Use this git"),
    );
    form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      const p = input.value.trim();
      drafts.set(key, input.value);
      void change(key, () => api.setGitPath(p === "" ? null : p), p === "" ? "Finding git automatically." : "Saved.").then(
        () => {
          if (status?.ok) options.onGitChanged();
        },
      );
    });

    const auto = g.chosen
      ? el(
          "button",
          {
            type: "button",
            class: "link",
            on: {
              click: () =>
                void change(key, () => api.setGitPath(null), "Finding git automatically.").then(() => {
                  if (status?.ok) options.onGitChanged();
                }),
            },
          },
          "Find git automatically instead",
        )
      : null;

    return el(
      "section",
      { class: "group" },
      el("h2", {}, "git"),
      summary,
      el(
        "div",
        { class: "row row-text-wide" },
        el(
          "div",
          { class: "row-text" },
          el("p", { class: "muted help" }, "To use a different git, enter or browse to its program file."),
        ),
        el("div", { class: "row-control" }, form, auto, rowStatus(key)),
      ),
    );
  }

  void load();
}

function message(e: unknown): string {
  return e instanceof AppError ? sentence(e.message) : String(e);
}

function sentence(s: string): string {
  const first = s.split(/\s/, 1)[0];
  const t = first.includes(".") || first.includes("/") ? s : s.charAt(0).toUpperCase() + s.slice(1);
  return /[.!?]$/.test(t) ? t : t + ".";
}

function failure(e: unknown): HTMLElement {
  return el(
    "div",
    { class: "verdict problem", attrs: { role: "alert" } },
    el("p", { class: "lede" }, "Couldn't read your settings."),
    el("p", { class: "mono detail" }, e instanceof Error ? e.message : String(e)),
  );
}

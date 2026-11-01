// The scopes view: every key visible from a folder, each value it has in the
// system, global and repository config, and which value git actually uses.

import { api, AppError } from "../lib/api";
import { append, clear, el } from "../lib/dom";
import { basename, tilde } from "../lib/paths";
import { recentFolders } from "../lib/recent";
import type { AddTo, KeyView, Scope, ScopeValue, ScopesView, ValueRef } from "../lib/types";

const SCOPE_NAME: Record<Scope, string> = {
  system: "System",
  global: "Global",
  local: "Repository",
  worktree: "Worktree",
  command: "Command",
  unknown: "Config",
};

const STATUS_TEXT = { used: "used", overridden: "overridden", alsoUsed: "used" } as const;

interface Editing {
  ref: ValueRef;
  /** Removal waiting for confirmation, instead of an edit. */
  removing: boolean;
}

export function mountScopesView(root: HTMLElement): void {
  const bar = el("section", { class: "folder-bar", attrs: { "aria-label": "Folder" } });
  const body = el("div", { class: "scopes" });
  append(root, bar, body);

  let folder: string | null = recentFolders()[0] ?? null;
  let view: ScopesView | null = null;
  let filter = "";
  let editing: Editing | null = null;
  let adding = false;
  let notice: { ok: boolean; text: string } | null = null;
  let loadError: string | null = null;

  const sameRef = (a: ValueRef | null, b: ValueRef | null) =>
    !!a && !!b && a.file === b.file && a.key === b.key && a.nth === b.nth;

  async function load(): Promise<void> {
    try {
      view = await api.scopes(folder);
      loadError = null;
    } catch (e) {
      view = null;
      loadError = message(e);
    }
    render();
  }

  /** Run a change; show the outcome above the list. */
  async function apply(action: () => Promise<ScopesView>, done: string): Promise<boolean> {
    try {
      view = await action();
      notice = { ok: true, text: done };
      editing = null;
      return true;
    } catch (e) {
      notice = { ok: false, text: message(e) };
      // The value may have changed underneath us: reload so the list is current.
      if (e instanceof AppError && e.kind === "invalidInput" && /changed since/.test(e.message)) {
        editing = null;
        await load();
      }
      return false;
    } finally {
      render();
    }
  }

  function render(): void {
    renderBar();
    clear(body);
    if (loadError) {
      append(body, el("div", { class: "verdict problem", attrs: { role: "alert" } }, el("p", { class: "lede" }, "Couldn't read the config."), el("p", { class: "mono detail" }, loadError)));
      return;
    }
    if (!view) {
      append(body, el("p", { class: "muted" }, "Loading…"));
      return;
    }
    const v = view;

    const filterInput = el("input", {
      type: "text",
      attrs: { value: filter, placeholder: "Filter keys and values", "aria-label": "Filter keys and values", spellcheck: "false" },
    });
    filterInput.addEventListener("input", () => {
      filter = filterInput.value;
      renderList(v);
    });
    append(
      body,
      el(
        "p",
        { class: "muted intro" },
        "git reads the system config, then your global config, then the repository's own. For most keys the last value wins.",
      ),
      el(
        "div",
        { class: "scopes-tools" },
        filterInput,
        el("button", { type: "button", on: { click: () => { adding = !adding; render(); } } }, adding ? "Close" : "Add a setting"),
      ),
    );
    if (adding) append(body, addForm(v));
    append(body, el("p", { class: `status ${notice ? (notice.ok ? "ok" : "bad") : ""}`, attrs: { "aria-live": "polite" } }, notice?.text ?? ""));
    append(body, el("div", { class: "scope-list" }));
    renderList(v);
  }

  function renderList(v: ScopesView): void {
    const list = body.querySelector<HTMLElement>(".scope-list");
    if (!list) return;
    clear(list);
    const q = filter.trim().toLowerCase();
    const keys = v.keys.filter(
      (k) => !q || k.key.toLowerCase().includes(q) || k.values.some((x) => (x.value ?? "").toLowerCase().includes(q)),
    );
    if (keys.length === 0) {
      append(list, el("p", { class: "muted" }, q ? "Nothing matches." : "No git config is set here."));
      return;
    }
    let section = "";
    let group: HTMLElement | null = null;
    for (const k of keys) {
      const s = k.key.split(".")[0];
      if (s !== section || !group) {
        section = s;
        group = el("section", { class: "scope-section" }, el("h2", { class: "mono" }, s));
        append(list, group);
      }
      append(group, keyBlock(k));
    }
  }

  function keyBlock(k: KeyView): HTMLElement {
    return el(
      "div",
      { class: "key-block" },
      el(
        "h3",
        { class: "mono key-name" },
        k.key,
        k.multiValued ? el("span", { class: "muted key-note" }, "a list: every value applies") : null,
      ),
      el("ul", { class: "values" }, ...k.values.map((x) => valueRow(k, x))),
    );
  }

  function valueRow(k: KeyView, x: ScopeValue): HTMLElement {
    const file = x.origin.kind === "file" ? x.origin.value : null;
    const isEditing = editing !== null && sameRef(editing.ref, x.at) && !editing.removing;
    const isRemoving = editing !== null && sameRef(editing.ref, x.at) && editing.removing;

    const where = el(
      "p",
      { class: "muted where" },
      file ? el("span", { class: "mono", title: file }, tilde(file)) : x.origin.kind === "commandLine" ? "Set by git -c or GIT_CONFIG_* variables; can't be changed here." : "",
      x.included ? " (included)" : "",
    );

    let valueCell: HTMLElement;
    if (isEditing && x.at) {
      const at = x.at;
      const input = el("input", { type: "text", class: "mono", attrs: { value: x.value ?? "true", "aria-label": `New value for ${k.key}`, spellcheck: "false" } });
      const form = el(
        "form",
        { class: "text-row" },
        input,
        el("button", { type: "submit" }, "Save"),
        el("button", { type: "button", on: { click: () => { editing = null; render(); } } }, "Cancel"),
      );
      form.addEventListener("submit", (ev) => {
        ev.preventDefault();
        void apply(() => api.editScopeValue(folder, at, input.value), `Saved ${k.key} in ${tilde(at.file)}.`);
      });
      queueMicrotask(() => input.focus());
      valueCell = el("div", { class: "value-cell" }, form, where);
    } else {
      valueCell = el(
        "div",
        { class: "value-cell" },
        el("p", { class: `mono value ${x.status === "overridden" ? "is-overridden" : ""}` }, x.value === null ? "true (no value given)" : x.value),
        where,
      );
    }

    const actions = el("div", { class: "value-actions" });
    if (x.at && !isEditing && !isRemoving) {
      const at = x.at;
      if (!x.masked) {
        append(actions, el("button", { type: "button", class: "link", on: { click: () => { editing = { ref: at, removing: false }; notice = null; render(); } } }, "Edit"));
      }
      append(actions, el("button", { type: "button", class: "link", on: { click: () => { editing = { ref: at, removing: true }; notice = null; render(); } } }, "Remove"));
    }

    const row = el(
      "li",
      { class: `value-row status-${x.status}` },
      el("span", { class: "scope-tag" }, SCOPE_NAME[x.scope]),
      valueCell,
      el("span", { class: "status-text" }, STATUS_TEXT[x.status]),
      actions,
    );
    if (isRemoving && x.at) {
      const at = x.at;
      append(
        row,
        el(
          "div",
          { class: "confirm", attrs: { role: "alert" } },
          el("p", {}, `Remove this value of ${k.key} from ${tilde(at.file)}?`),
          el("button", { type: "button", class: "danger", on: { click: () => void apply(() => api.removeScopeValue(folder, at), `Removed a value of ${k.key} from ${tilde(at.file)}.`) } }, "Remove"),
          el("button", { type: "button", on: { click: () => { editing = null; render(); } } }, "Cancel"),
        ),
      );
      queueMicrotask(() => row.querySelector<HTMLElement>(".confirm .danger")?.focus());
    }
    return row;
  }

  function addForm(v: ScopesView): HTMLElement {
    const scope = el("select", { attrs: { "aria-label": "Where to add it" } });
    const options: [AddTo, string, boolean][] = [
      ["global", "Your global config", true],
      ["local", v.repo ? "This repository" : "This repository (choose a folder in a repo)", v.repo !== null],
      ["system", "System config (needs admin rights)", true],
    ];
    for (const [value, label, ok] of options) {
      append(scope, el("option", { attrs: { value, ...(ok ? {} : { disabled: "" }) } }, label));
    }
    const key = el("input", { type: "text", class: "mono", attrs: { placeholder: "section.key, e.g. core.autocrlf", "aria-label": "Key", spellcheck: "false", autocapitalize: "off" } });
    const value = el("input", { type: "text", class: "mono", attrs: { placeholder: "value", "aria-label": "Value", spellcheck: "false", autocapitalize: "off" } });
    const form = el("form", { class: "add-form" }, scope, key, value, el("button", { type: "submit", class: "primary" }, "Add"));
    form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      const k = key.value.trim();
      if (!k) {
        notice = { ok: false, text: "Enter a key, like core.autocrlf." };
        render();
        return;
      }
      const where = scope.value as AddTo;
      void apply(() => api.addScopeValue(folder, where, k, value.value), `Set ${k}.`).then((ok) => {
        if (ok) {
          adding = false;
          filter = k;
          render();
        }
      });
    });
    queueMicrotask(() => key.focus());
    return form;
  }

  function renderBar(): void {
    clear(bar);
    const row = el(
      "div",
      { class: "bar-row" },
      el(
        "button",
        {
          type: "button",
          class: "primary",
          on: {
            click: async () => {
              const picked = await api.pickFolder().catch(() => null);
              if (picked) {
                folder = picked;
                editing = null;
                notice = null;
                await load();
              }
            },
          },
        },
        "Choose a folder",
      ),
      el(
        "span",
        { class: "current mono", title: folder ?? "" },
        folder ? tilde(folder) : "No folder: system and global config only",
      ),
    );
    if (folder) {
      append(
        row,
        el(
          "button",
          {
            type: "button",
            on: {
              click: () => {
                folder = null;
                editing = null;
                notice = null;
                void load();
              },
            },
          },
          "Without a folder",
        ),
      );
    }
    append(bar, row);
    const others = recentFolders().filter((p) => p !== folder).slice(0, 5);
    if (others.length) {
      const list = el("ul", { class: "recent", attrs: { "aria-label": "Recent folders" } });
      for (const p of others) {
        append(
          list,
          el(
            "li",
            {},
            el(
              "button",
              {
                type: "button",
                class: "chip",
                title: tilde(p),
                on: {
                  click: () => {
                    folder = p;
                    editing = null;
                    notice = null;
                    void load();
                  },
                },
              },
              basename(p),
            ),
          ),
        );
      }
      append(bar, el("div", { class: "recent-row" }, el("span", { class: "muted" }, "Recent:"), list));
    }
  }

  void load();
}

function message(e: unknown): string {
  const text = e instanceof AppError ? e.message : String(e);
  const first = text.split(/\s/, 1)[0];
  const t = /[./"`]/.test(first) ? text : text.charAt(0).toUpperCase() + text.slice(1);
  return /[.!?]$/.test(t) ? t : t + ".";
}

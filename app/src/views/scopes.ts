// The scopes view: every key visible from a folder, each value it has in the
// system, global and repository config, and which value git actually uses.

import { api, AppError } from "../lib/api";
import { type MessageKey, t } from "../lib/i18n";
import { errorDetail, errorText } from "../lib/messages";
import { append, clear, el } from "../lib/dom";
import { basename, tilde } from "../lib/paths";
import { recentFolders } from "../lib/recent";
import type { AddTo, KeyView, Scope, ScopeValue, ScopesView, ValueRef } from "../lib/types";

const SCOPE_NAME: Record<Scope, MessageKey> = {
  system: "scopes.scope.system",
  global: "scopes.scope.global",
  local: "scopes.scope.local",
  worktree: "scopes.scope.worktree",
  command: "scopes.scope.command",
  unknown: "scopes.scope.unknown",
};

const STATUS_TEXT: Record<ScopeValue["status"], MessageKey> = {
  used: "scopes.status.used",
  overridden: "scopes.status.overridden",
  alsoUsed: "scopes.status.used",
};

interface Editing {
  ref: ValueRef;
  /** Removal waiting for confirmation, instead of an edit. */
  removing: boolean;
}

export function mountScopesView(root: HTMLElement): void {
  const bar = el("section", { class: "folder-bar", attrs: { "aria-label": t("common.folder") } });
  const body = el("div", { class: "scopes" });
  append(root, bar, body);

  let folder: string | null = recentFolders()[0] ?? null;
  let view: ScopesView | null = null;
  let filter = "";
  let editing: Editing | null = null;
  let adding = false;
  let notice: { ok: boolean; text: string } | null = null;
  let loadError: unknown = null;

  const sameRef = (a: ValueRef | null, b: ValueRef | null) =>
    !!a && !!b && a.file === b.file && a.key === b.key && a.nth === b.nth;

  async function load(): Promise<void> {
    try {
      view = await api.scopes(folder);
      loadError = null;
    } catch (e) {
      view = null;
      loadError = e;
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
      if (e instanceof AppError && e.code === "staleValue") {
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
      const detail = errorDetail(loadError);
      append(
        body,
        el(
          "div",
          { class: "verdict problem", attrs: { role: "alert" } },
          el("p", { class: "lede" }, t("scopes.loadFailed")),
          el("p", {}, errorText(loadError)),
          detail ? el("p", { class: "mono detail" }, detail) : null,
        ),
      );
      return;
    }
    if (!view) {
      append(body, el("p", { class: "muted" }, t("common.loading")));
      return;
    }
    const v = view;

    const filterInput = el("input", {
      type: "text",
      attrs: { value: filter, placeholder: t("scopes.filter"), "aria-label": t("scopes.filter"), spellcheck: "false" },
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
        t("scopes.intro"),
      ),
      el(
        "div",
        { class: "scopes-tools" },
        filterInput,
        el("button", { type: "button", on: { click: () => { adding = !adding; render(); } } }, t(adding ? "scopes.close" : "scopes.add")),
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
      append(list, el("p", { class: "muted" }, t(q ? "scopes.noMatch" : "scopes.nothingSet")));
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
        k.multiValued ? el("span", { class: "muted key-note" }, t("scopes.multiValued")) : null,
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
      file ? el("span", { class: "mono", title: file }, tilde(file)) : x.origin.kind === "commandLine" ? t("scopes.fromCommand") : "",
      x.included ? t("scopes.included") : "",
    );

    let valueCell: HTMLElement;
    if (isEditing && x.at) {
      const at = x.at;
      const input = el("input", { type: "text", class: "mono", attrs: { value: x.value ?? "true", "aria-label": t("scopes.newValue", { key: k.key }), spellcheck: "false" } });
      const form = el(
        "form",
        { class: "text-row" },
        input,
        el("button", { type: "submit" }, t("common.save")),
        el("button", { type: "button", on: { click: () => { editing = null; render(); } } }, t("common.cancel")),
      );
      form.addEventListener("submit", (ev) => {
        ev.preventDefault();
        void apply(() => api.editScopeValue(folder, at, input.value), t("scopes.savedIn", { key: k.key, file: tilde(at.file) }));
      });
      queueMicrotask(() => input.focus());
      valueCell = el("div", { class: "value-cell" }, form, where);
    } else {
      valueCell = el(
        "div",
        { class: "value-cell" },
        el("p", { class: `mono value ${x.status === "overridden" ? "is-overridden" : ""}` }, x.value === null ? t("scopes.noValue") : x.value),
        where,
      );
    }

    const actions = el("div", { class: "value-actions" });
    if (x.at && !isEditing && !isRemoving) {
      const at = x.at;
      if (!x.masked) {
        append(actions, el("button", { type: "button", class: "link", on: { click: () => { editing = { ref: at, removing: false }; notice = null; render(); } } }, t("common.edit")));
      }
      append(actions, el("button", { type: "button", class: "link", on: { click: () => { editing = { ref: at, removing: true }; notice = null; render(); } } }, t("common.remove")));
    }

    const row = el(
      "li",
      { class: `value-row status-${x.status}` },
      el("span", { class: "scope-tag" }, t(SCOPE_NAME[x.scope])),
      valueCell,
      el("span", { class: "status-text" }, t(STATUS_TEXT[x.status])),
      actions,
    );
    if (isRemoving && x.at) {
      const at = x.at;
      append(
        row,
        el(
          "div",
          { class: "confirm", attrs: { role: "alert" } },
          el("p", {}, t("scopes.confirmRemove", { key: k.key, file: tilde(at.file) })),
          el(
            "button",
            {
              type: "button",
              class: "danger",
              on: {
                click: () =>
                  void apply(
                    () => api.removeScopeValue(folder, at),
                    t("scopes.removedFrom", { key: k.key, file: tilde(at.file) }),
                  ),
              },
            },
            t("common.remove"),
          ),
          el("button", { type: "button", on: { click: () => { editing = null; render(); } } }, t("common.cancel")),
        ),
      );
      queueMicrotask(() => row.querySelector<HTMLElement>(".confirm .danger")?.focus());
    }
    return row;
  }

  function addForm(v: ScopesView): HTMLElement {
    const scope = el("select", { attrs: { "aria-label": t("scopes.addTo.label") } });
    const options: [AddTo, string, boolean][] = [
      ["global", t("scopes.addTo.global"), true],
      ["local", t(v.repo ? "scopes.addTo.local" : "scopes.addTo.localNoRepo"), v.repo !== null],
      ["system", t("scopes.addTo.system"), true],
    ];
    for (const [value, label, ok] of options) {
      append(scope, el("option", { attrs: { value, ...(ok ? {} : { disabled: "" }) } }, label));
    }
    const key = el("input", { type: "text", class: "mono", attrs: { placeholder: t("scopes.addKey.placeholder"), "aria-label": t("scopes.addKey.label"), spellcheck: "false", autocapitalize: "off" } });
    const value = el("input", { type: "text", class: "mono", attrs: { placeholder: t("scopes.addValue.placeholder"), "aria-label": t("scopes.addValue.label"), spellcheck: "false", autocapitalize: "off" } });
    const form = el("form", { class: "add-form" }, scope, key, value, el("button", { type: "submit", class: "primary" }, t("scopes.addButton")));
    form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      const k = key.value.trim();
      if (!k) {
        notice = { ok: false, text: t("scopes.needKey") };
        render();
        return;
      }
      const where = scope.value as AddTo;
      void apply(() => api.addScopeValue(folder, where, k, value.value), t("scopes.set", { key: k })).then((ok) => {
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
              const picked = await api.pickFolder(t("dialog.chooseFolder")).catch(() => null);
              if (picked) {
                folder = picked;
                editing = null;
                notice = null;
                await load();
              }
            },
          },
        },
        t("common.chooseFolder"),
      ),
      el(
        "span",
        { class: "current mono", title: folder ?? "" },
        folder ? tilde(folder) : t("scopes.noFolder"),
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
          t("scopes.withoutFolder"),
        ),
      );
    }
    append(bar, row);
    const others = recentFolders().filter((p) => p !== folder).slice(0, 5);
    if (others.length) {
      const list = el("ul", { class: "recent", attrs: { "aria-label": t("common.recentFolders") } });
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
      append(bar, el("div", { class: "recent-row" }, el("span", { class: "muted" }, t("common.recent")), list));
    }
  }

  void load();
}

function message(e: unknown): string {
  return errorText(e);
}

// The rules view: identities that apply in specific folders or for specific
// servers. List, add, edit, remove, and check where a rule applies.

import { api } from "../lib/api";
import { type MessageKey, formatNumber, t, tx } from "../lib/i18n";
import { errorDetail, errorText, ruleLine } from "../lib/messages";
import { append, clear, el } from "../lib/dom";
import { tilde } from "../lib/paths";
import type { RuleCheck, RuleView, RulesView, SaveRuleResult, When } from "../lib/types";

interface Form {
  /** The rule being edited, or null when adding. */
  editing: RuleView | null;
  id: string;
  name: string;
  email: string;
  when: When[];
  error: string | null;
  saving: boolean;
}

export function mountRulesView(root: HTMLElement): void {
  const body = el("div", { class: "rules" });
  append(root, body);

  let data: RulesView | null = null;
  let form: Form | null = null;
  /** Shown above the list after a save. */
  let saved: SaveRuleResult | null = null;
  /** Rule id waiting for "really remove?" */
  let confirming: string | null = null;
  /** Per-rule result of "Check a folder". */
  const checks = new Map<string, RuleCheck | string>();
  let loadError: unknown = null;

  async function load(): Promise<void> {
    try {
      data = await api.rules();
      loadError = null;
    } catch (e) {
      loadError = e;
    }
    render();
  }

  function render(): void {
    clear(body);
    if (loadError) {
      const detail = errorDetail(loadError);
      append(
        body,
        el(
          "div",
          { class: "verdict problem", attrs: { role: "alert" } },
          el("p", { class: "lede" }, t("rules.loadFailed")),
          el("p", {}, errorText(loadError)),
          detail ? el("p", { class: "mono detail" }, detail) : null,
        ),
      );
      return;
    }
    if (!data) {
      append(body, el("p", { class: "muted" }, t("common.loading")));
      return;
    }
    if (form) {
      append(body, formPanel(form, data));
      return;
    }
    append(body, listPanel(data));
  }

  // ---------------------------------------------------------------- list

  function listPanel(d: RulesView): HTMLElement {
    const panel = el("div", {});
    append(
      panel,
      el(
        "div",
        { class: "rules-head" },
        el("p", { class: "muted intro" }, t("rules.intro")),
        el("button", { type: "button", class: "primary", on: { click: () => openForm(null) } }, t("rules.add")),
      ),
    );
    if (saved) append(panel, savedBanner(saved));

    if (d.rules.length === 0) {
      append(
        panel,
        el(
          "div",
          { class: "empty" },
          el("p", { class: "lede" }, t("rules.empty.lede")),
          el("p", { class: "muted" }, tx("rules.empty.body", { folder: el("code", {}, "~/work") })),
        ),
      );
      return panel;
    }
    const list = el("ul", { class: "rule-list" });
    for (const r of d.rules) append(list, ruleItem(r));
    append(panel, list);
    return panel;
  }

  function savedBanner(s: SaveRuleResult): HTMLElement {
    const misses = s.checks.filter((c) => !c.applies);
    const box = el(
      "div",
      // Saving worked either way; folders it doesn't cover are a note, not a failure.
      { class: `verdict ${misses.length ? "note" : "ok"}`, attrs: { role: "status" } },
      el("p", { class: "lede" }, t("rules.saved", { id: s.rule.id })),
    );
    if (s.checks.length) {
      const ul = el("ul", { class: "check-list" });
      for (const c of s.checks) {
        append(ul, el("li", { class: c.applies ? "ok-text" : "problem-text" }, appliesText(c.applies, c.path)));
      }
      append(box, ul);
      if (misses.length) {
        append(box, el("p", { class: "muted" }, t("rules.saved.misses")));
      }
    } else {
      append(box, el("p", { class: "muted" }, t("rules.saved.tryCheck")));
    }
    return box;
  }

  function ruleItem(r: RuleView): HTMLElement {
    const who =
      r.name && r.email
        ? `${r.name} <${r.email}>`
        : r.email
          ? `<${r.email}>`
          : r.name
            ? r.name
            : t(r.fileExists ? "rules.who.none" : "rules.who.fileMissing");
    const whoNote = r.name && !r.email ? t("rules.who.nameOnly") : !r.name && r.email ? t("rules.who.emailOnly") : "";

    const actions = el("div", { class: "rule-actions" });
    if (r.managed) {
      append(actions, el("button", { type: "button", on: { click: () => openForm(r) } }, t("common.edit")));
    }
    append(
      actions,
      el(
        "button",
        {
          type: "button",
          on: {
            click: () => {
              confirming = r.id;
              render();
              body.querySelector<HTMLElement>(".confirm button")?.focus();
            },
          },
        },
        t("common.remove"),
      ),
    );

    const item = el(
      "li",
      { class: "rule" },
      el(
        "div",
        { class: "rule-top" },
        el("h2", { class: "rule-id mono" }, r.id),
        el("p", { class: "rule-who mono" }, who, el("span", { class: "muted" }, whoNote)),
        actions,
      ),
      el("ul", { class: "rule-when" }, ...r.lines.map((line) => el("li", {}, ruleLine(line)))),
      el(
        "p",
        { class: "muted rule-file" },
        r.managed ? "" : `${t("rules.handWritten")} `,
        el("span", { class: "mono", title: r.file }, tilde(r.file)),
      ),
    );

    if (confirming === r.id) {
      append(
        item,
        el(
          "div",
          { class: "confirm", attrs: { role: "alert" } },
          el("p", {}, t(r.managed ? "rules.confirm.managed" : "rules.confirm.handWritten", { id: r.id })),
          el("button", { type: "button", class: "danger", on: { click: () => void remove(r) } }, t("rules.confirm.remove")),
          el(
            "button",
            {
              type: "button",
              on: {
                click: () => {
                  confirming = null;
                  render();
                },
              },
            },
            t("common.cancel"),
          ),
        ),
      );
    }

    const check = checks.get(r.id);
    const checkButton = el("button", { type: "button", class: "link", on: { click: () => void checkFolder(r) } }, t("rules.check"));
    append(
      item,
      el(
        "div",
        { class: "rule-check" },
        checkButton,
        check === undefined
          ? null
          : typeof check === "string"
            ? el("span", { class: "problem-text" }, check)
            : el("span", { class: check.applies ? "ok-text" : "problem-text" }, appliesText(check.applies, check.path)),
      ),
    );
    return item;
  }

  async function remove(r: RuleView): Promise<void> {
    try {
      data = await api.removeRule(r.id, r.managed);
      saved = null;
      checks.delete(r.id);
    } catch (e) {
      checks.set(r.id, message(e));
    }
    confirming = null;
    render();
  }

  async function checkFolder(r: RuleView): Promise<void> {
    try {
      const folder = await api.pickFolder(t("dialog.chooseFolder"));
      if (!folder) return;
      checks.set(r.id, await api.checkRule(r.id, folder));
    } catch (e) {
      checks.set(r.id, message(e));
    }
    render();
  }

  // ---------------------------------------------------------------- form

  function openForm(r: RuleView | null): void {
    form = {
      editing: r,
      id: r?.id ?? "",
      name: r?.name ?? "",
      email: r?.email ?? "",
      when: r ? r.when.map((w) => (w.kind === "folder" ? { ...w, path: tilde(w.path) } : { ...w })) : [{ kind: "folder", path: "" }],
      error: null,
      saving: false,
    };
    saved = null;
    render();
    body.querySelector<HTMLElement>("input:not([readonly])")?.focus();
  }

  function formPanel(f: Form, d: RulesView): HTMLElement {
    const field = (label: MessageKey, help: MessageKey, input: HTMLInputElement) =>
      el(
        "div",
        { class: "row" },
        el("div", { class: "row-text" }, el("label", { attrs: { for: input.id } }, t(label)), el("p", { class: "muted help" }, t(help))),
        el("div", { class: "row-control" }, input),
      );
    const text = (id: string, value: string, placeholder: string, onInput: (v: string) => void, extra: Record<string, string> = {}) => {
      const input = el("input", {
        type: "text",
        class: "mono",
        attrs: { id, value, placeholder, autocomplete: "off", autocapitalize: "off", spellcheck: "false", ...extra },
      });
      input.addEventListener("input", () => onInput(input.value));
      return input;
    };

    const formEl = el("form", { class: "rule-form" });
    append(
      formEl,
      el("h2", { class: "form-title" }, f.editing ? t("rules.form.editTitle", { id: f.editing.id }) : t("rules.form.addTitle")),
      field(
        "rules.form.id",
        f.editing ? "rules.form.id.helpEditing" : "rules.form.id.help",
        text("rule-id", f.id, "work", (v) => (f.id = v), f.editing ? { readonly: "" } : {}),
      ),
      field("rules.form.name", "rules.form.name.help", text("rule-name", f.name, t("rules.form.name.placeholder"), (v) => (f.name = v))),
      field("rules.form.email", "rules.form.email.help", text("rule-email", f.email, "you@company.example", (v) => (f.email = v))),
    );

    const conds = el("div", { class: "conditions" });
    f.when.forEach((w, i) => append(conds, whenRow(f, w, i, d.supportsHasconfig)));
    append(
      formEl,
      el(
        "section",
        { class: "group" },
        el("h2", {}, t("rules.form.appliesTo")),
        el("p", { class: "muted group-note" }, t("rules.form.appliesTo.note")),
        conds,
        el(
          "button",
          {
            type: "button",
            class: "link add-condition",
            on: {
              click: () => {
                f.when.push({ kind: "folder", path: "" });
                render();
                const inputs = body.querySelectorAll<HTMLElement>(".condition input");
                inputs[inputs.length - 1]?.focus();
              },
            },
          },
          t("rules.form.addLine"),
        ),
      ),
    );

    append(
      formEl,
      el(
        "div",
        { class: "form-actions" },
        el("button", { type: "submit", class: "primary", attrs: f.saving ? { disabled: "" } : {} }, t(f.saving ? "rules.form.saving" : "rules.form.save")),
        el(
          "button",
          {
            type: "button",
            on: {
              click: () => {
                form = null;
                render();
              },
            },
          },
          t("common.cancel"),
        ),
        f.error ? el("p", { class: "status bad", attrs: { role: "alert" } }, f.error) : null,
      ),
    );

    formEl.addEventListener("submit", (ev) => {
      ev.preventDefault();
      void save(f);
    });
    return formEl;
  }

  function whenRow(f: Form, w: When, i: number, serversOk: boolean): HTMLElement {
    const row = el("div", { class: "condition" });
    const removeButton =
      f.when.length > 1
        ? el(
            "button",
            {
              type: "button",
              class: "link",
              attrs: { "aria-label": t("rules.line.remove", { n: formatNumber(i + 1) }) },
              on: {
                click: () => {
                  f.when.splice(i, 1);
                  render();
                },
              },
            },
            t("common.remove"),
          )
        : null;

    if (w.kind === "raw") {
      append(row, el("p", { class: "raw" }, tx("rules.line.raw", { condition: el("code", {}, w.raw) })), removeButton);
      return row;
    }

    const n = formatNumber(i + 1);
    const kinds: [When["kind"], string][] = [
      ["folder", t("rules.line.kind.folder")],
      ["remoteHost", t("rules.line.kind.remoteHost")],
      ["remotePattern", t("rules.line.kind.remotePattern")],
    ];
    const select = el("select", { attrs: { "aria-label": t("rules.line.kindLabel", { n }) } });
    for (const [k, label] of kinds) {
      const disabled = k !== "folder" && !serversOk;
      append(
        select,
        el(
          "option",
          { attrs: { value: k, ...(k === w.kind ? { selected: "" } : {}), ...(disabled ? { disabled: "" } : {}) } },
          disabled ? t("rules.line.kind.needsGit", { kind: label }) : label,
        ),
      );
    }
    select.addEventListener("change", () => {
      const k = select.value as When["kind"];
      f.when[i] =
        k === "folder"
          ? { kind: "folder", path: "" }
          : k === "remoteHost"
            ? { kind: "remoteHost", host: "", sshPort: null }
            : { kind: "remotePattern", pattern: "" };
      render();
      body.querySelectorAll<HTMLElement>(".condition")[i]?.querySelector<HTMLElement>("input")?.focus();
    });

    const input = (value: string, placeholder: string, label: string, onInput: (v: string) => void) => {
      const inp = el("input", {
        type: "text",
        class: "mono",
        attrs: { value, placeholder, "aria-label": label, autocomplete: "off", spellcheck: "false" },
      });
      inp.addEventListener("input", () => onInput(inp.value.trim()));
      return inp;
    };

    append(row, select);
    if (w.kind === "folder") {
      const inp = input(w.path, "~/work", t("rules.line.folderLabel", { n }), (v) => (w.path = v));
      const browse = el(
        "button",
        {
          type: "button",
          on: {
            click: async () => {
              const picked = await api.pickFolder(t("dialog.chooseFolder")).catch(() => null);
              if (picked) {
                w.path = tilde(picked);
                inp.value = w.path;
              }
            },
          },
        },
        t("common.browse"),
      );
      append(row, inp, browse);
    } else if (w.kind === "remoteHost") {
      const port = input(w.sshPort === null ? "" : String(w.sshPort), "22", t("rules.line.portLabel", { n }), (v) => {
        const n = Number(v);
        w.sshPort = v === "" || !Number.isInteger(n) ? null : n;
      });
      port.classList.add("port");
      append(
        row,
        input(w.host, "gitea.example.com", t("rules.line.serverLabel", { n }), (v) => (w.host = v)),
        el("label", { class: "muted port-label" }, t("rules.line.sshPort"), port),
      );
    } else {
      append(row, input(w.pattern, "https://example.com/team/**", t("rules.line.patternLabel", { n }), (v) => (w.pattern = v)));
    }
    append(row, removeButton);
    return row;
  }

  async function save(f: Form): Promise<void> {
    const when = f.when.filter((w) =>
      w.kind === "folder" ? w.path !== "" : w.kind === "remoteHost" ? w.host !== "" : w.kind === "remotePattern" ? w.pattern !== "" : true,
    );
    if (when.length === 0) {
      f.error = t("rules.form.needsWhere");
      render();
      return;
    }
    f.saving = true;
    f.error = null;
    render();
    try {
      saved = await api.saveRule({
        id: f.id.trim(),
        name: f.name.trim() || null,
        email: f.email.trim() || null,
        when,
        replace: f.editing !== null,
      });
      form = null;
      data = await api.rules();
    } catch (e) {
      f.error = message(e);
    } finally {
      f.saving = false;
    }
    render();
  }

  void load();
}

function message(e: unknown): string {
  return errorText(e);
}

/** "Applies in ~/code/blog" or "Doesn't apply in ...", with the path in code style. */
function appliesText(applies: boolean, path: string): DocumentFragment {
  return tx(applies ? "rules.appliesIn" : "rules.notAppliesIn", { path: el("span", { class: "mono" }, tilde(path)) });
}

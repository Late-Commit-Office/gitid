// The rules view: identities that apply in specific folders or for specific
// servers. List, add, edit, remove, and check where a rule applies.

import { api, AppError } from "../lib/api";
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
  let loadError: string | null = null;

  async function load(): Promise<void> {
    try {
      data = await api.rules();
      loadError = null;
    } catch (e) {
      loadError = message(e);
    }
    render();
  }

  function render(): void {
    clear(body);
    if (loadError) {
      append(body, el("div", { class: "verdict problem", attrs: { role: "alert" } }, el("p", { class: "lede" }, "Couldn't read your rules."), el("p", { class: "mono detail" }, loadError)));
      return;
    }
    if (!data) {
      append(body, el("p", { class: "muted" }, "Loading…"));
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
        el("p", { class: "muted intro" }, "A rule gives the repos in a folder, or the repos on a server, their own name and email."),
        el("button", { type: "button", class: "primary", on: { click: () => openForm(null) } }, "Add a rule"),
      ),
    );
    if (saved) append(panel, savedBanner(saved));

    if (d.rules.length === 0) {
      append(
        panel,
        el(
          "div",
          { class: "empty" },
          el("p", { class: "lede" }, "No rules yet."),
          el("p", { class: "muted" }, "For example: everything under ~/work commits with your work email, and every repo on your Gitea server with your Gitea account."),
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
      el("p", { class: "lede" }, `Saved the rule “${s.rule.id}”.`),
    );
    if (s.checks.length) {
      const ul = el("ul", { class: "check-list" });
      for (const c of s.checks) {
        append(ul, el("li", { class: c.applies ? "ok-text" : "problem-text" }, c.applies ? "Applies in " : "Doesn't apply in ", el("span", { class: "mono" }, tilde(c.path))));
      }
      append(box, ul);
      if (misses.length) {
        append(box, el("p", { class: "muted" }, "Open those folders in Identity to see what they use instead."));
      }
    } else {
      append(box, el("p", { class: "muted" }, "Use “Check a folder” below to see it working in a repository."));
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
            : r.fileExists
              ? "Sets no name or email"
              : "Its identity file is missing";
    const whoNote = r.name && !r.email ? " (name only)" : !r.name && r.email ? " (email only)" : "";

    const actions = el("div", { class: "rule-actions" });
    if (r.managed) {
      append(actions, el("button", { type: "button", on: { click: () => openForm(r) } }, "Edit"));
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
        "Remove",
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
      el("ul", { class: "rule-when" }, ...r.description.map((line) => el("li", {}, readable(line)))),
      el(
        "p",
        { class: "muted rule-file" },
        r.managed ? "" : "Written by hand, so gitid can remove it but not edit it. ",
        el("span", { class: "mono", title: r.file }, tilde(r.file)),
      ),
    );

    if (confirming === r.id) {
      append(
        item,
        el(
          "div",
          { class: "confirm", attrs: { role: "alert" } },
          el(
            "p",
            {},
            `Remove the rule “${r.id}”?`,
            r.managed ? " Its identity file is deleted too." : " Its file stays where it is.",
          ),
          el("button", { type: "button", class: "danger", on: { click: () => void remove(r) } }, "Remove rule"),
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
            "Cancel",
          ),
        ),
      );
    }

    const check = checks.get(r.id);
    const checkButton = el("button", { type: "button", class: "link", on: { click: () => void checkFolder(r) } }, "Check a folder");
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
            : el(
                "span",
                { class: check.applies ? "ok-text" : "problem-text" },
                check.applies ? "Applies in " : "Doesn't apply in ",
                el("span", { class: "mono" }, tilde(check.path)),
              ),
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
      const folder = await api.pickFolder();
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
    const field = (label: string, help: string, input: HTMLInputElement) =>
      el(
        "div",
        { class: "row" },
        el("div", { class: "row-text" }, el("label", { attrs: { for: input.id } }, label), el("p", { class: "muted help" }, help)),
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
      el("h2", { class: "form-title" }, f.editing ? `Edit the rule “${f.editing.id}”` : "Add a rule"),
      field(
        "Rule name",
        f.editing ? "Can't be changed. It is also the name of the rule's identity file." : "Letters, digits, - and _, like work or gitea. Also names the rule's identity file.",
        text("rule-id", f.id, "work", (v) => (f.id = v), f.editing ? { readonly: "" } : {}),
      ),
      field("Name", "Leave empty to keep your default name in these repos.", text("rule-name", f.name, "Your Name", (v) => (f.name = v))),
      field("Email", "Leave empty to keep your default email in these repos.", text("rule-email", f.email, "you@company.example", (v) => (f.email = v))),
    );

    const conds = el("div", { class: "conditions" });
    f.when.forEach((w, i) => append(conds, whenRow(f, w, i, d.supportsHasconfig)));
    append(
      formEl,
      el(
        "section",
        { class: "group" },
        el("h2", {}, "Applies to"),
        el("p", { class: "muted group-note" }, "The rule applies to repos that match any of these."),
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
          "Add another folder or server",
        ),
      ),
    );

    append(
      formEl,
      el(
        "div",
        { class: "form-actions" },
        el("button", { type: "submit", class: "primary", attrs: f.saving ? { disabled: "" } : {} }, f.saving ? "Saving…" : "Save rule"),
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
          "Cancel",
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
              attrs: { "aria-label": `Remove line ${i + 1}` },
              on: {
                click: () => {
                  f.when.splice(i, 1);
                  render();
                },
              },
            },
            "Remove",
          )
        : null;

    if (w.kind === "raw") {
      append(row, el("p", { class: "raw" }, "Kept as written: ", el("code", {}, w.raw)), removeButton);
      return row;
    }

    const kinds: [When["kind"], string][] = [
      ["folder", "Repos in a folder"],
      ["remoteHost", "Repos on a server"],
      ["remotePattern", "Remote URL pattern"],
    ];
    const select = el("select", { attrs: { "aria-label": `Line ${i + 1}: kind` } });
    for (const [k, label] of kinds) {
      const disabled = k !== "folder" && !serversOk;
      append(
        select,
        el(
          "option",
          { attrs: { value: k, ...(k === w.kind ? { selected: "" } : {}), ...(disabled ? { disabled: "" } : {}) } },
          disabled ? `${label} (needs git 2.36)` : label,
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
      const inp = input(w.path, "~/work", `Line ${i + 1}: folder`, (v) => (w.path = v));
      const browse = el(
        "button",
        {
          type: "button",
          on: {
            click: async () => {
              const picked = await api.pickFolder().catch(() => null);
              if (picked) {
                w.path = tilde(picked);
                inp.value = w.path;
              }
            },
          },
        },
        "Browse",
      );
      append(row, inp, browse);
    } else if (w.kind === "remoteHost") {
      const port = input(w.sshPort === null ? "" : String(w.sshPort), "22", `Line ${i + 1}: SSH port`, (v) => {
        const n = Number(v);
        w.sshPort = v === "" || !Number.isInteger(n) ? null : n;
      });
      port.classList.add("port");
      append(
        row,
        input(w.host, "gitea.example.com", `Line ${i + 1}: server`, (v) => (w.host = v)),
        el("label", { class: "muted port-label" }, "SSH port", port),
      );
    } else {
      append(row, input(w.pattern, "https://example.com/team/**", `Line ${i + 1}: remote URL pattern`, (v) => (w.pattern = v)));
    }
    append(row, removeButton);
    return row;
  }

  async function save(f: Form): Promise<void> {
    const when = f.when.filter((w) =>
      w.kind === "folder" ? w.path !== "" : w.kind === "remoteHost" ? w.host !== "" : w.kind === "remotePattern" ? w.pattern !== "" : true,
    );
    if (when.length === 0) {
      f.error = "Add at least one folder or server.";
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
  const text = e instanceof AppError ? e.message : String(e);
  const first = text.split(/\s/, 1)[0];
  const t = first.includes(".") || first.includes("/") || first.startsWith('"') ? text : text.charAt(0).toUpperCase() + text.slice(1);
  return /[.!?]$/.test(t) ? t : t + ".";
}

/** Capitalize gitid's own descriptions; leave raw conditions (onbranch:...) as written. */
function readable(s: string): string {
  return /^(repos|remote) /.test(s) ? s.charAt(0).toUpperCase() + s.slice(1) : s;
}

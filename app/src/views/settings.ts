// The settings view: your default identity, the "never guess" guard, a few
// common defaults, commit signing, and which git the app uses.

import { api, AppError } from "../lib/api";
import { append, clear, el } from "../lib/dom";
import {
  type LanguageChoice,
  type MessageKey,
  LOCALE_NAMES,
  LOCALES,
  PSEUDO,
  PSEUDO_AVAILABLE,
  PSEUDO_NAME,
  resolveLocale,
  t,
  tx,
} from "../lib/i18n";
import { choiceLabel, errorDetail, errorText, settingHelp, settingLabel, settingPlaceholder } from "../lib/messages";
import { tilde } from "../lib/paths";
import type { CommandError, SettingGroup, SettingValue, SettingsView } from "../lib/types";

const GROUPS: { id: SettingGroup; heading: MessageKey; note?: MessageKey }[] = [
  { id: "identity", heading: "settings.group.identity", note: "settings.group.identity.note" },
  { id: "defaults", heading: "settings.group.defaults" },
  { id: "signing", heading: "settings.group.signing" },
];

interface Status {
  key: string;
  ok: boolean;
  text: string;
}

export interface SettingsOptions {
  /** Called after the app starts using a different git. */
  onGitChanged: () => void;
  /** The language chosen in Settings. */
  language: LanguageChoice;
  /** Called after a new language is saved; the app redraws in it. */
  onLanguageChanged: (language: LanguageChoice) => void;
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
      ? el("p", { class: "muted intro" }, tx("settings.savedTo", { file: el("code", {}, tilde(view.globalFile)) }))
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
            el("h2", {}, t(g.heading)),
            g.note ? el("p", { class: "muted group-note" }, t(g.note)) : null,
            ...rows.map(row),
          ),
        );
      }
    }
    append(body, gitSection(view), languageSection());
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
    const label = el("label", { attrs: { for: id } }, settingLabel(s.key));
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
          void change(s.key, () => api.setSetting(s.key, String(next)), t(next ? "settings.turnedOn" : "settings.turnedOff"));
        });
        control = box;
        break;
      }
      case "choice": {
        const kind = s.kind;
        const current = s.value ?? kind.default;
        const select = el("select", { attrs: { id, "data-key": s.key } });
        for (const c of kind.choices) {
          const name = choiceLabel(s.key, c.value);
          const label = c.value === kind.default ? t("settings.gitsDefault", { choice: name }) : name;
          append(select, el("option", { attrs: { value: c.value, ...(c.value === current ? { selected: "" } : {}) } }, label));
        }
        select.addEventListener("change", () => {
          void change(s.key, () => api.setSetting(s.key, select.value), t("common.saved"));
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
            placeholder: settingPlaceholder(s.key, kind.placeholder),
            autocomplete: "off",
            autocapitalize: "off",
            spellcheck: "false",
          },
        });
        const save = el("button", { type: "submit" }, t("common.save"));
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
          void change(s.key, () => api.setSetting(s.key, v === "" ? null : v), t(v === "" ? "common.removed" : "common.saved"));
        });
        control = form;
        break;
      }
    }

    return el(
      "div",
      { class: `row row-${s.kind.kind}` },
      el("div", { class: "row-text" }, label, el("p", { class: "muted help" }, settingHelp(s.key))),
      el("div", { class: "row-control" }, control, rowStatus(s.key)),
    );
  }

  function languageSection(): HTMLElement {
    const key = "language";
    const id = "setting-language";
    // The system's language, named in itself, so it reads right whatever is chosen now.
    const resolved = resolveLocale("system");
    const system = resolved === PSEUDO ? PSEUDO : LOCALE_NAMES[resolved];
    const select = el("select", { attrs: { id, "data-key": key } });
    const choices: [LanguageChoice, string][] = [
      ["system", t("settings.language.system", { language: system })],
      ...LOCALES.map((l): [LanguageChoice, string] => [l, LOCALE_NAMES[l]]),
      ...(PSEUDO_AVAILABLE ? [[PSEUDO, PSEUDO_NAME] as [LanguageChoice, string]] : []),
    ];
    for (const [value, label] of choices) {
      const attrs: Record<string, string> = { value, ...(value === options.language ? { selected: "" } : {}) };
      // Each language name is in its own language; tell screen readers which.
      if (value !== "system" && value !== PSEUDO) attrs.lang = value;
      append(select, el("option", { attrs }, label));
    }
    select.addEventListener("change", async () => {
      const next = select.value as LanguageChoice;
      try {
        await api.setLanguage(next === "system" ? null : next);
      } catch (e) {
        status = { key, ok: false, text: message(e) };
        render();
        return;
      }
      // Redraws the whole app (this view included) in the new language.
      options.onLanguageChanged(next);
    });

    return el(
      "section",
      { class: "group" },
      el("h2", {}, t("settings.language.title")),
      el(
        "div",
        { class: "row row-choice" },
        el(
          "div",
          { class: "row-text" },
          el("label", { attrs: { for: id } }, t("settings.language.label")),
          el("p", { class: "muted help" }, t("settings.language.help")),
        ),
        el("div", { class: "row-control" }, select, rowStatus(key)),
      ),
    );
  }

  function gitSection(v: SettingsView): HTMLElement {
    const g = v.git;
    const key = "git-path";
    const summary = g.path
      ? el(
          "p",
          { class: "git-summary" },
          tx(g.chosen ? "settings.git.usingChosen" : "settings.git.usingFound", {
            version: g.version ?? "",
            path: el("code", {}, tilde(g.path)),
          }),
        )
      : el(
          "div",
          { class: "git-summary" },
          el(
            "p",
            { class: "problem-text" },
            t(g.chosen ? "settings.git.chosenBroken" : "settings.git.notFound"),
          ),
          ...gitError(g.error),
        );

    const input = el("input", {
      type: "text",
      class: "mono",
      attrs: {
        "data-key": key,
        "aria-label": t("settings.git.pathLabel"),
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
                const picked = await api.pickGitExecutable(t("dialog.chooseGit"));
                if (picked) input.value = picked;
              } catch (e) {
                status = { key, ok: false, text: message(e) };
                render();
              }
            },
          },
        },
        t("common.browse"),
      ),
      el("button", { type: "submit" }, t("settings.git.use")),
    );
    form.addEventListener("submit", (ev) => {
      ev.preventDefault();
      const p = input.value.trim();
      drafts.set(key, input.value);
      void change(key, () => api.setGitPath(p === "" ? null : p), t(p === "" ? "settings.git.findingAuto" : "common.saved")).then(
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
                void change(key, () => api.setGitPath(null), t("settings.git.findingAuto")).then(() => {
                  if (status?.ok) options.onGitChanged();
                }),
            },
          },
          t("settings.git.findAuto"),
        )
      : null;

    return el(
      "section",
      { class: "group" },
      el("h2", {}, t("settings.git.title")),
      summary,
      el(
        "div",
        { class: "row row-text-wide" },
        el(
          "div",
          { class: "row-text" },
          el("p", { class: "muted help" }, t("settings.git.help")),
        ),
        el("div", { class: "row-control" }, form, auto, rowStatus(key)),
      ),
    );
  }

  void load();
}

function message(e: unknown): string {
  return errorText(e);
}

/** Why no usable git was found: the message, then git's or the system's own words. */
function gitError(error: CommandError | null): HTMLElement[] {
  if (!error) return [];
  const e = new AppError(error);
  const detail = errorDetail(e);
  return [el("p", {}, errorText(e)), ...(detail ? [el("p", { class: "mono detail" }, detail)] : [])];
}

function failure(e: unknown): HTMLElement {
  return el(
    "div",
    { class: "verdict problem", attrs: { role: "alert" } },
    el("p", { class: "lede" }, t("settings.loadFailed")),
    el("p", {}, errorText(e)),
    errorDetail(e) ? el("p", { class: "mono detail" }, errorDetail(e)) : null,
  );
}

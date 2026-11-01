// App shell: picks the language, finds git, then shows one of the views.

import { api } from "./lib/api";
import { append, clear, el } from "./lib/dom";
import { type LanguageChoice, type MessageKey, LOCALES, resolveLocale, setLocale, t, tx } from "./lib/i18n";
import { setHome } from "./lib/paths";
import { rememberFolder } from "./lib/recent";
import { mountIdentityView } from "./views/identity";
import { mountRulesView } from "./views/rules";
import { mountScanView } from "./views/scan";
import { mountScopesView } from "./views/scopes";
import { mountSettingsView } from "./views/settings";

type ViewName = "identity" | "scan" | "rules" | "scopes" | "settings";

const TABS: [ViewName, MessageKey][] = [
  ["identity", "tab.identity"],
  ["scan", "tab.scan"],
  ["rules", "tab.rules"],
  ["scopes", "tab.scopes"],
  ["settings", "tab.settings"],
];

const root = document.getElementById("app");
if (root) void start(root);

/** The saved choice; anything unknown (a language removed later) means "system". */
async function loadLanguage(): Promise<LanguageChoice> {
  try {
    const saved = await api.language();
    return LOCALES.find((l) => l === saved) ?? "system";
  } catch {
    return "system";
  }
}

async function start(root: HTMLElement): Promise<void> {
  let language = await loadLanguage();
  setLocale(resolveLocale(language));

  const status = el("p", { class: "git-status muted" });
  const tabs = el("nav", { class: "tabs" });
  const content = el("div", { class: "content" });
  append(root, el("header", { class: "top" }, el("h1", {}, t("app.title")), tabs, status), content);

  let git: { version: string; path: string } | null = null;
  let looking = true;
  let current: ViewName = "identity";

  function renderStatus(): void {
    if (looking) {
      status.textContent = t("app.git.looking");
      status.removeAttribute("title");
    } else if (git) {
      status.textContent = t("app.git.version", { version: git.version });
      status.title = git.path;
    } else {
      status.textContent = t("app.git.notFound");
      status.removeAttribute("title");
    }
  }

  async function refreshGit(): Promise<void> {
    try {
      const info = await api.gitInfo();
      setHome(info.home);
      git = { version: info.version, path: info.path };
    } catch {
      git = null;
    }
    looking = false;
    renderStatus();
  }

  /** Switch language and redraw everything, staying on the current view. */
  function changeLanguage(next: LanguageChoice): void {
    language = next;
    setLocale(resolveLocale(next));
    renderStatus();
    show(current);
  }

  function show(view: ViewName): void {
    current = view;
    document.title = t("app.title");
    tabs.setAttribute("aria-label", t("app.views"));
    clear(tabs);
    for (const [name, label] of TABS) {
      append(
        tabs,
        el(
          "button",
          {
            type: "button",
            class: "tab",
            attrs: name === current ? { "aria-current": "page" } : {},
            on: { click: () => name !== current && show(name) },
          },
          t(label),
        ),
      );
    }

    clear(content);
    if (view !== "settings" && !git) {
      append(content, noGit(() => show("settings")));
      return;
    }
    if (view === "identity") {
      mountIdentityView(content, { openSettings: () => show("settings") });
    } else if (view === "scan") {
      mountScanView(content, {
        // The Identity view opens the most recent folder.
        openFolder: (path) => {
          rememberFolder(path);
          show("identity");
        },
      });
    } else if (view === "rules") {
      mountRulesView(content);
    } else if (view === "scopes") {
      mountScopesView(content);
    } else {
      mountSettingsView(content, {
        onGitChanged: () => void refreshGit(),
        language,
        onLanguageChanged: changeLanguage,
      });
    }
  }

  renderStatus();
  await refreshGit();
  show(git ? "identity" : "settings");
}

function noGit(openSettings: () => void): HTMLElement {
  return el(
    "div",
    { class: "verdict problem", attrs: { role: "alert" } },
    el("p", { class: "lede" }, t("noGit.lede")),
    el("p", {}, tx("noGit.body", { command: el("code", {}, "xcode-select --install") })),
    el("p", {}, el("button", { type: "button", class: "primary", on: { click: openSettings } }, t("noGit.choose"))),
  );
}

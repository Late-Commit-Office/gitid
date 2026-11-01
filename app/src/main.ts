// App shell: finds git, then shows the Identity, Rules or Settings view.

import { api } from "./lib/api";
import { append, clear, el } from "./lib/dom";
import { setHome } from "./lib/paths";
import { mountIdentityView } from "./views/identity";
import { mountRulesView } from "./views/rules";
import { mountSettingsView } from "./views/settings";

type ViewName = "identity" | "rules" | "settings";

const root = document.getElementById("app");
if (root) void start(root);

async function start(root: HTMLElement): Promise<void> {
  const status = el("p", { class: "git-status muted" }, "Looking for git…");
  const tabs = el("nav", { class: "tabs", attrs: { "aria-label": "Views" } });
  const content = el("div", { class: "content" });
  append(root, el("header", { class: "top" }, el("h1", {}, "gitid"), tabs, status), content);

  let gitOk = false;
  let current: ViewName = "identity";

  async function refreshGit(): Promise<void> {
    try {
      const git = await api.gitInfo();
      setHome(git.home);
      status.textContent = `git ${git.version}`;
      status.title = git.path;
      gitOk = true;
    } catch {
      status.textContent = "git not found";
      status.removeAttribute("title");
      gitOk = false;
    }
  }

  function show(view: ViewName): void {
    current = view;
    clear(tabs);
    for (const [name, label] of [
      ["identity", "Identity"],
      ["rules", "Rules"],
      ["settings", "Settings"],
    ] as [ViewName, string][]) {
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
          label,
        ),
      );
    }

    clear(content);
    if (view !== "settings" && !gitOk) {
      append(content, noGit(() => show("settings")));
      return;
    }
    if (view === "identity") {
      mountIdentityView(content, { openSettings: () => show("settings") });
    } else if (view === "rules") {
      mountRulesView(content);
    } else {
      mountSettingsView(content, {
        onGitChanged: () => void refreshGit(),
      });
    }
  }

  await refreshGit();
  show(gitOk ? "identity" : "settings");
}

function noGit(openSettings: () => void): HTMLElement {
  return el(
    "div",
    { class: "verdict problem", attrs: { role: "alert" } },
    el("p", { class: "lede" }, "gitid needs git, and couldn't find it."),
    el(
      "p",
      {},
      "Install git from git-scm.com (on a Mac, running xcode-select --install also works) and reopen gitid, or tell gitid where git is.",
    ),
    el("p", {}, el("button", { type: "button", class: "primary", on: { click: openSettings } }, "Choose where git is")),
  );
}


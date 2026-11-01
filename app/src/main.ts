// App shell: finds git, then shows the identity view.

import { api, AppError } from "./lib/api";
import { append, el } from "./lib/dom";
import { setHome, tilde } from "./lib/paths";
import { mountIdentityView } from "./views/identity";

async function start(): Promise<void> {
  const root = document.getElementById("app");
  if (!root) return;

  const status = el("p", { class: "git-status muted" }, "Looking for git…");
  append(root, el("header", { class: "top" }, el("h1", {}, "gitid"), status));

  try {
    const git = await api.gitInfo();
    setHome(git.home);
    status.textContent = `git ${git.version}, ${tilde(git.path)}`;
    status.title = git.path;
  } catch (e) {
    const err = e instanceof AppError ? e : new AppError("internal", String(e));
    status.textContent = "git not found";
    append(
      root,
      el(
        "div",
        { class: "verdict problem", attrs: { role: "alert" } },
        el("p", { class: "lede" }, "gitid needs git, and couldn't find it."),
        el("p", {}, "Install git from git-scm.com (on a Mac, running xcode-select --install also works), then reopen gitid."),
        el("p", { class: "mono detail" }, err.message),
      ),
    );
    return;
  }

  mountIdentityView(root);
}

void start();

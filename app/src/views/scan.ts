// The scan view: find repositories under a folder whose identity is missing,
// guessed, or not your default, and why.

import { api } from "../lib/api";
import { t } from "../lib/i18n";
import { errorText } from "../lib/messages";
import { append, clear, el } from "../lib/dom";
import { tilde } from "../lib/paths";
import type { Finding, Reason, ScanProgress, ScanReport } from "../lib/types";

const ROOT_KEY = "gitid.scanRoot";

function savedRoot(): string | null {
  try {
    return localStorage.getItem(ROOT_KEY);
  } catch {
    return null;
  }
}

function saveRoot(path: string): void {
  try {
    localStorage.setItem(ROOT_KEY, path);
  } catch {
    /* storage unavailable: nothing to remember */
  }
}

export interface ScanOptions {
  /** Show one repository in the Identity view. */
  openFolder: (path: string) => void;
}

export function mountScanView(root: HTMLElement, options: ScanOptions): void {
  const body = el("div", { class: "scan" });
  append(root, body);

  let folder: string | null = savedRoot();
  let hidden = false;
  let running = false;
  let progress: ScanProgress | null = null;
  let report: ScanReport | null = null;
  let error: string | null = null;
  // Updated in place while scanning, so the page doesn't re-render per repo.
  let progressText: HTMLElement | null = null;
  let progressBar: HTMLProgressElement | null = null;

  async function run(): Promise<void> {
    if (!folder || running) return;
    running = true;
    progress = null;
    report = null;
    error = null;
    render();
    try {
      report = await api.scan(folder, hidden, (p) => {
        progress = p;
        updateProgress();
      });
      saveRoot(folder);
    } catch (e) {
      error = errorText(e);
    } finally {
      running = false;
      render();
    }
  }

  function updateProgress(): void {
    if (!progressText || !progressBar) return;
    const p = progress;
    progressText.textContent = p
      ? p.checked === 0
        ? `Looking through folders: ${p.foldersSeen.toLocaleString()} so far, ${p.found} repositories found.`
        : `Checking repositories: ${p.checked} of ${p.found}.`
      : "Starting…";
    if (p && p.found > 0 && p.checked > 0) {
      progressBar.max = p.found;
      progressBar.value = p.checked;
    } else {
      progressBar.removeAttribute("value"); // indeterminate while walking
    }
  }

  function render(): void {
    clear(body);
    append(body, controls());
    if (running) {
      progressText = el("p", { attrs: { "aria-live": "polite" } });
      progressBar = el("progress", { attrs: { "aria-label": "Scan progress" } });
      append(
        body,
        el(
          "div",
          { class: "scan-progress" },
          progressBar,
          progressText,
          el("button", { type: "button", on: { click: () => void api.cancelScan() } }, "Stop"),
        ),
      );
      updateProgress();
      return;
    }
    progressText = progressBar = null;
    if (error) {
      append(body, el("div", { class: "verdict problem", attrs: { role: "alert" } }, el("p", { class: "lede" }, "The scan couldn't run."), el("p", { class: "mono detail" }, error)));
      return;
    }
    if (report) append(body, results(report));
  }

  function controls(): HTMLElement {
    const hiddenBox = el("input", {
      type: "checkbox",
      attrs: { id: "scan-hidden", ...(hidden ? { checked: "" } : {}), ...(running ? { disabled: "" } : {}) },
    });
    hiddenBox.addEventListener("change", () => (hidden = hiddenBox.checked));

    return el(
      "div",
      { class: "scan-controls" },
      el("p", { class: "muted intro" }, "Find the repositories under a folder whose commits would fail, use a made-up identity, or use something other than your default."),
      el(
        "div",
        { class: "bar-row" },
        el(
          "button",
          {
            type: "button",
            attrs: running ? { disabled: "" } : {},
            on: {
              click: async () => {
                const picked = await api.pickFolder(t("dialog.chooseFolder")).catch(() => null);
                if (picked) {
                  folder = picked;
                  report = null;
                  render();
                }
              },
            },
          },
          folder ? "Choose another folder" : "Choose a folder",
        ),
        el("span", { class: "current mono", title: folder ?? "" }, folder ? tilde(folder) : "No folder chosen"),
        el(
          "button",
          { type: "button", class: "primary", attrs: !folder || running ? { disabled: "" } : {}, on: { click: () => void run() } },
          report ? "Scan again" : "Scan",
        ),
      ),
      el("label", { class: "check-label muted", attrs: { for: "scan-hidden" } }, hiddenBox, " Also look in hidden folders (like ~/.config)"),
    );
  }

  function results(r: ScanReport): HTMLElement {
    const problems = r.findings.filter((f) => f.verdict.kind === "missing" || f.verdict.kind === "guessed");
    const different = r.findings.filter((f) => f.verdict.kind === "different");
    const same = r.findings.filter((f) => f.verdict.kind === "same");
    const out = el("div", { class: "scan-results" });

    const summary =
      r.findings.length === 0
        ? "No repositories found."
        : problems.length
          ? `${problems.length} of ${count(r.findings.length)} can't commit with a proper identity.`
          : r.findings.length === 1
            ? "The repository has a proper identity."
            : `All ${count(r.findings.length)} have a proper identity.`;
    append(
      out,
      el(
        "div",
        { class: `verdict ${problems.length ? "problem" : "ok"}` },
        el("p", { class: "lede" }, summary),
        r.cancelled ? el("p", { class: "muted" }, "Stopped early: these are the repositories checked so far.") : null,
        r.unreadable ? el("p", { class: "muted" }, `${r.unreadable} folders couldn't be read and were skipped.`) : null,
      ),
    );

    if (problems.length) append(out, section("To fix", problems, r.root, "problem"));
    if (different.length) append(out, section("Not your default identity", different, r.root, "note"));
    if (same.length) {
      const details = el("details", { class: "same-list" }, el("summary", {}, `Your default identity (${same.length})`));
      append(details, list(same, r.root));
      append(out, details);
    }
    return out;
  }

  function section(title: string, items: Finding[], rootPath: string, tone: "problem" | "note"): HTMLElement {
    return el("section", { class: `scan-section tone-${tone}` }, el("h2", {}, `${title} (${items.length})`), list(items, rootPath));
  }

  function list(items: Finding[], rootPath: string): HTMLElement {
    const ul = el("ul", { class: "findings" });
    for (const f of items) {
      append(
        ul,
        el(
          "li",
          { class: "finding" },
          el("p", { class: "mono finding-path", title: f.path }, relative(f.path, rootPath)),
          el("p", { class: "mono finding-who" }, f.identity ?? "No identity"),
          f.verdict.kind === "same" ? null : el("p", { class: "muted finding-why" }, why(f)),
          el("button", { type: "button", class: "link", on: { click: () => options.openFolder(f.path) } }, "Open"),
        ),
      );
    }
    return ul;
  }

  render();
}

function why(f: Finding): string {
  const v = f.verdict;
  switch (v.kind) {
    case "missing":
      return `git will refuse to commit: ${v.message}`;
    case "guessed":
      return "Made up by git from your computer's login.";
    case "different":
      return "From " + v.because.map(reason).join(" and ") + ".";
    case "same":
      return "";
  }
}

function reason(r: Reason): string {
  switch (r.kind) {
    case "rule":
      return `the rule “${r.id}”`;
    case "repoConfig":
      return "the repository's own config";
    case "env":
      return `the environment variable ${r.var}`;
    case "otherConfig":
      return r.file ? tilde(r.file) : "another config file";
  }
}

function relative(path: string, rootPath: string): string {
  const p = path.replace(/\\/g, "/");
  const r = rootPath.replace(/\\/g, "/").replace(/\/+$/, "");
  if (p === r) return tilde(path);
  return p.startsWith(r + "/") ? p.slice(r.length + 1) : tilde(path);
}

function count(n: number): string {
  return `${n} ${n === 1 ? "repository" : "repositories"}`;
}

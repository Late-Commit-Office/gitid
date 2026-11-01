// The scan view: find repositories under a folder whose identity is missing,
// guessed, or not your default, and why.

import { api } from "../lib/api";
import { type MessageKey, formatList, formatNumber, t, tn, tx } from "../lib/i18n";
import { errorDetail, errorText } from "../lib/messages";
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
  let error: unknown = null;
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
      error = e;
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
        ? tn("scan.progress.walking", p.found, { folders: formatNumber(p.foldersSeen) })
        : t("scan.progress.checking", { checked: formatNumber(p.checked), found: formatNumber(p.found) })
      : t("scan.progress.starting");
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
      progressBar = el("progress", { attrs: { "aria-label": t("scan.progress.label") } });
      append(
        body,
        el(
          "div",
          { class: "scan-progress" },
          progressBar,
          progressText,
          el("button", { type: "button", on: { click: () => void api.cancelScan() } }, t("scan.stop")),
        ),
      );
      updateProgress();
      return;
    }
    progressText = progressBar = null;
    if (error) {
      const detail = errorDetail(error);
      append(
        body,
        el(
          "div",
          { class: "verdict problem", attrs: { role: "alert" } },
          el("p", { class: "lede" }, t("scan.failed")),
          el("p", {}, errorText(error)),
          detail ? el("p", { class: "mono detail" }, detail) : null,
        ),
      );
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
      el("p", { class: "muted intro" }, t("scan.intro")),
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
          t(folder ? "common.chooseAnother" : "common.chooseFolder"),
        ),
        el("span", { class: "current mono", title: folder ?? "" }, folder ? tilde(folder) : t("scan.noFolder")),
        el(
          "button",
          { type: "button", class: "primary", attrs: !folder || running ? { disabled: "" } : {}, on: { click: () => void run() } },
          t(report ? "scan.runAgain" : "scan.run"),
        ),
      ),
      el("label", { class: "check-label muted", attrs: { for: "scan-hidden" } }, hiddenBox, el("span", {}, tx("scan.hidden", { example: el("code", {}, "~/.config") }))),
    );
  }

  function results(r: ScanReport): HTMLElement {
    const problems = r.findings.filter((f) => f.verdict.kind === "missing" || f.verdict.kind === "guessed");
    const different = r.findings.filter((f) => f.verdict.kind === "different");
    const same = r.findings.filter((f) => f.verdict.kind === "same");
    const out = el("div", { class: "scan-results" });

    const total = r.findings.length;
    const summary =
      total === 0
        ? t("scan.summary.none")
        : problems.length
          ? tn("scan.summary.problems", total, { problems: formatNumber(problems.length) })
          : tn("scan.summary.allOk", total);
    append(
      out,
      el(
        "div",
        { class: `verdict ${problems.length ? "problem" : "ok"}` },
        el("p", { class: "lede" }, summary),
        r.cancelled ? el("p", { class: "muted" }, t("scan.cancelled")) : null,
        r.unreadable ? el("p", { class: "muted" }, tn("scan.unreadable", r.unreadable)) : null,
      ),
    );

    if (problems.length) append(out, section("scan.section.toFix", problems, r.root, "problem"));
    if (different.length) append(out, section("scan.section.different", different, r.root, "note"));
    if (same.length) {
      const details = el(
        "details",
        { class: "same-list" },
        el("summary", {}, t("scan.section.same", { count: formatNumber(same.length) })),
      );
      append(details, list(same, r.root));
      append(out, details);
    }
    return out;
  }

  function section(heading: MessageKey, items: Finding[], rootPath: string, tone: "problem" | "note"): HTMLElement {
    return el(
      "section",
      { class: `scan-section tone-${tone}` },
      el("h2", {}, t(heading, { count: formatNumber(items.length) })),
      list(items, rootPath),
    );
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
          el("p", { class: "mono finding-who" }, f.identity ?? t("scan.noIdentity")),
          f.verdict.kind === "same" ? null : el("p", { class: "muted finding-why" }, why(f)),
          el("button", { type: "button", class: "link", on: { click: () => options.openFolder(f.path) } }, t("scan.open")),
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
      return t("scan.why.missing", { detail: v.message });
    case "guessed":
      return t("scan.why.guessed");
    case "different":
      return t("scan.why.different", { reasons: formatList(v.because.map(reason)) });
    case "same":
      return "";
  }
}

function reason(r: Reason): string {
  switch (r.kind) {
    case "rule":
      return t("scan.reason.rule", { id: r.id });
    case "repoConfig":
      return t("scan.reason.repoConfig");
    case "env":
      return t("scan.reason.env", { var: r.var });
    case "otherConfig":
      return r.file ? tilde(r.file) : t("scan.reason.otherConfig");
  }
}

function relative(path: string, rootPath: string): string {
  const p = path.replace(/\\/g, "/");
  const r = rootPath.replace(/\\/g, "/").replace(/\/+$/, "");
  if (p === r) return tilde(path);
  return p.startsWith(r + "/") ? p.slice(r.length + 1) : tilde(path);
}

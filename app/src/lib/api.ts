// Typed wrappers around the Rust commands. Nothing else calls invoke().

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AddTo,
  CliStatus,
  CommandError,
  ErrorParam,
  GitInfo,
  IdentityView,
  RuleCheck,
  RuleInput,
  RulesView,
  SaveRuleResult,
  ScanProgress,
  ScanReport,
  ScopesView,
  SettingsView,
  ValueRef,
} from "./types";

/** A failed command. `message` is English; show errors with errorText() from lib/errors. */
export class AppError extends Error {
  readonly kind: string;
  readonly code: string;
  readonly params: Record<string, ErrorParam>;
  readonly detail: string | null;

  constructor(e: CommandError) {
    super(e.message);
    this.kind = e.kind;
    this.code = e.code;
    this.params = e.params ?? {};
    this.detail = e.detail ?? null;
  }
}

function isCommandError(e: unknown): e is CommandError {
  return typeof e === "object" && e !== null && "kind" in e && "code" in e && "message" in e;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    if (isCommandError(e)) throw new AppError(e);
    const text = String(e);
    throw new AppError({ kind: "internal", code: "internal", params: {}, message: text, detail: text });
  }
}

export const api = {
  gitInfo: () => call<GitInfo>("git_info"),
  identity: (path: string) => call<IdentityView>("get_identity", { path }),
  /** Native folder picker; null if cancelled. */
  pickFolder: (title: string) => call<string | null>("pick_folder", { title }),

  settings: () => call<SettingsView>("get_settings"),
  /** null (or git's default) removes the setting. Returns the updated view. */
  setSetting: (key: string, value: string | null) =>
    call<SettingsView>("set_setting", { key, value }),
  /** null: find git automatically again. */
  setGitPath: (path: string | null) => call<SettingsView>("set_git_path", { path }),
  pickGitExecutable: (title: string) => call<string | null>("pick_git_executable", { title }),
  /** The language chosen in Settings; null follows the system. */
  language: () => call<string | null>("get_language"),
  setLanguage: (language: string | null) => call<void>("set_language", { language }),
  /** Whether the gitid command is installed, and where. */
  cliStatus: () => call<CliStatus>("cli_status"),
  /** macOS: link the command (asks for a password). `prompt` is shown above the password field. */
  installCli: (replace: boolean, prompt: string) => call<CliStatus>("install_cli", { replace, prompt }),
  removeCli: (prompt: string) => call<CliStatus>("remove_cli", { prompt }),

  rules: () => call<RulesView>("list_rules"),
  saveRule: (input: RuleInput) => call<SaveRuleResult>("save_rule", { input }),
  removeRule: (id: string, deleteFile: boolean) => call<RulesView>("remove_rule", { id, deleteFile }),
  checkRule: (id: string, path: string) => call<RuleCheck>("check_rule", { id, path }),

  /** Config visible from `path` (null: outside any folder). */
  scopes: (path: string | null) => call<ScopesView>("get_scopes", { path }),
  editScopeValue: (path: string | null, at: ValueRef, value: string) =>
    call<ScopesView>("edit_scope_value", { path, at, value }),
  removeScopeValue: (path: string | null, at: ValueRef) =>
    call<ScopesView>("remove_scope_value", { path, at }),
  addScopeValue: (path: string | null, scope: AddTo, key: string, value: string) =>
    call<ScopesView>("add_scope_value", { path, scope, key, value }),

  /** Scan a folder for repos; progress arrives through `onProgress`. */
  scan: (path: string, includeHidden: boolean, onProgress: (p: ScanProgress) => void) => {
    const channel = new Channel<ScanProgress>();
    channel.onmessage = onProgress;
    return call<ScanReport>("scan_folder", { path, depth: null, includeHidden, onProgress: channel });
  },
  /** Stop the scan in progress; `scan` then resolves with what was found so far. */
  cancelScan: () => call<void>("cancel_scan"),
};

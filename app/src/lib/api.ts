// Typed wrappers around the Rust commands. Nothing else calls invoke().

import { invoke } from "@tauri-apps/api/core";
import type { CommandError, GitInfo, IdentityView, SettingsView } from "./types";

export class AppError extends Error {
  constructor(
    readonly kind: string,
    message: string,
  ) {
    super(message);
  }
}

function isCommandError(e: unknown): e is CommandError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    if (isCommandError(e)) throw new AppError(e.kind, e.message);
    throw new AppError("internal", String(e));
  }
}

export const api = {
  gitInfo: () => call<GitInfo>("git_info"),
  identity: (path: string) => call<IdentityView>("get_identity", { path }),
  /** Native folder picker; null if cancelled. */
  pickFolder: () => call<string | null>("pick_folder"),

  settings: () => call<SettingsView>("get_settings"),
  /** null (or git's default) removes the setting. Returns the updated view. */
  setSetting: (key: string, value: string | null) =>
    call<SettingsView>("set_setting", { key, value }),
  /** null: find git automatically again. */
  setGitPath: (path: string | null) => call<SettingsView>("set_git_path", { path }),
  pickGitExecutable: () => call<string | null>("pick_git_executable"),
};

// The JSON the Rust commands return. Keep in sync with the serde attributes in
// gitid-core and app/src-tauri/src/commands.rs; crates/core/tests/json_contract.rs
// pins the shape so a change on the Rust side fails CI instead of the UI.

export type Scope = "system" | "global" | "local" | "worktree" | "command" | "unknown";

export type Origin =
  | { kind: "file"; value: string }
  | { kind: "blob"; value: string }
  | { kind: "commandLine" }
  | { kind: "stdin" }
  | { kind: "other"; value: string };

export interface Entry {
  key: string;
  value: string | null;
  scope: Scope;
  origin: Origin;
}

export type Source =
  | { kind: "env"; var: string }
  | ({ kind: "config" } & Entry)
  | { kind: "guessed" };

export interface Field {
  value: string;
  source: Source;
}

export type Role = "author" | "committer";
export type Part = "name" | "email";

export interface RoleIdentity {
  role: Role;
  name: Field | null;
  email: Field | null;
  /** Set when git would refuse to commit. */
  error: string | null;
}

export interface EnvVar {
  name: string;
  /** Empty when sensitive: never sent to the UI. */
  value: string;
  sensitive: boolean;
}

export interface RepoLocation {
  workTree: string | null;
  gitDir: string;
}

export type Severity = "problem" | "note";

export type Warning =
  | { kind: "missing"; role: Role; message: string }
  | { kind: "guessed"; role: Role; part: Part }
  | { kind: "fromEnv"; role: Role; part: Part; var: string }
  | { kind: "injected"; role: Role; part: Part; key: string }
  | { kind: "authorCommitterDiffer" }
  | { kind: "guessingAllowed" };

export interface WarningReport {
  severity: Severity;
  warning: Warning;
  message: string;
}

/** Rule ids that supplied each value, if any. */
export interface RoleRules {
  name: string | null;
  email: string | null;
}

export interface IdentityView {
  folder: string;
  repo: RepoLocation | null;
  author: RoleIdentity;
  committer: RoleIdentity;
  useConfigOnly: boolean;
  env: EnvVar[];
  warnings: WarningReport[];
  hasProblems: boolean;
  authorRules: RoleRules;
  committerRules: RoleRules;
}

export interface GitInfo {
  path: string;
  version: string;
  supportsHasconfig: boolean;
  home: string | null;
}

/** What a failed command rejects with. */
export interface CommandError {
  kind: string;
  message: string;
}

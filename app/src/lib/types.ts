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

// ---------------------------------------------------------------- settings

export type SettingKind =
  | { kind: "bool"; default: boolean }
  | { kind: "text"; placeholder: string }
  | { kind: "choice"; choices: { value: string; label: string }[]; default: string };

export type SettingGroup = "identity" | "defaults" | "signing";

export interface SettingValue {
  key: string;
  label: string;
  help: string;
  group: SettingGroup;
  kind: SettingKind;
  /** null when the global config doesn't set it (git's default applies). */
  value: string | null;
  origin: Origin | null;
}

export interface GitLocation {
  /** Chosen in Settings; null means "search for it". */
  chosen: string | null;
  path: string | null;
  version: string | null;
  error: string | null;
}

export interface SettingsView {
  git: GitLocation;
  globalFile: string | null;
  /** Empty when no working git was found. */
  settings: SettingValue[];
}

// ---------------------------------------------------------------- rules

export type Condition =
  | { kind: "folder"; pattern: string; caseInsensitive: boolean }
  | { kind: "remoteUrl"; pattern: string }
  | { kind: "other"; raw: string };

/** One "applies to" line of the rule form; sent back unchanged when saving. */
export type When =
  | { kind: "folder"; path: string }
  | { kind: "remoteHost"; host: string; sshPort: number | null }
  | { kind: "remotePattern"; pattern: string }
  | { kind: "raw"; raw: string };

export interface RuleView {
  id: string;
  includePath: string;
  file: string;
  fileExists: boolean;
  conditions: Condition[];
  name: string | null;
  email: string | null;
  /** Created by gitid; others can be removed but not edited. */
  managed: boolean;
  description: string[];
  when: When[];
}

export interface RulesView {
  rules: RuleView[];
  supportsHasconfig: boolean;
}

export interface RuleCheck {
  path: string;
  applies: boolean;
}

export interface SaveRuleResult {
  rule: RuleView;
  checks: RuleCheck[];
}

export interface RuleInput {
  id: string;
  name: string | null;
  email: string | null;
  when: When[];
  replace: boolean;
}

// ---------------------------------------------------------------- scopes

/** What git does with one value: uses it, ignores it for a later one, or (for list keys) uses it too. */
export type ValueStatus = "used" | "overridden" | "alsoUsed";

/** Points at one value for editing: the n-th value of `key` in `file`. */
export interface ValueRef {
  file: string;
  key: string;
  nth: number;
}

export interface ScopeValue {
  scope: Scope;
  status: ValueStatus;
  /** Masked if it looks like a secret; null for a bare `key` line (true). */
  value: string | null;
  masked: boolean;
  origin: Origin;
  /** Read from a file the scope's main file includes (e.g. a rule's file). */
  included: boolean;
  /** null when the value can't be edited here (set by git -c or the environment). */
  at: ValueRef | null;
}

export interface KeyView {
  key: string;
  multiValued: boolean;
  /** In git's reading order: system, global, local (later wins). */
  values: ScopeValue[];
}

export interface ScopesView {
  folder: string | null;
  repo: RepoLocation | null;
  keys: KeyView[];
}

export type AddTo = "system" | "global" | "local";

// ---------------------------------------------------------------- scan

export interface ScanProgress {
  foldersSeen: number;
  found: number;
  checked: number;
}

/** Why a repo's identity differs from your default. */
export type Reason =
  | { kind: "rule"; id: string }
  | { kind: "repoConfig" }
  | { kind: "env"; var: string }
  | { kind: "otherConfig"; file: string | null };

export type Verdict =
  | { kind: "missing"; message: string }
  | { kind: "guessed" }
  | { kind: "different"; because: Reason[] }
  | { kind: "same" };

export interface Finding {
  path: string;
  /** `Name <email>` as commits would show it, if git can produce one. */
  identity: string | null;
  verdict: Verdict;
}

export interface ScanReport {
  root: string;
  /** Problems first, then different, then same. */
  findings: Finding[];
  cancelled: boolean;
  unreadable: number;
}

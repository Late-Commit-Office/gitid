> **Note:** This repository is mostly coded by an LLM (Claude), with direction and review by a human.

# gitid

See which git identity applies to any folder, and why, so commits never fall back to your OS login name again.

Status: **all v1 features are in.** The `gitid` CLI and the desktop app explain the effective identity, manage per-folder and per-server identity rules, show and edit all config with which value wins, scan folders for repos with a wrong identity, and edit your global settings.

## The desktop app

Needs [Rust](https://rustup.rs), [Node.js](https://nodejs.org) (LTS), and on macOS the Xcode Command Line Tools (`xcode-select --install`). On Linux, also Tauri's [system packages](https://v2.tauri.app/start/prerequisites/).

```sh
cd app
npm install                  # first time; commit the package-lock.json it creates
npm run tauri dev            # run with live reload
npm run tauri build          # installable bundle in target/release/bundle/
```

- **Identity:** pick a folder to see who commits there will be from, which file (or rule, or environment variable) each value comes from, and a button to fix each problem.
- **Scan:** pick a folder (like `~/code`) to find the repositories under it whose commits would fail, would use an identity git made up, or would use something other than your default, and why (a rule, the repo's own config, an environment variable). "Open" shows any of them in Identity.
- **Rules:** add, edit and remove identities for folders (`~/work`) or servers (your Gitea). Saving checks the rule against repos in its folders; "Check a folder" tests any repo. Rules written by hand are listed and can be removed, but not edited.
- **All config:** every key visible from a folder, each value it has in system, global and repository config (and files those include), and which one git uses. Values can be edited, removed and added. Secrets such as tokens in URLs are masked before they reach the app's window, so masked values can be removed but not edited.
- **Settings:** your default name and email, "never guess my identity" (`user.useConfigOnly`), default branch, pull behavior, editor, commit signing, and which git the app uses. Changes go to your global git config; the app's own choice of git is saved separately in its config folder.

## Try it

```sh
cargo install --path crates/cli         # puts `gitid` on your PATH

gitid whoami                            # who will commits here be from, and why?
gitid whoami ~/code/app
gitid show ~/code/app --key user.email  # every value of a key, and which one wins
gitid whoami --json                     # the same as JSON, for scripts

gitid scan ~/code                       # repos with a missing, guessed or non-default identity
gitid settings                          # the settings the app manages, with current values
gitid set user.name "Ann Lee"           # global by default; managed settings are checked first
gitid set user.email ann@repo.example --local
gitid set user.useConfigOnly true       # never let git guess an identity again
gitid unset user.email --local
```

### Identity rules

A rule says "repos here commit as X". It's an identity file in `~/.config/git/identities/` plus `includeIf` entries in your global config; git does the matching.

```sh
# Every repo under ~/work
gitid rule add work --email ann@company.example --dir ~/work

# Every repo whose remote is on a server: https, http or ssh, with or without
# credentials in the URL (needs git 2.36+)
gitid rule add gitea --name annlee --email ann@gitea.example \
    --host gitea.example.com --ssh-port 2222 --check ~/code/blog

gitid rule list                         # --json for scripts
gitid rule check gitea ~/code/blog      # does it apply there?
gitid rule remove work                  # also deletes its identity file
```

`rule add` checks the new rule against real repos (inside `--dir`, plus any `--check` folders) and says where it does and doesn't apply. Running it again with the same id adds to that rule; with `--replace` the rule becomes exactly what you pass (folders, servers, name and email), like editing it in the app. `gitid whoami` labels values that come from a rule.

`gitid whoami`, `rule add` and `rule check` exit with **3** when something is wrong (identity missing or guessed, rule not applying), so they can guard scripts or a pre-commit hook.

## Layout

```
crates/core    gitid-core: all logic, no UI
  git/         locate.rs (find git per OS), runner.rs (the Git trait: only place a process is spawned), version.rs
  config/      model.rs (Scope, Origin, Entry), read.rs (list with origin/scope, per-file reads), write.rs (set/add/unset/replace with an explicit Target), layers.rs (which value wins)
  repo.rs      repository discovery (work tree, git dir)
  identity.rs  effective author/committer + where each value came from + warnings
  settings.rs  the global settings gitid manages: labels, help, validation
  rules.rs     identity rules: list, add, edit, remove, check, describe
  scan.rs      find repos under a folder and sort them by identity
  platform.rs  home folder, case sensitivity, how paths are written in includeIf
  paths.rs     canonical paths without Windows \\?\ prefixes
  secrets.rs   masks credentials before display
crates/cli     gitid binary (clap)
app/           desktop app frontend: plain TypeScript, built with Vite
  src/lib/     api.ts (typed calls to Rust), types.ts (the JSON shapes), dom.ts, paths.ts, recent.ts
  src/views/   identity.ts, scan.ts, rules.ts, scopes.ts (All config), settings.ts
app/src-tauri/ desktop app backend: commands.rs, rule_commands.rs, scope_commands.rs, scan_commands.rs (the only calls the UI can make), state.rs (which git; saved settings.json)
```

The app's webview gets no plugin permissions: it can only call gitid's own commands, and even the folder picker is opened from Rust. Text from config is only ever inserted as text, never as HTML.

The rule: every read and write goes through the `git` executable. Config files are never parsed or written directly. (gitid only creates the identities folder, and deletes identity files it created itself.)

## Tests

```sh
cargo test                                   # core + CLI (the app isn't a default member)
cargo test --features gitid-core/serde       # also the JSON contract tests the app relies on
cd app && npm run typecheck                  # frontend types
```

Integration tests run real git in temp folders with `GIT_CONFIG_GLOBAL` pointed at a temp file, `GIT_CONFIG_NOSYSTEM=1` and a temp `HOME`, so your own config is never read or changed. They need git installed (2.36+ for the server-rule tests; older versions skip them).

## How rules match (found while building this)

- **Server rules need four patterns per host.** `hasconfig:remote.*.url:https://host/**` misses `https://user:token@host/...`, `ssh://git@host/...` and `git@host:owner/repo.git`. gitid writes `http*://HOST/**`, `http*://*@HOST/**`, `ssh://*@HOST/**` and `*@HOST:*/**` (plus `ssh://*@HOST:PORT/**` with `--ssh-port`). The obvious `*@HOST:**` doesn't work: `**` only crosses folders next to a `/`.
- **Folder rules use the real path.** git matches a repo's real location, so a rule on a symlinked folder never applies. gitid resolves symlinks before writing the rule.
- **Folder rules are case-insensitive on Windows and macOS** (`gitdir/i:`), where `Work` and `work` are the same folder.

## Notes for later steps

- The app's bundle identifier is a placeholder (`io.github.gitid` in `app/src-tauri/tauri.conf.json`). Change it to a reverse domain you own before the first release; changing it later moves the app's data folder.
- To change the app icon, `npm run tauri icon path/to/icon.png` (a 1024×1024 PNG) regenerates everything in `app/src-tauri/icons/`.
- Steps 1–2 were built without registry access, so `gitid-core` has no required dependencies (serde is an optional feature).
- macOS: `/usr/bin/git` may be the Command Line Tools shim; detect with `xcode-select -p` before calling it (TODO in `git/locate.rs`).

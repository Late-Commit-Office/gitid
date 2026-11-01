> **Note:** This repository is mostly coded by an LLM (Claude), with direction and review by a human.

# gitid

See which git identity applies to any folder, and why, so commits never fall back to your OS login name again.

Status: **step 2 of 4** — the core library and the `gitid` CLI can explain the effective identity, edit config, and manage per-folder and per-server identity rules. The Tauri app comes next.

## Try it

```sh
cargo install --path crates/cli         # puts `gitid` on your PATH

gitid whoami                            # who will commits here be from, and why?
gitid whoami ~/code/app
gitid show ~/code/app --key user.email  # every value of a key, and which one wins

gitid set user.name "Ann Lee"           # global by default
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

gitid rule list
gitid rule check gitea ~/code/blog      # does it apply there?
gitid rule remove work                  # also deletes its identity file
```

`rule add` checks the new rule against real repos (inside `--dir`, plus any `--check` folders) and says where it does and doesn't apply. Running it again with the same id updates that rule. `gitid whoami` labels values that come from a rule.

`gitid whoami`, `rule add` and `rule check` exit with **3** when something is wrong (identity missing or guessed, rule not applying), so they can guard scripts or a pre-commit hook.

## Layout

```
crates/core    gitid-core: all logic, no UI
  git/         locate.rs (find git per OS), runner.rs (the Git trait: only place a process is spawned), version.rs
  config/      model.rs (Scope, Origin, Entry), read.rs (list with origin/scope, per-file reads), write.rs (set/add/unset with an explicit Target)
  repo.rs      repository discovery (work tree, git dir)
  identity.rs  effective author/committer + where each value came from + warnings
  rules.rs     identity rules: list, add, remove, check, describe
  platform.rs  home folder, case sensitivity, how paths are written in includeIf
  paths.rs     canonical paths without Windows \\?\ prefixes
  secrets.rs   masks credentials before display
crates/cli     gitid binary
```

The rule: every read and write goes through the `git` executable. Config files are never parsed or written directly. (gitid only creates the identities folder, and deletes identity files it created itself.)

## Tests

```sh
cargo test
```

Integration tests run real git in temp folders with `GIT_CONFIG_GLOBAL` pointed at a temp file, `GIT_CONFIG_NOSYSTEM=1` and a temp `HOME`, so your own config is never read or changed. They need git installed (2.36+ for the server-rule tests; older versions skip them).

## How rules match (found while building this)

- **Server rules need four patterns per host.** `hasconfig:remote.*.url:https://host/**` misses `https://user:token@host/...`, `ssh://git@host/...` and `git@host:owner/repo.git`. gitid writes `http*://HOST/**`, `http*://*@HOST/**`, `ssh://*@HOST/**` and `*@HOST:*/**` (plus `ssh://*@HOST:PORT/**` with `--ssh-port`). The obvious `*@HOST:**` doesn't work: `**` only crosses folders next to a `/`.
- **Folder rules use the real path.** git matches a repo's real location, so a rule on a symlinked folder never applies. gitid resolves symlinks before writing the rule.
- **Folder rules are case-insensitive on Windows and macOS** (`gitdir/i:`), where `Work` and `work` are the same folder.

## Notes for later steps

- No dependencies yet: crates.io was unreachable when this was written. Swap the hand-rolled arg parser for clap, and add serde (for Tauri and `--json`), in step 3.
- macOS: `/usr/bin/git` may be the Command Line Tools shim; detect with `xcode-select -p` before calling it (TODO in `git/locate.rs`).

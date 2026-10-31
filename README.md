> **Note:** This repository is mostly coded by an LLM (Claude), with direction and review by a human.

# gitid

See which git identity applies to any folder, and why, so commits never fall back to your OS login name again.

Status: **step 1 of 4** — the core library and the `gitid` CLI can explain the effective identity. Editing config, `includeIf` rules and the Tauri app come next.

## Try it

```sh
cargo run -p gitid -- whoami            # current folder
cargo run -p gitid -- whoami ~/code/app
cargo run -p gitid -- show ~/code/app --key user.email
cargo install --path crates/cli         # puts `gitid` on your PATH
```

`gitid whoami` exits with **3** when the identity is missing or guessed, so it can guard scripts or a pre-commit hook.

## Layout

```
crates/core   gitid-core: all logic, no UI
  git/        locate.rs (find git per OS), runner.rs (the Git trait: only place a process is spawned), version.rs
  config/     model.rs (Scope, Origin, Entry, ConfigSnapshot), read.rs (`git config --list --show-origin --show-scope -z`)
  repo.rs     repository discovery (work tree, git dir)
  identity.rs effective author/committer + where each value came from + warnings
  secrets.rs  masks credentials before display
crates/cli    gitid binary
```

The rule: every read (and later write) goes through the `git` executable. Config files are never parsed or written directly.

## Tests

```sh
cargo test
```

Integration tests run real git in temp folders with `GIT_CONFIG_GLOBAL` pointed at a temp file and `GIT_CONFIG_NOSYSTEM=1`, so your own `~/.gitconfig` is never read or changed. They need git installed (2.36+ for the `hasconfig` test; older versions skip it).

## Notes for later steps

- `includeIf "hasconfig:remote.*.url:https://host/**"` does **not** match remotes with credentials in the URL (`https://user:token@host/...`). The rule builder should also write `https://*@host/**`.
- No dependencies yet: crates.io was unreachable when this was written. Swap the hand-rolled arg parser for clap, and add serde (for Tauri and `--json`), in step 3.
- macOS: `/usr/bin/git` may be the Command Line Tools shim; detect with `xcode-select -p` before calling it (TODO in `git/locate.rs`).

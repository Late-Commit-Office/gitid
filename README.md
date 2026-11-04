> **Note:** This repository is mostly coded by an LLM (Claude), with direction and review by a human.

# gitid

See which git identity applies to any folder, and why, so commits never fall back to your OS login name again.

Status: **all v1 features are in.** The `gitid` CLI and the desktop app explain the effective identity, manage per-folder and per-server identity rules, show and edit all config with which value wins, scan folders for repos with a wrong identity, and edit your global settings.

## Download and install

Each download is named for what's in it:

| File | What it is |
| --- | --- |
| `gitid-gui-cli-macos-universal.tar.gz` | The app for Macs, Apple silicon and Intel, with the `gitid` command inside |
| `gitid-gui-cli-windows-x64.zip` | The app's installer for 64-bit Windows, with the `gitid` command |
| `gitid-gui-cli-linux-x64.tar.gz` | The app for 64-bit Linux, as a `.deb` package and an AppImage, with the `gitid` command |
| `gitid-cli-<platform>` | The `gitid` command alone, for machines without a desktop (a server) |

Until the first release, these are under **Artifacts** on a CI run's summary page. There's no build for Arm Linux or Windows on Arm yet; [build from source](#building-from-source) there.

You'll also need [git](https://git-scm.com) itself. gitid finds it, or you can point it at one in Settings.

**macOS:** unpack the `.tar.gz` (double-click it) and drag `gitid.app` to Applications. The app isn't notarized yet, so the first time, right-click it and choose **Open**, or allow it under **System Settings > Privacy & Security**. If macOS says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/gitid.app` in Terminal and open it again.

**Windows:** unzip the download and run `gitid_<version>_x64-setup.exe`. It installs for your user only and needs no administrator rights. The installer isn't signed yet, so SmartScreen may warn: choose **More info > Run anyway**.

**Linux:** unpack the `.tar.gz`. On Debian or Ubuntu (22.04 or newer), install the small `.deb`, which also installs the web engine the app needs:

```sh
sudo apt install ./deb/gitid_*_amd64.deb
```

On other distributions, use the AppImage (it's 70–100 MB because it carries its own web engine): `chmod +x appimage/gitid_*.AppImage`, then run it.

### Using the app

- **Identity:** pick a folder to see who commits there will be from, which file (or rule, or environment variable) each value comes from, and a button to fix each problem.
- **Scan:** pick a folder (like `~/code`) to find the repositories under it whose commits would fail, would use an identity git made up, or would use something other than your default, and why (a rule, the repo's own config, an environment variable). "Open" shows any of them in Identity.
- **Rules:** add, edit and remove identities for folders (`~/work`) or servers (your Gitea). Saving checks the rule against repos in its folders; "Check a folder" tests any repo. Rules gitid didn't create (added to your config by hand or by another tool) are listed and can be removed, but not edited.
- **All config:** every key visible from a folder, each value it has in system, global and repository config (and files those include), and which one git uses. Values can be edited, removed and added. Secrets such as tokens in URLs are masked before they reach the app's window, so masked values can be removed but not edited.
- **Settings:** your default name and email, "never guess my identity" (`user.useConfigOnly`), default branch, pull behavior, editor, commit signing, which git the app uses, and the `gitid` command. Changes go to your global git config; the app's own choices are saved separately in its config folder.

## The command line

The `gitid` command does what the app does, in a terminal. It comes with the app:

- **Linux (.deb):** installed to `/usr/bin/gitid` with the app.
- **Windows:** the installer adds its folder to your PATH (and the uninstaller removes it). Open a new terminal afterwards.
- **macOS:** apps are dragged to Applications with no install step, so open gitid and choose **Settings > Command line > Install the gitid command**. It links `/usr/local/bin/gitid` to the copy inside the app, after macOS asks for your password (like VS Code's "Install 'code' command in PATH").
- **Linux AppImage:** an AppImage can't install commands; use the `.deb`, or the command-only download.

Settings > Command line shows whether a terminal will find it. On a machine without a desktop, unpack `gitid-cli-<platform>` and put `gitid` somewhere on your PATH, such as `/usr/local/bin` (or [build it](#building-from-source)).

```sh
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

## Building from source

### What you need

- **[Rust](https://rustup.rs) 1.90 or newer.** After installing it, open a new terminal so `cargo` is on your PATH, and check with `cargo --version`. Without it, `npm run tauri dev` stops at once with "failed to run 'cargo metadata' ... program not found". If Rust is already installed, run `rustup update` first: an older compiler stops with "requires rustc 1.90 or newer".
- **[Node.js](https://nodejs.org) LTS**, for the app's frontend.
- **[git](https://git-scm.com)** 2.26 or newer (2.36+ for server rules and their tests).
- **macOS:** the Xcode Command Line Tools: `xcode-select --install`.
- **Windows:** the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload (Rust needs its linker); the Rust installer offers to install them, so accept. WebView2 already ships with Windows 10 and 11.
- **Linux (Debian/Ubuntu):** the web engine and other libraries Tauri builds against:

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
      libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

  Other distributions: see Tauri's [prerequisites](https://v2.tauri.app/start/prerequisites/).

The command alone needs only Rust and git.

### Build

```sh
git clone <this repository> gitid
cd gitid

cargo install --path crates/cli         # just the command, into ~/.cargo/bin

cd app
npm ci                                  # the frontend's dependencies, from package-lock.json
npm run tauri dev                       # run the app with live reload
npm run tauri build                     # installers, in target/release/bundle/ at the repository root
```

The first `tauri dev` or `tauri build` downloads and compiles a few hundred Rust crates, which takes several minutes (longer on Windows); later runs are much faster. For much of that time the last line on screen is Vite's `Local: http://localhost:1420/`. That's the address the app window loads its screens from, not the app: opened in a browser it can only say so, because there's no gitid behind it. Wait for the gitid window to open by itself. `npm run build` on its own builds only the frontend and isn't needed first.

`npm run tauri build` makes the same installer types CI does: `macos/gitid.app` on macOS, `nsis/gitid_<version>_x64-setup.exe` on Windows, and `deb/` plus `appimage/` on Linux. On a Mac it builds for that Mac's own architecture. CI's downloads are universal (Apple silicon and Intel in one app); to build one yourself:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin   # in target/universal-apple-darwin/release/bundle/macos/
```

The bundled `gitid` command is made universal too (`scripts/build-sidecar.mjs` builds both and joins them with `lipo`). Other formats are opt-in, such as `npm run tauri build -- --bundles dmg` for a disk image (Tauri's script lays it out by driving Finder, so run it from a logged-in desktop session, not over SSH).

Both `tauri dev` and `tauri build` first build the `gitid` command into `app/src-tauri/binaries/` (`npm run sidecar`, see `scripts/build-sidecar.mjs`), so the app can ship it. If you run cargo on the app directly (`cargo clippy -p gitid-app`, `cargo test -p gitid-app`, or an editor's rust-analyzer), run `npm run sidecar` once first, or the build stops with "resource path `binaries/gitid-<target>` doesn't exist".

DevTools (right-click > Inspect Element) are always on in `tauri dev`. Release builds leave them out unless built with `npm run tauri build -- --features devtools`, which CI's test builds use; releases must not, because on macOS it relies on private APIs.

## Tests

```sh
cargo test                                   # core + CLI (the app isn't a default member)
cargo test --features gitid-core/serde       # also the JSON contract tests the app relies on
cd app && npm run typecheck                  # frontend types
```

Integration tests run real git in temp folders with `GIT_CONFIG_GLOBAL` pointed at a temp file, `GIT_CONFIG_NOSYSTEM=1` and a temp `HOME`, so your own config is never read or changed. They need git installed (2.36+ for the server-rule tests; older versions skip them).

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
  src/lib/     api.ts (typed calls to Rust), types.ts (the JSON shapes), i18n.ts (t(), plurals, language choice), messages.ts (words for errors, warnings, rules, settings), dom.ts, paths.ts, recent.ts
  src/locales/ en.json (source and fallback), zh-Hans.json; sources/ records the English each translation was made from
  scripts/     find-literals.mjs (check:strings), check-locales.mjs (check:locales), build-sidecar.mjs (builds the CLI the installers ship)
  src/views/   identity.ts, scan.ts, rules.ts, scopes.ts (All config), settings.ts
app/src-tauri/ desktop app backend: commands.rs, rule_commands.rs, scope_commands.rs, scan_commands.rs, cli_commands.rs (the only calls the UI can make), state.rs (which git and the language; saved settings.json), error.rs (errors as a code + values; the frontend words them)
  windows/     installer-hooks.nsh (adds the install folder to the user's PATH, and removes it on uninstall)
```

The backend never sends the app a sentence to show: errors, warnings, rule descriptions and setting labels arrive as codes and values, and the frontend words them in the user's language (`src/locales/`). Text from git itself (why it refused, a failed command's error) is shown as it is, under gitid's own sentence. The CLI and its `--json` output stay in English.

The app's webview gets no plugin permissions: it can only call gitid's own commands, and even the folder picker is opened from Rust. Text from config is only ever inserted as text, never as HTML.

Apart from git, the app runs one program: `osascript` on macOS, when you install or remove the `gitid` command in Settings, so macOS can ask for your password.

The rule: every read and write goes through the `git` executable. Config files are never parsed or written directly. (gitid only creates the identities folder, and deletes identity files it created itself.)

## Localisation

The app is in English and Simplified Chinese. It follows the system language (Simplified Chinese for zh, zh-CN, zh-SG, zh-Hans; English otherwise), and Settings > Language overrides it. The CLI is English only.

- **Text lives in `app/src/locales/`**: `en.json` is the source and the fallback, `zh-Hans.json` the translation. Code never contains text a person reads: it calls `t("key")`, `tn("key", count)` for plurals (`key.one` / `key.other`) or `tx("key", { name: element })` when a placeholder is an element, like a `<code>` key. Placeholders are named (`{path}`), so a translation can put them in any order.
- **The backend sends codes, not sentences.** Errors arrive as `{ kind, code, params, message, detail }`; their text is the entry `error.<code>`. git's own words (`detail`) are shown as they are.
- **Adding a string:** add it to `en.json` and `zh-Hans.json`, then run `npm run check:locales -- --update zh-Hans` to record which English the translation was made from.
- **Changing English text:** the checks then list the Chinese entries made from the old English. Update them, then run the `--update` command above.
- **Adding a language:** add `app/src/locales/<tag>.json` and its name in `LOCALES` / `LOCALE_NAMES` (`src/lib/i18n.ts`), and map its system tags in `matchLocale`.
- **Pseudo-language:** development builds (`npm run tauri dev`) offer "Pseudo (en-XA)" in Settings. It shows every entry accented and a third longer, so text that skipped the catalog stays plain English, and layouts too tight for longer words show.

Checks (all run in CI):

```sh
cd app
npm run check:strings    # no text written in the code (uses TypeScript's parser)
npm run check:locales    # every language: same entries and placeholders, right plural forms, nothing out of date
cargo test -p gitid-app  # every error code the backend can send has an English entry (run `npm run sidecar` once first)
```

`check:strings` can't tell a lone lowercase word shown as text (like "effective") from a code value, so review those by hand.

## How rules match (found while building this)

- **Server rules need four patterns per host.** `hasconfig:remote.*.url:https://host/**` misses `https://user:token@host/...`, `ssh://git@host/...` and `git@host:owner/repo.git`. gitid writes `http*://HOST/**`, `http*://*@HOST/**`, `ssh://*@HOST/**` and `*@HOST:*/**` (plus `ssh://*@HOST:PORT/**` with `--ssh-port`). The obvious `*@HOST:**` doesn't work: `**` only crosses folders next to a `/`.
- **Folder rules use the real path.** git matches a repo's real location, so a rule on a symlinked folder never applies. gitid resolves symlinks before writing the rule.
- **Folder rules are case-insensitive on Windows and macOS** (`gitdir/i:`), where `Work` and `work` are the same folder.

## Notes for later steps

- The app's bundle identifier is a placeholder (`io.github.gitid` in `app/src-tauri/tauri.conf.json`). Change it to a reverse domain you own before the first release; changing it later moves the app's data folder.
- To change the app icon, `npm run tauri icon path/to/icon.png` (a 1024×1024 PNG) regenerates everything in `app/src-tauri/icons/`.
- Steps 1–2 were built without registry access, so `gitid-core` has no required dependencies (serde is an optional feature).
- macOS: `/usr/bin/git` may be the Command Line Tools shim; detect with `xcode-select -p` before calling it (TODO in `git/locate.rs`).

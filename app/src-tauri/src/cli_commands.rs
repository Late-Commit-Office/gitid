//! The `gitid` command-line tool that ships inside the app, and whether a
//! terminal can find it.
//!
//! The installers put the CLI next to the app's own executable (`bundle >
//! externalBin` in tauri.conf.json). Linux packages install both to
//! `/usr/bin`, and the Windows installer adds its folder to the user's PATH
//! (`windows/installer-hooks.nsh`), so there it works straight away. A macOS
//! app is dragged to Applications with no install step, so, like VS Code's
//! "Install 'code' command in PATH", Settings offers to link
//! `/usr/local/bin/gitid` to the copy inside the app. macOS asks for an
//! administrator password for that itself (`osascript`).
//!
//! Besides `git` (in gitid-core), `osascript` is the only program the app
//! runs, and only on macOS, only when the user presses Install or Remove.

use std::path::{Path, PathBuf};

use gitid_core::paths;
use serde::Serialize;
use serde_json::json;

use crate::commands::blocking;
use crate::error::{CmdResult, CommandError};

/// Where the macOS app links the command. In the default PATH (`/etc/paths`)
/// on every macOS, Intel and Apple silicon.
const LINK_DIR: &str = "/usr/local/bin";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CliState {
    /// A new terminal runs the copy that comes with this app.
    Ready,
    /// macOS: not linked yet; Install links it.
    CanLink,
    /// A different `gitid` is in the way (`command`): on macOS a file at the
    /// link's place, elsewhere one that comes first on PATH.
    Taken,
    /// macOS: the app runs from a temporary or read-only place (opened
    /// straight from Downloads or a disk image), so a link would break.
    MoveFirst,
    /// Bundled, but its folder isn't on PATH.
    NotOnPath,
    /// Linux AppImage: the bundled copy only exists while the app runs.
    AppImage,
    /// This build has no bundled CLI (e.g. built without the sidecar).
    Missing,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    state: CliState,
    /// The CLI that comes with the app.
    bundled: Option<PathBuf>,
    /// `Ready`: the `gitid` a terminal finds. `Taken`: the other one.
    /// `CanLink`: where the link will go.
    command: Option<PathBuf>,
    /// Whether this app can install the command itself (macOS only).
    can_link: bool,
    /// The folder the macOS app links into.
    link_dir: PathBuf,
}

const CAN_LINK: bool = cfg!(target_os = "macos");

/// Whether the `gitid` command is installed, and where.
#[tauri::command]
pub async fn cli_status() -> CmdResult<CliStatus> {
    blocking(|| Ok(status())).await
}

/// macOS: link `/usr/local/bin/gitid` to the CLI inside the app. `replace`
/// also replaces a different `gitid` already there. `prompt` is the line
/// macOS shows above the password field, in the user's language. Returns the
/// new status; unchanged if the user cancels the password prompt.
#[tauri::command]
pub async fn install_cli(replace: bool, prompt: String) -> CmdResult<CliStatus> {
    blocking(move || {
        let now = status();
        let bundled = match (now.state, &now.bundled) {
            (CliState::Ready, _) => return Ok(now),
            (CliState::Missing, _) | (_, None) => return Err(cli_error("cliMissing", json!({}))),
            (_, Some(_)) if !CAN_LINK => return Err(cli_error("cliCantLink", json!({}))),
            (CliState::MoveFirst, _) => return Err(cli_error("cliMoveFirst", json!({}))),
            (CliState::Taken, _) if !replace => {
                return Err(cli_error("cliTaken", json!({ "path": now.command })))
            }
            (_, Some(b)) => b.clone(),
        };
        let link = link_path();
        match link_directly(&bundled, &link) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let script = "\"/bin/mkdir -p \" & quoted form of item 3 of argv & \
                     \" && /bin/ln -sfn \" & quoted form of item 1 of argv & \
                     \" \" & quoted form of item 4 of argv";
                as_admin(
                    script,
                    &[&bundled, Path::new(&prompt), Path::new(LINK_DIR), &link],
                )?;
            }
            Err(e) => return Err(link_failed(e.to_string())),
        }
        Ok(status())
    })
    .await
}

/// macOS: remove the link Install made. Leaves any other `gitid` alone.
#[tauri::command]
pub async fn remove_cli(prompt: String) -> CmdResult<CliStatus> {
    blocking(move || {
        if !CAN_LINK {
            return Err(cli_error("cliCantLink", json!({})));
        }
        if status().state != CliState::Ready {
            return Ok(status());
        }
        let link = link_path();
        match std::fs::remove_file(&link) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let script = "\"/bin/rm -f \" & quoted form of item 3 of argv";
                as_admin(script, &[Path::new(""), Path::new(&prompt), &link])?;
            }
            Err(e) => return Err(link_failed(e.to_string())),
        }
        Ok(status())
    })
    .await
}

fn link_path() -> PathBuf {
    Path::new(LINK_DIR).join("gitid")
}

/// The CLI next to the app's own executable, if this build has one.
fn bundled() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let cli = exe
        .parent()?
        .join(format!("gitid{}", std::env::consts::EXE_SUFFIX));
    cli.is_file().then_some(cli)
}

fn status() -> CliStatus {
    let bundled = bundled();
    let (state, command) = match &bundled {
        None => (CliState::Missing, None),
        Some(b) if CAN_LINK => mac_state(b),
        Some(b) => path_state(b, &path_dirs(), std::env::var_os("APPIMAGE").is_some()),
    };
    CliStatus {
        state,
        bundled,
        command,
        can_link: CAN_LINK,
        link_dir: PathBuf::from(LINK_DIR),
    }
}

/// Linux and Windows: which `gitid` a new terminal runs.
fn path_state(bundled: &Path, dirs: &[PathBuf], appimage: bool) -> (CliState, Option<PathBuf>) {
    let name = format!("gitid{}", std::env::consts::EXE_SUFFIX);
    let found = dirs.iter().map(|d| d.join(&name)).find(|p| p.is_file());
    match found {
        Some(f) if same_file(&f, bundled) => (CliState::Ready, Some(f)),
        // Another gitid (cargo install, a download) comes first.
        Some(f) if !appimage => (CliState::Taken, Some(f)),
        _ if appimage => (CliState::AppImage, None),
        _ => (CliState::NotOnPath, None),
    }
}

/// macOS: whether `/usr/local/bin/gitid` is a link to this app's CLI.
fn mac_state(bundled: &Path) -> (CliState, Option<PathBuf>) {
    let link = link_path();
    let text = bundled.to_string_lossy();
    // Opened from Downloads without moving it, macOS runs the app from a
    // random read-only copy ("App Translocation"); a disk image is read-only
    // and goes away when ejected. A link to either would break.
    if text.contains("/AppTranslocation/") || text.starts_with("/Volumes/") {
        return (CliState::MoveFirst, Some(link));
    }
    (link_state(&link, bundled), Some(link))
}

/// What's at `link`: nothing, a link to `bundled`, a stale link to some
/// gitid.app (moved or deleted; fine to replace), or something else.
fn link_state(link: &Path, bundled: &Path) -> CliState {
    let Ok(meta) = std::fs::symlink_metadata(link) else {
        return CliState::CanLink;
    };
    if !meta.file_type().is_symlink() {
        return CliState::Taken;
    }
    if same_file(link, bundled) {
        return CliState::Ready;
    }
    let points_into_an_app = std::fs::read_link(link)
        .map(|t| t.to_string_lossy().ends_with(".app/Contents/MacOS/gitid"))
        .unwrap_or(false);
    if points_into_an_app {
        CliState::CanLink
    } else {
        CliState::Taken
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (paths::canonicalize(a), paths::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Make the link without asking for a password; works when the folder is
/// writable (e.g. Homebrew made /usr/local/bin the user's on Intel Macs).
#[cfg(unix)]
fn link_directly(bundled: &Path, link: &Path) -> std::io::Result<()> {
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::remove_file(link) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::os::unix::fs::symlink(bundled, link)
}

#[cfg(not(unix))]
fn link_directly(_bundled: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

/// Run a shell command as an administrator; macOS asks for the password.
/// `body` is an AppleScript expression building the command from `args`
/// (`item 1 of argv`, ...); item 2 is always the prompt. Values travel as
/// arguments and are quoted with `quoted form of`, never pasted into the
/// script. Cancelling the prompt isn't an error.
fn as_admin(body: &str, args: &[&Path]) -> CmdResult<()> {
    let script = format!(
        "on run argv\n\
         do shell script ({body}) with prompt (item 2 of argv) with administrator privileges\n\
         end run"
    );
    let out = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(&script)
        .args(args)
        .output()
        .map_err(|e| link_failed(e.to_string()))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    // "User canceled. (-128)"
    if stderr.contains("(-128)") {
        return Ok(());
    }
    Err(link_failed(stderr.trim().to_string()))
}

fn cli_error(code: &str, params: serde_json::Value) -> CommandError {
    let message = match code {
        "cliMissing" => "this build of gitid has no command-line tool".to_string(),
        "cliCantLink" => "only the macOS app installs the gitid command itself".to_string(),
        "cliMoveFirst" => "move gitid to Applications and open it from there first".to_string(),
        "cliTaken" => format!("{} is a different gitid", params["path"]),
        _ => code.to_string(),
    };
    CommandError::cli(code, params, message)
}

fn link_failed(detail: String) -> CommandError {
    CommandError::cli(
        "cliLinkFailed",
        json!({ "dir": LINK_DIR }),
        format!("couldn't install the gitid command in {LINK_DIR}: {detail}"),
    )
    .with_detail(detail)
}

/// The folders a new terminal searches for commands.
fn path_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    if let Some(dirs) = registry::registry_path() {
        return dirs;
    }
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

/// On Windows the app's own PATH is whatever it started with (e.g. from the
/// installer, before PATH changed), so read what a new terminal will get:
/// the system PATH, then the user's, from the registry.
#[cfg(windows)]
mod registry {
    use std::path::PathBuf;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    pub fn registry_path() -> Option<Vec<PathBuf>> {
        let system = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment")
            .and_then(|k| k.get_value::<String, _>("Path"))
            .ok();
        let user = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Environment")
            .and_then(|k| k.get_value::<String, _>("Path"))
            .ok();
        if system.is_none() && user.is_none() {
            return None;
        }
        let joined = [system, user]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(";");
        Some(
            joined
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| PathBuf::from(expand(s.trim())))
                .collect(),
        )
    }

    /// Expand `%NAME%` from the app's environment, as Windows does for
    /// REG_EXPAND_SZ values. Unknown names stay as written.
    fn expand(s: &str) -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(start) = rest.find('%') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            match after.find('%') {
                Some(end) => {
                    let name = &after[..end];
                    match std::env::var(name) {
                        Ok(v) if !name.is_empty() => out.push_str(&v),
                        _ => {
                            out.push('%');
                            out.push_str(name);
                            out.push('%');
                        }
                    }
                    rest = &after[end + 1..];
                }
                None => {
                    out.push_str(&rest[start..]);
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        out
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn expands_known_names_only() {
            std::env::set_var("GITID_TEST_DIR", r"C:\x");
            assert_eq!(super::expand(r"%GITID_TEST_DIR%\bin"), r"C:\x\bin");
            assert_eq!(super::expand(r"%NOPE_GITID%\bin"), r"%NOPE_GITID%\bin");
            assert_eq!(super::expand("50%"), "50%");
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gitid-cli-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    #[test]
    fn link_states() {
        let t = temp();
        let bundled = t.join("gitid.app/Contents/MacOS/gitid");
        file(&bundled);
        let link = t.join("bin/gitid");

        assert_eq!(link_state(&link, &bundled), CliState::CanLink);

        link_directly(&bundled, &link).unwrap();
        assert_eq!(link_state(&link, &bundled), CliState::Ready);

        // A link left behind by a copy of the app that has moved: replaceable.
        std::fs::remove_file(&link).unwrap();
        symlink(t.join("Old/gitid.app/Contents/MacOS/gitid"), &link).unwrap();
        assert_eq!(link_state(&link, &bundled), CliState::CanLink);
        // Replacing it works without removing it first.
        link_directly(&bundled, &link).unwrap();
        assert_eq!(link_state(&link, &bundled), CliState::Ready);

        // Some other program: only replaced when asked.
        std::fs::remove_file(&link).unwrap();
        file(&link);
        assert_eq!(link_state(&link, &bundled), CliState::Taken);
        std::fs::remove_file(&link).unwrap();
        symlink(t.join("cargo/bin/gitid"), &link).unwrap();
        assert_eq!(link_state(&link, &bundled), CliState::Taken);

        std::fs::remove_dir_all(&t).unwrap();
    }

    #[test]
    fn path_states() {
        let t = temp();
        let app = t.join("usr/bin");
        let bundled = app.join("gitid");
        file(&bundled);
        let other = t.join("cargo/bin");
        file(&other.join("gitid"));
        let empty = t.join("empty");
        std::fs::create_dir_all(&empty).unwrap();

        assert_eq!(
            path_state(&bundled, &[empty.clone(), app.clone()], false).0,
            CliState::Ready
        );
        assert_eq!(
            path_state(&bundled, std::slice::from_ref(&empty), false).0,
            CliState::NotOnPath
        );
        let (state, found) = path_state(&bundled, &[other.clone(), app.clone()], false);
        assert_eq!(state, CliState::Taken);
        assert_eq!(found, Some(other.join("gitid")));
        // A symlink to the bundled copy counts as it.
        let linked = t.join("local/bin");
        std::fs::create_dir_all(&linked).unwrap();
        symlink(&bundled, linked.join("gitid")).unwrap();
        assert_eq!(path_state(&bundled, &[linked], false).0, CliState::Ready);
        assert_eq!(path_state(&bundled, &[empty], true).0, CliState::AppImage);

        std::fs::remove_dir_all(&t).unwrap();
    }
}

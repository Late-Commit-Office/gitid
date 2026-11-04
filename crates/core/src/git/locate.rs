use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[cfg(windows)]
const GIT_EXE: &str = "git.exe";
#[cfg(not(windows))]
const GIT_EXE: &str = "git";

/// Find the git executable.
///
/// A user-supplied path always wins (and is not second-guessed if it exists).
/// Otherwise the first existing file from [`candidates`] is used.
pub fn locate(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return if p.is_file() {
            Ok(p.to_path_buf())
        } else {
            Err(Error::GitNotFound {
                searched: vec![p.to_path_buf()],
            })
        };
    }

    let searched = candidates();
    searched
        .iter()
        .find(|p| p.is_file() && !is_apple_stub(p))
        .cloned()
        .ok_or(Error::GitNotFound { searched })
}

/// macOS: `/usr/bin/git` is Apple's stand-in, not git. It runs the git from
/// Apple's developer tools (Xcode or the Command Line Tools); when they aren't
/// installed it opens Apple's "install the command line developer tools?"
/// dialog and fails. gitid checks git as soon as it opens, so on a Mac
/// without the tools that dialog would be the first thing a new user sees,
/// unexplained. Skip the stand-in then: gitid's own "git not found" screen
/// says what to install.
///
/// Decided without running anything: the tools' folder is found the way
/// Apple's tools find it ([`developer_dir`]), then checked for a real git.
fn is_apple_stub(path: &Path) -> bool {
    if !cfg!(target_os = "macos") || path != Path::new(APPLE_STUB) {
        return false;
    }
    let dir = developer_dir(
        env::var_os("DEVELOPER_DIR").as_deref(),
        Path::new(XCODE_SELECT_LINK),
        &DEFAULT_DEVELOPER_DIRS.map(Path::new),
    );
    !dir.is_some_and(|d| has_git(&d))
}

const APPLE_STUB: &str = "/usr/bin/git";
/// The folder `xcode-select --switch` chose, as a symlink.
const XCODE_SELECT_LINK: &str = "/var/db/xcode_select_link";
/// Where the tools are when nothing was chosen: Xcode, then the Command Line Tools.
const DEFAULT_DEVELOPER_DIRS: [&str; 2] = [
    "/Applications/Xcode.app/Contents/Developer",
    "/Library/Developer/CommandLineTools",
];

/// The active developer-tools folder, in the order Apple's tools look:
/// `$DEVELOPER_DIR`, then the folder `xcode-select` chose, then the defaults.
/// `None` if none of them exists. `$DEVELOPER_DIR` can name Xcode itself
/// (`/Applications/Xcode.app`); its `Contents/Developer` is used then.
fn developer_dir(
    env_dir: Option<&OsStr>,
    select_link: &Path,
    defaults: &[&Path],
) -> Option<PathBuf> {
    if let Some(dir) = env_dir.filter(|d| !d.is_empty()) {
        let dir = PathBuf::from(dir);
        let inside_app = dir.join("Contents/Developer");
        // An explicit choice is final, even when it's wrong: Apple's tools
        // don't fall back either.
        return [inside_app, dir].into_iter().find(|d| d.is_dir());
    }
    if let Ok(chosen) = std::fs::read_link(select_link) {
        let chosen = if chosen.is_relative() {
            select_link.parent().unwrap_or(Path::new("/")).join(chosen)
        } else {
            chosen
        };
        if chosen.is_dir() {
            return Some(chosen);
        }
    }
    defaults
        .iter()
        .find(|d| d.is_dir())
        .map(|d| d.to_path_buf())
}

/// Whether a developer-tools folder has a real git in it.
fn has_git(developer_dir: &Path) -> bool {
    developer_dir.join("usr/bin/git").is_file()
}

/// Places git is looked for, in order, on this OS.
pub fn candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    // macOS: apps launched from Finder/Dock get a minimal PATH (/usr/bin:/bin:...),
    // so Homebrew's git would be missed. Check it first so the app sees the same
    // git the user's terminal does.
    #[cfg(target_os = "macos")]
    {
        out.push("/opt/homebrew/bin/git".into()); // Apple silicon
        out.push("/usr/local/bin/git".into()); // Intel Homebrew / manual installs
    }

    if let Some(path) = env::var_os("PATH") {
        out.extend(env::split_paths(&path).map(|dir| dir.join(GIT_EXE)));
    }

    #[cfg(windows)]
    {
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(base) = env::var_os(var) {
                out.push(Path::new(&base).join(r"Git\cmd\git.exe"));
            }
        }
        if let Some(base) = env::var_os("LOCALAPPDATA") {
            out.push(Path::new(&base).join(r"Programs\Git\cmd\git.exe"));
        }
    }

    // On macOS /usr/bin/git is Apple's stand-in; `locate` skips it when
    // Apple's developer tools aren't installed (see `is_apple_stub`).
    #[cfg(unix)]
    {
        out.push("/usr/bin/git".into());
        out.push("/usr/local/bin/git".into());
    }

    // Keep order, drop duplicates.
    let mut seen = Vec::new();
    out.retain(|p| {
        if seen.contains(p) {
            false
        } else {
            seen.push(p.clone());
            true
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_override_is_reported() {
        let p = Path::new("/definitely/not/here/git");
        match locate(Some(p)) {
            Err(Error::GitNotFound { searched }) => assert_eq!(searched, vec![p.to_path_buf()]),
            other => panic!("expected GitNotFound, got {other:?}"),
        }
    }

    /// A fake developer-tools folder, with or without git in it.
    fn tools(root: &Path, name: &str, with_git: bool) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("usr/bin")).unwrap();
        if with_git {
            std::fs::write(dir.join("usr/bin/git"), "").unwrap();
        }
        dir
    }

    fn temp() -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "gitid-locate-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn developer_dir_follows_apples_order() {
        let t = temp();
        let xcode = tools(&t, "Xcode.app/Contents/Developer", true);
        let clt = tools(&t, "CommandLineTools", true);
        let no_link = t.join("no-link");
        let defaults = [xcode.as_path(), clt.as_path()];

        // Nothing chosen: the first default that exists.
        assert_eq!(
            developer_dir(None, &no_link, &defaults),
            Some(xcode.clone())
        );
        assert_eq!(
            developer_dir(None, &no_link, &defaults[1..]),
            Some(clt.clone())
        );
        // No tools at all: none, so /usr/bin/git would be skipped.
        assert_eq!(
            developer_dir(None, &no_link, &[t.join("gone").as_path()]),
            None
        );

        // DEVELOPER_DIR wins, and may name Xcode.app itself.
        let app = t.join("Xcode.app");
        assert_eq!(
            developer_dir(Some(app.as_os_str()), &no_link, &[]),
            Some(xcode.clone())
        );
        assert_eq!(
            developer_dir(Some(clt.as_os_str()), &no_link, &defaults),
            Some(clt.clone())
        );
        // A wrong DEVELOPER_DIR doesn't fall back to the defaults.
        let wrong = t.join("nonexistent");
        assert_eq!(
            developer_dir(Some(wrong.as_os_str()), &no_link, &defaults),
            None
        );
        assert_eq!(
            developer_dir(Some(OsStr::new("")), &no_link, &defaults),
            Some(xcode.clone())
        );

        #[cfg(unix)]
        {
            // xcode-select's choice beats the defaults.
            let link = t.join("xcode_select_link");
            std::os::unix::fs::symlink(&clt, &link).unwrap();
            assert_eq!(developer_dir(None, &link, &defaults), Some(clt.clone()));
        }

        // A tools folder without git (a partial install) has no git.
        let empty = tools(&t, "Empty", false);
        assert!(has_git(&xcode));
        assert!(!has_git(&empty));

        std::fs::remove_dir_all(&t).unwrap();
    }

    /// The real thing, on a real Mac (CI's macOS runner has Xcode). With the
    /// tools present /usr/bin/git is used; with DEVELOPER_DIR pointing
    /// nowhere, Apple's stand-in really fails, and gitid would skip it.
    #[cfg(target_os = "macos")]
    #[test]
    fn apple_stub_on_this_mac() {
        let defaults = DEFAULT_DEVELOPER_DIRS.map(Path::new);
        let link = Path::new(XCODE_SELECT_LINK);

        let dir = developer_dir(None, link, &defaults).expect("no developer tools on this Mac");
        assert!(has_git(&dir), "{} has no usr/bin/git", dir.display());

        let nowhere = OsStr::new("/nonexistent-developer-dir");
        assert_eq!(developer_dir(Some(nowhere), link, &defaults), None);
        // The premise: without the tools, /usr/bin/git isn't a working git.
        let out = std::process::Command::new(APPLE_STUB)
            .arg("--version")
            .env("DEVELOPER_DIR", nowhere)
            .output()
            .unwrap();
        assert!(!out.status.success(), "the stand-in ran: {out:?}");
    }

    #[test]
    fn candidates_have_no_duplicates() {
        let c = candidates();
        for (i, p) in c.iter().enumerate() {
            assert!(!c[i + 1..].contains(p), "duplicate {}", p.display());
        }
    }
}

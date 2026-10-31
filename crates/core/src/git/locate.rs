use std::env;
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
        .find(|p| p.is_file())
        .cloned()
        .ok_or(Error::GitNotFound { searched })
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

    // TODO(macOS): /usr/bin/git is a shim that may pop the "install Command Line
    // Tools" dialog. Detect with `xcode-select -p` before running it.
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

    #[test]
    fn candidates_have_no_duplicates() {
        let c = candidates();
        for (i, p) in c.iter().enumerate() {
            assert!(!c[i + 1..].contains(p), "duplicate {}", p.display());
        }
    }
}

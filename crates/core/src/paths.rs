//! Path helpers that keep paths in a form git understands on every OS.

use std::io;
use std::path::{Path, PathBuf};

/// Like [`std::fs::canonicalize`], but never returns Windows "verbatim" paths.
///
/// On Windows, std returns `\\?\C:\Users\...` (and `\\?\UNC\server\share\...`).
/// Git for Windows doesn't understand that prefix, so passing such a path to
/// `git -C`, `GIT_CONFIG_GLOBAL` or an `includeIf` pattern breaks. Strip it
/// back to the ordinary `C:\Users\...` / `\\server\share\...` form.
pub fn canonicalize(p: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(p).map(simplify)
}

/// Remove a Windows verbatim prefix when the path can be written without it.
/// A no-op on other platforms.
pub fn simplify(p: PathBuf) -> PathBuf {
    if cfg!(windows) {
        if let Some(s) = p.to_str().and_then(strip_verbatim) {
            return PathBuf::from(s);
        }
    }
    p
}

/// `\\?\C:\x` -> `C:\x`, `\\?\UNC\srv\share\x` -> `\\srv\share\x`.
/// Returns `None` when there is no prefix, or when stripping would change
/// the meaning (e.g. `\\?\Volume{...}` or a non-drive device path).
fn strip_verbatim(s: &str) -> Option<String> {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return Some(format!(r"\\{rest}"));
    }
    let rest = s.strip_prefix(r"\\?\")?;
    let b = rest.as_bytes();
    let is_drive = b.len() >= 2
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b.len() == 2 || b[2] == b'\\');
    is_drive.then(|| rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_drive_and_unc_prefixes() {
        assert_eq!(
            strip_verbatim(r"\\?\C:\Users\ann\AppData").as_deref(),
            Some(r"C:\Users\ann\AppData")
        );
        assert_eq!(strip_verbatim(r"\\?\D:").as_deref(), Some(r"D:"));
        assert_eq!(
            strip_verbatim(r"\\?\UNC\nas\share\code").as_deref(),
            Some(r"\\nas\share\code")
        );
    }

    #[test]
    fn leaves_other_paths_alone() {
        assert_eq!(strip_verbatim(r"C:\Users"), None);
        assert_eq!(strip_verbatim("/home/ann"), None);
        assert_eq!(strip_verbatim(r"\\?\Volume{1234}\x"), None);
        assert_eq!(strip_verbatim(r"\\?\GLOBALROOT\Device\x"), None);
    }

    #[test]
    fn simplify_is_identity_off_windows() {
        if !cfg!(windows) {
            let p = PathBuf::from(r"\\?\C:\x");
            assert_eq!(simplify(p.clone()), p);
        }
    }
}

//! Per-OS details: home folder, case sensitivity, and how paths are written
//! inside `includeIf` conditions.

use std::path::{Path, PathBuf};

use crate::git::Git;
use crate::paths;

/// The home folder git uses (`~` in config): `HOME`, then `USERPROFILE` on Windows.
pub fn home_dir(git: &dyn Git) -> Option<PathBuf> {
    let from = |var| {
        git.env_var(var)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let home = from("HOME").or_else(|| {
        if cfg!(windows) {
            from("USERPROFILE")
        } else {
            None
        }
    })?;
    Some(paths::canonicalize(&home).unwrap_or(home))
}

/// The file `git config --global` writes to, following git's own rule:
/// `$GIT_CONFIG_GLOBAL` if set; otherwise `~/.gitconfig` if it exists;
/// otherwise the XDG file (`$XDG_CONFIG_HOME/git/config`, default
/// `~/.config/git/config`) if that exists; otherwise `~/.gitconfig`.
pub fn global_config_file(git: &dyn Git) -> Option<PathBuf> {
    let var = |name| {
        git.env_var(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(p) = var("GIT_CONFIG_GLOBAL") {
        return Some(p);
    }
    let home = home_dir(git)?;
    let dotfile = home.join(".gitconfig");
    if dotfile.exists() {
        return Some(dotfile);
    }
    let xdg = var("XDG_CONFIG_HOME")
        .unwrap_or_else(|| home.join(".config"))
        .join("git")
        .join("config");
    if xdg.exists() {
        return Some(xdg);
    }
    Some(dotfile)
}

/// Windows and (by default) macOS treat `Work` and `work` as the same folder,
/// so folder rules must match case-insensitively there or they silently fail.
pub fn case_insensitive_fs() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

/// `gitdir/i:` on case-insensitive filesystems, else `gitdir:`.
pub fn gitdir_keyword() -> &'static str {
    if case_insensitive_fs() {
        "gitdir/i:"
    } else {
        "gitdir:"
    }
}

/// Write a folder the way `includeIf "gitdir:..."` wants it:
/// forward slashes, a trailing slash (so it matches everything below),
/// and `~/` when it is inside the home folder (portable and readable).
///
/// `dir` should already be canonical: git matches against the real path,
/// so a rule on a symlinked folder would never apply.
pub fn gitdir_pattern(dir: &Path, home: Option<&Path>) -> String {
    let rel_home = home.and_then(|h| dir.strip_prefix(h).ok());
    let mut s = match rel_home {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", slashes(rest)),
        None => slashes(dir),
    };
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

/// Path with forward slashes (git accepts them everywhere, including Windows).
pub fn slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Resolve a path written in config: `~/` is the home folder, relative paths
/// are relative to the file that contains them.
pub fn resolve_config_path(
    value: &str,
    containing_file: Option<&Path>,
    home: Option<&Path>,
) -> PathBuf {
    if let (Some(rest), Some(home)) = (value.strip_prefix("~/"), home) {
        return home.join(rest);
    }
    if value == "~" {
        if let Some(home) = home {
            return home.to_path_buf();
        }
    }
    let p = PathBuf::from(value);
    match containing_file.and_then(Path::parent) {
        Some(dir) if p.is_relative() => dir.join(p),
        _ => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitdir_patterns() {
        let home = Path::new("/home/ann");
        assert_eq!(
            gitdir_pattern(Path::new("/home/ann/work"), Some(home)),
            "~/work/"
        );
        assert_eq!(gitdir_pattern(Path::new("/home/ann"), Some(home)), "~/");
        assert_eq!(
            gitdir_pattern(Path::new("/srv/code"), Some(home)),
            "/srv/code/"
        );
        assert_eq!(gitdir_pattern(Path::new("/srv/code/"), None), "/srv/code/");
        // Not fooled by a sibling folder sharing a prefix.
        assert_eq!(
            gitdir_pattern(Path::new("/home/anna/x"), Some(home)),
            "/home/anna/x/"
        );
    }

    #[test]
    fn resolves_config_paths() {
        let home = Path::new("/home/ann");
        let global = Path::new("/home/ann/.gitconfig");
        assert_eq!(
            resolve_config_path("~/.config/git/w.gitconfig", Some(global), Some(home)),
            PathBuf::from("/home/ann/.config/git/w.gitconfig")
        );
        assert_eq!(
            resolve_config_path("w.gitconfig", Some(global), Some(home)),
            PathBuf::from("/home/ann/w.gitconfig")
        );
        assert_eq!(
            resolve_config_path("/etc/x", Some(global), Some(home)),
            PathBuf::from("/etc/x")
        );
    }
}

//! Writing config, always through `git config` with an explicit target.
//!
//! git does the locking (`config.lock`), quoting and escaping. Nothing here
//! opens a config file.

use std::path::{Path, PathBuf};

use crate::error::{last_meaningful_line, Error, Invalid, Result};
use crate::git::{Git, Output};
use crate::repo;

/// Which config file a write goes to. There is deliberately no "default":
/// every write says where it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// All users on this machine (`--system`). Usually needs admin rights.
    System,
    /// The current user (`--global`): `~/.gitconfig` or `$GIT_CONFIG_GLOBAL`.
    Global,
    /// The repository containing this folder (`--local`).
    Local(PathBuf),
    /// One specific file (`--file`), e.g. an identity file used by a rule.
    File(PathBuf),
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Target::System => "system".into(),
            Target::Global => "global".into(),
            Target::Local(p) => format!("local ({})", p.display()),
            Target::File(p) => format!("file {}", p.display()),
        }
    }

    /// `(folder to run in, flags selecting the file)`.
    pub(crate) fn invocation(&self) -> Result<(Option<&Path>, Vec<&str>)> {
        Ok(match self {
            Target::System => (None, vec!["--system"]),
            Target::Global => (None, vec!["--global"]),
            Target::Local(dir) => (Some(dir.as_path()), vec!["--local"]),
            Target::File(file) => (None, vec!["--file", path_str(file)?]),
        })
    }
}

pub(crate) fn path_str(p: &Path) -> Result<&str> {
    p.to_str().ok_or_else(|| {
        Error::from(Invalid::PathNotUtf8 {
            path: p.to_path_buf(),
        })
    })
}

/// Set a single-valued key. Fails with [`Error::MultipleValues`] if the key
/// already has several values in that file, rather than guessing which to replace.
pub fn set(git: &dyn Git, target: &Target, key: &str, value: &str) -> Result<()> {
    run_write(git, target, &[], key, &[value], WriteKind::Set).map(|_| ())
}

/// Add another value to a (multi-valued) key, keeping existing ones.
pub fn add(git: &dyn Git, target: &Target, key: &str, value: &str) -> Result<()> {
    run_write(git, target, &["--add"], key, &[value], WriteKind::Set).map(|_| ())
}

/// Remove a single-valued key. `Ok(false)` if it wasn't set.
pub fn unset(git: &dyn Git, target: &Target, key: &str) -> Result<bool> {
    run_write(git, target, &["--unset"], key, &[], WriteKind::Unset)
}

/// Remove every value of a key. `Ok(false)` if it wasn't set.
pub fn unset_all(git: &dyn Git, target: &Target, key: &str) -> Result<bool> {
    run_write(git, target, &["--unset-all"], key, &[], WriteKind::Unset)
}

/// Remove exactly the values equal to `value` (other values of the key stay).
/// `Ok(false)` if no value matched.
pub fn unset_value(git: &dyn Git, target: &Target, key: &str, value: &str) -> Result<bool> {
    // `--fixed-value` would be simpler but needs git 2.30; an anchored,
    // escaped regex works everywhere.
    let pattern = format!("^{}$", escape_regex(value));
    run_write(
        git,
        target,
        &["--unset-all"],
        key,
        &[&pattern],
        WriteKind::Unset,
    )
}

/// Change every value of `key` equal to `old` into `new`, in place. Other
/// values of the key are untouched. `Ok(false)` if no value matched.
pub fn replace_value(
    git: &dyn Git,
    target: &Target,
    key: &str,
    old: &str,
    new: &str,
) -> Result<bool> {
    let pattern = format!("^{}$", escape_regex(old));
    let (dir, flags) = target.invocation()?;
    // Check first: --replace-all with no match would *add* the value instead.
    let mut get = vec!["config"];
    get.extend(flags.iter().copied());
    get.extend(["--get-all", "--", key, pattern.as_str()]);
    if !git.run(dir, &get)?.success() {
        return Ok(false);
    }
    // `key new pattern`: replace only the values matching the pattern.
    run_write(
        git,
        target,
        &["--replace-all"],
        key,
        &[new, &pattern],
        WriteKind::Set,
    )
}

#[derive(PartialEq)]
enum WriteKind {
    Set,
    Unset,
}

fn run_write(
    git: &dyn Git,
    target: &Target,
    mode: &[&str],
    key: &str,
    values: &[&str],
    kind: WriteKind,
) -> Result<bool> {
    if key.trim().is_empty() {
        return Err(Invalid::EmptyKey.into());
    }
    if let Target::Local(dir) = target {
        if repo::discover(git, dir)?.is_none() {
            return Err(Error::NotARepository(dir.clone()));
        }
    }

    let (dir, flags) = target.invocation()?;
    let mut args: Vec<&str> = vec!["config"];
    args.extend(flags);
    args.extend(mode);
    // `--` so values like "-x" aren't taken as options.
    args.push("--");
    args.push(key);
    args.extend(values);

    let out = git.run(dir, &args)?;
    if out.success() {
        return Ok(true);
    }
    classify_failure(&args, key, out, kind)
}

/// Turn git's exit codes into errors the UI can act on.
/// (`git help config`: 1 invalid key/section, 2 no section/name, 3 broken
/// file, 4 can't write, 5 missing key or multiple values, 6 bad regex.)
fn classify_failure(args: &[&str], key: &str, out: Output, kind: WriteKind) -> Result<bool> {
    let stderr = out.stderr.to_lowercase();
    match out.code {
        Some(5) if stderr.contains("multiple values") => {
            Err(Error::MultipleValues { key: key.into() })
        }
        Some(5) if kind == WriteKind::Unset => Ok(false),
        Some(1) | Some(2) if stderr.contains("key") || stderr.contains("section") => {
            let msg = last_meaningful_line(&out.stderr);
            let msg = msg.strip_prefix("error: ").unwrap_or(msg);
            Err(Invalid::BadKey {
                key: key.into(),
                detail: msg.to_string(),
            }
            .into())
        }
        _ => Err(Error::GitFailed {
            args: args.iter().map(|s| s.to_string()).collect(),
            code: out.code,
            stderr: out.stderr,
        }),
    }
}

/// Escape a literal for git's POSIX extended regex.
pub fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if r"\^$.|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_regex_metacharacters() {
        assert_eq!(escape_regex("~/a.b/(x)*"), r"~/a\.b/\(x\)\*");
        assert_eq!(escape_regex(r"C:\Users"), r"C:\\Users");
        assert_eq!(escape_regex("plain"), "plain");
    }
}

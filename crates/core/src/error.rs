use std::fmt;
use std::io;
use std::path::PathBuf;

/// Everything that can go wrong in the core.
///
/// Note that "no identity configured" is NOT an error: it is a normal answer,
/// reported through [`crate::identity::RoleIdentity::error`].
#[derive(Debug)]
pub enum Error {
    /// A local (repo) write was asked for a folder that is not in a repository.
    NotARepository(PathBuf),
    /// The installed git is too old for this feature.
    Unsupported {
        feature: &'static str,
        needs: crate::git::GitVersion,
        have: crate::git::GitVersion,
    },
    /// The key has several values; a plain set would be ambiguous.
    MultipleValues { key: String },
    /// Something the user typed can't be used (bad key, empty host, ...).
    InvalidInput(String),
    /// Filesystem work outside git (creating the identities folder).
    Io { path: PathBuf, source: io::Error },
    /// No git executable was found in any of the searched locations.
    GitNotFound { searched: Vec<PathBuf> },
    /// git exists but could not be started.
    Spawn { git: PathBuf, source: io::Error },
    /// git ran and exited with a non-zero status.
    GitFailed {
        args: Vec<String>,
        code: Option<i32>,
        stderr: String,
    },
    /// git's output was not in the shape we expected.
    Parse(String),
    /// The folder the user picked does not exist or is not a directory.
    NotADirectory(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotARepository(p) => write!(f, "not inside a git repository: {}", p.display()),
            Error::Unsupported {
                feature,
                needs,
                have,
            } => {
                write!(f, "{feature} needs git {needs} or newer (you have {have})")
            }
            Error::MultipleValues { key } => write!(
                f,
                "{key} has several values; remove the extra ones first (or use add)"
            ),
            Error::InvalidInput(msg) => f.write_str(msg),
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::GitNotFound { searched } => match searched.as_slice() {
                [] => write!(f, "git was not found"),
                [only] => write!(f, "git was not found at {}", only.display()),
                [first, ..] => write!(
                    f,
                    "git was not found (looked in {} places, e.g. {})",
                    searched.len(),
                    first.display()
                ),
            },
            Error::Spawn { git, source } => {
                write!(f, "could not start {}: {}", git.display(), source)
            }
            Error::GitFailed { args, code, stderr } => {
                let code = code.map_or_else(|| "signal".to_string(), |c| c.to_string());
                let msg = last_meaningful_line(stderr);
                write!(f, "`git {}` failed ({}): {}", args.join(" "), code, msg)
            }
            Error::Parse(msg) => write!(f, "unexpected git output: {msg}"),
            Error::NotADirectory(p) => write!(f, "not a folder: {}", p.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Spawn { source, .. } | Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// git usually puts the useful part of an error on its last `fatal:`/`error:` line.
pub(crate) fn last_meaningful_line(stderr: &str) -> &str {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines
        .iter()
        .rev()
        .find(|l| l.starts_with("fatal:") || l.starts_with("error:"))
        .or(lines.last())
        .copied()
        .unwrap_or("")
}

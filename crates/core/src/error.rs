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
        feature: Feature,
        needs: crate::git::GitVersion,
        have: crate::git::GitVersion,
    },
    /// The key has several values; a plain set would be ambiguous.
    MultipleValues { key: String },
    /// Something the user typed (or picked) can't be used.
    Invalid(Invalid),
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
            Error::Invalid(why) => write!(f, "{why}"),
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

impl From<Invalid> for Error {
    fn from(why: Invalid) -> Self {
        Error::Invalid(why)
    }
}

/// A feature that needs a newer git than the one installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub enum Feature {
    /// `hasconfig:remote.*.url:` conditions (git 2.36).
    RemoteRules,
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Feature::RemoteRules => "matching repos by remote server",
        })
    }
}

/// Why something the user typed or picked can't be used.
///
/// Each variant carries the values a message needs, so the app can phrase it
/// in any language; `Display` is the English the CLI prints.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(
        tag = "code",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Invalid {
    // ---- config keys and values
    /// A config key was empty.
    EmptyKey,
    /// git refused the key; `detail` is git's own message.
    BadKey {
        key: String,
        detail: String,
    },
    /// A path that git config can't hold (not valid UTF-8).
    PathNotUtf8 {
        path: PathBuf,
    },
    /// An edit pointed at a value that's no longer there.
    StaleValue,
    /// Repository config was asked for outside a repository.
    NeedsRepoFolder,

    // ---- managed settings
    /// Not one of the settings gitid manages.
    UnknownSetting {
        key: String,
    },
    NotBool {
        key: String,
    },
    NotOneOf {
        key: String,
        allowed: Vec<String>,
    },
    LineBreak {
        key: String,
    },
    /// `user.name` can't contain `<` or `>` (git uses them around the email).
    NameAngleBrackets,
    BadEmail {
        email: String,
    },
    BadBranchName {
        name: String,
    },

    // ---- rules
    BadRuleId {
        id: String,
    },
    /// A rule needs a name, an email, or both.
    RuleNeedsIdentity,
    /// A rule's name can't be empty or contain `<`, `>` or newlines.
    BadRuleName,
    /// A rule needs at least one folder or server.
    RuleNeedsWhere,
    EmptyPattern,
    EmptyCondition,
    BadServer {
        input: String,
    },
    BadPort {
        input: String,
    },
    /// No rule with this id; `known` lists the ids there are.
    NoSuchRule {
        id: String,
        known: Vec<String>,
    },
    /// HOME isn't set, so `~` can't be resolved.
    NoHome,

    // ---- which git
    /// The chosen program ran but isn't git.
    NotGit {
        path: PathBuf,
    },
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::EmptyKey => f.write_str("config key is empty"),
            Invalid::BadKey { detail, .. } => f.write_str(detail),
            Invalid::PathNotUtf8 { path } => {
                write!(f, "path is not valid UTF-8: {}", path.display())
            }
            Invalid::StaleValue => f.write_str(
                "that value has changed since the list was loaded; it has been refreshed",
            ),
            Invalid::NeedsRepoFolder => f.write_str("choose a folder inside a repository first"),
            Invalid::UnknownSetting { key } => {
                write!(f, "{key} is not one of the settings gitid manages")
            }
            Invalid::NotBool { key } => write!(f, "{key} must be true or false"),
            Invalid::NotOneOf { key, allowed } => {
                write!(f, "{key} must be one of: {}", allowed.join(", "))
            }
            Invalid::LineBreak { key } => {
                let label = crate::settings::find(key).map_or(key.as_str(), |s| s.label);
                write!(f, "{label} can't contain line breaks")
            }
            Invalid::NameAngleBrackets => f.write_str("Name can't contain < or >"),
            Invalid::BadEmail { email } => write!(f, "{email:?} doesn't look like an email address"),
            Invalid::BadBranchName { name } => write!(f, "{name:?} isn't a valid branch name"),
            Invalid::BadRuleId { id } => write!(
                f,
                "rule id {id:?} must be letters, digits, '-' or '_' (e.g. work, gitea)"
            ),
            Invalid::RuleNeedsIdentity => f.write_str("a rule needs a name, an email, or both"),
            Invalid::BadRuleName => f.write_str("name can't be empty or contain <, > or newlines"),
            Invalid::RuleNeedsWhere => f.write_str("a rule needs at least one folder or server"),
            Invalid::EmptyPattern => f.write_str("remote pattern is empty"),
            Invalid::EmptyCondition => f.write_str("condition is empty"),
            Invalid::BadServer { input } => write!(
                f,
                "{input:?} is not a server name (expected something like gitea.example.com or gitea.example.com:3000)"
            ),
            Invalid::BadPort { input } => write!(f, "bad port in {input:?}"),
            Invalid::NoSuchRule { id, known } if known.is_empty() => {
                write!(f, "no rule {id:?}: there are no rules yet")
            }
            Invalid::NoSuchRule { id, known } => {
                write!(f, "no rule {id:?} (rules: {})", known.join(", "))
            }
            Invalid::NoHome => f.write_str("can't find your home folder (HOME is not set)"),
            Invalid::NotGit { path } => write!(f, "{} doesn't look like git", path.display()),
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
pub fn last_meaningful_line(stderr: &str) -> &str {
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

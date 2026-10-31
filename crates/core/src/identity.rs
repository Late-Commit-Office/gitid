//! The effective commit identity for a folder, and where each part came from.
//!
//! git itself decides the final answer (`git var GIT_AUTHOR_IDENT`), so env
//! overrides, includes and `includeIf` are all accounted for. The config
//! snapshot is only used to *explain* that answer.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::{self, ConfigSnapshot, Entry, Scope};
use crate::error::{last_meaningful_line, Error, Result};
use crate::git::Git;
use crate::repo::{self, RepoLocation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Author,
    Committer,
}

impl Role {
    pub fn label(&self) -> &'static str {
        match self {
            Role::Author => "author",
            Role::Committer => "committer",
        }
    }
    fn upper(&self) -> &'static str {
        match self {
            Role::Author => "AUTHOR",
            Role::Committer => "COMMITTER",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Name,
    Email,
}

impl Part {
    pub fn label(&self) -> &'static str {
        match self {
            Part::Name => "name",
            Part::Email => "email",
        }
    }
}

/// Why a name or email has the value it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// An environment variable such as `GIT_AUTHOR_NAME` or `EMAIL`.
    /// A GUI can't see the user's shell environment, so terminals may differ.
    Env { var: String },
    /// A config entry (which carries its scope and file).
    Config(Entry),
    /// Nothing configured: git made it up from the OS login and hostname.
    /// This is the "Gitea shows my computer username" bug.
    Guessed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub value: String,
    pub source: Source,
}

/// The identity git will use for one role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleIdentity {
    pub role: Role,
    /// `None` only when git refused to produce an identity and nothing is configured.
    pub name: Option<Field>,
    pub email: Option<Field>,
    /// Set when git would refuse to commit (message from git, `fatal:` stripped).
    pub error: Option<String>,
}

impl RoleIdentity {
    pub fn is_usable(&self) -> bool {
        self.error.is_none()
    }

    pub fn field(&self, part: Part) -> Option<&Field> {
        match part {
            Part::Name => self.name.as_ref(),
            Part::Email => self.email.as_ref(),
        }
    }

    /// `Name <email>` as it would appear in a commit, if git can produce one.
    pub fn display(&self) -> Option<String> {
        match (&self.error, &self.name, &self.email) {
            (None, Some(n), Some(e)) => Some(format!("{} <{}>", n.value, e.value)),
            _ => None,
        }
    }
}

/// An environment variable that changes what git does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
    /// Can carry credentials (e.g. `GIT_CONFIG_PARAMETERS`); mask before showing.
    pub sensitive: bool,
}

/// Everything the identity view needs for one folder.
#[derive(Debug, Clone)]
pub struct Identity {
    pub folder: PathBuf,
    pub repo: Option<RepoLocation>,
    pub author: RoleIdentity,
    pub committer: RoleIdentity,
    /// `user.useConfigOnly`: git refuses to guess when this is true.
    pub use_config_only: bool,
    pub env: Vec<EnvVar>,
}

const IDENTITY_ENV: &[&str] = &[
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "EMAIL",
];

const CONFIG_ENV: &[(&str, bool)] = &[
    ("GIT_CONFIG_GLOBAL", false),
    ("GIT_CONFIG_SYSTEM", false),
    ("GIT_CONFIG_NOSYSTEM", false),
    ("GIT_CONFIG", false),
    ("GIT_CONFIG_COUNT", false),
    ("GIT_CONFIG_PARAMETERS", true),
    ("GIT_DIR", false),
    ("GIT_WORK_TREE", false),
];

/// Work out who commits made in `folder` will be from.
pub fn effective_identity(git: &dyn Git, folder: &Path) -> Result<Identity> {
    if !folder.is_dir() {
        return Err(Error::NotADirectory(folder.to_path_buf()));
    }

    let repo = repo::discover(git, folder)?;
    let snapshot = config::read_all_in(git, Some(folder), repo.as_ref())?;

    let author = role_identity(git, folder, &snapshot, Role::Author)?;
    let committer = role_identity(git, folder, &snapshot, Role::Committer)?;

    let mut env = Vec::new();
    for name in IDENTITY_ENV {
        if let Some(value) = git.env_var(name) {
            env.push(EnvVar {
                name: name.to_string(),
                value,
                sensitive: false,
            });
        }
    }
    for (name, sensitive) in CONFIG_ENV {
        if let Some(value) = git.env_var(name) {
            env.push(EnvVar {
                name: name.to_string(),
                value,
                sensitive: *sensitive,
            });
        }
    }

    Ok(Identity {
        folder: folder.to_path_buf(),
        repo,
        author,
        committer,
        use_config_only: snapshot.get_bool("user.useConfigOnly").unwrap_or(false),
        env,
    })
}

fn role_identity(
    git: &dyn Git,
    folder: &Path,
    snapshot: &ConfigSnapshot,
    role: Role,
) -> Result<RoleIdentity> {
    let var = format!("GIT_{}_IDENT", role.upper());
    let out = git.run(Some(folder), &["var", &var])?;

    if out.success() {
        let line = out.stdout_str();
        let (name, email) =
            parse_ident(&line).ok_or_else(|| Error::Parse(format!("{var} printed {line:?}")))?;
        return Ok(RoleIdentity {
            role,
            name: Some(Field {
                value: name,
                source: source_of(git, snapshot, role, Part::Name),
            }),
            email: Some(Field {
                value: email,
                source: source_of(git, snapshot, role, Part::Email),
            }),
            error: None,
        });
    }

    // git refused. Show whatever IS configured, so the user sees what's missing.
    let configured = |part| match source_of(git, snapshot, role, part) {
        Source::Guessed => None,
        source => {
            let value = match &source {
                Source::Env { var } => git.env_var(var).unwrap_or_default(),
                Source::Config(e) => e.value.clone().unwrap_or_default(),
                Source::Guessed => unreachable!(),
            };
            Some(Field { value, source })
        }
    };
    let msg = last_meaningful_line(&out.stderr);
    Ok(RoleIdentity {
        role,
        name: configured(Part::Name),
        email: configured(Part::Email),
        error: Some(msg.strip_prefix("fatal: ").unwrap_or(msg).to_string()),
    })
}

/// Mirror git's own precedence (ident.c) to explain where a value came from:
/// `GIT_<ROLE>_<PART>` env, then `<role>.<part>`, then `user.<part>`,
/// then (email only) the `EMAIL` env var, then a guess.
fn source_of(git: &dyn Git, snapshot: &ConfigSnapshot, role: Role, part: Part) -> Source {
    let env_var = format!("GIT_{}_{}", role.upper(), part.label().to_ascii_uppercase());
    if git.env_var(&env_var).is_some() {
        return Source::Env { var: env_var };
    }
    for key in [
        format!("{}.{}", role.label(), part.label()),
        format!("user.{}", part.label()),
    ] {
        if let Some(entry) = snapshot.get(&key) {
            return Source::Config(entry.clone());
        }
    }
    if part == Part::Email && git.env_var("EMAIL").is_some() {
        return Source::Env {
            var: "EMAIL".into(),
        };
    }
    Source::Guessed
}

/// Parse `Name <email> 1700000000 +0000`.
fn parse_ident(line: &str) -> Option<(String, String)> {
    let line = line.trim_end_matches(['\n', '\r']);
    let lt = line.find('<')?;
    let gt = lt + line[lt..].find('>')?;
    Some((
        line[..lt].trim_end().to_string(),
        line[lt + 1..gt].to_string(),
    ))
}

// ---------------------------------------------------------------- warnings

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Commits will fail or carry the wrong identity.
    Problem,
    /// Worth knowing; may be intentional.
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// git would refuse to commit.
    Missing { role: Role, message: String },
    /// git is inventing a value from the OS login / hostname.
    Guessed { role: Role, part: Part },
    /// Set by an environment variable this process sees; other terminals may differ.
    FromEnv { role: Role, part: Part, var: String },
    /// Set through `git -c` / `GIT_CONFIG_COUNT` rather than a file.
    Injected { role: Role, part: Part, key: String },
    /// Author and committer would differ.
    AuthorCommitterDiffer,
    /// Nothing stops git from guessing if config is ever missing.
    GuessingAllowed,
}

impl Warning {
    pub fn severity(&self) -> Severity {
        match self {
            Warning::Missing { .. } | Warning::Guessed { .. } => Severity::Problem,
            _ => Severity::Note,
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::Missing { role, message } => {
                write!(f, "git will refuse to commit: no usable {} identity ({message})", role.label())
            }
            Warning::Guessed { role, part } => write!(
                f,
                "{} {} is not configured; git is guessing it from your OS login",
                role.label(),
                part.label()
            ),
            Warning::FromEnv { role, part, var } => write!(
                f,
                "{} {} comes from the environment variable {var}; other terminals and apps may differ",
                role.label(),
                part.label()
            ),
            Warning::Injected { role, part, key } => write!(
                f,
                "{} {} is set by `git -c` or GIT_CONFIG_* variables ({key}), not a config file",
                role.label(),
                part.label()
            ),
            Warning::AuthorCommitterDiffer => {
                f.write_str("author and committer differ in this folder")
            }
            Warning::GuessingAllowed => f.write_str(
                "user.useConfigOnly is off, so git will silently guess an identity if config is missing",
            ),
        }
    }
}

impl Identity {
    pub fn roles(&self) -> [&RoleIdentity; 2] {
        [&self.author, &self.committer]
    }

    /// Author and committer have the same values (sources may differ).
    pub fn author_matches_committer(&self) -> bool {
        let v = |r: &RoleIdentity, p| r.field(p).map(|f| f.value.clone());
        self.author.error == self.committer.error
            && v(&self.author, Part::Name) == v(&self.committer, Part::Name)
            && v(&self.author, Part::Email) == v(&self.committer, Part::Email)
    }

    /// Problems first, then notes. Duplicates across roles are collapsed
    /// when author and committer agree.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out = Vec::new();
        let roles: &[&RoleIdentity] = if self.author_matches_committer() {
            &[&self.author]
        } else {
            out.push(Warning::AuthorCommitterDiffer);
            &[&self.author, &self.committer]
        };

        for r in roles {
            if let Some(message) = &r.error {
                out.push(Warning::Missing {
                    role: r.role,
                    message: message.clone(),
                });
            }
            for part in [Part::Name, Part::Email] {
                let Some(field) = r.field(part) else { continue };
                match &field.source {
                    Source::Guessed => out.push(Warning::Guessed { role: r.role, part }),
                    Source::Env { var } => out.push(Warning::FromEnv {
                        role: r.role,
                        part,
                        var: var.clone(),
                    }),
                    Source::Config(e) if e.scope == Scope::Command => out.push(Warning::Injected {
                        role: r.role,
                        part,
                        key: e.key.clone(),
                    }),
                    Source::Config(_) => {}
                }
            }
        }

        if !self.use_config_only {
            out.push(Warning::GuessingAllowed);
        }
        out.sort_by_key(Warning::severity);
        out
    }

    pub fn has_problems(&self) -> bool {
        self.warnings()
            .iter()
            .any(|w| w.severity() == Severity::Problem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ident_lines() {
        assert_eq!(
            parse_ident("Ann Lee <ann@example.com> 1793487560 +0000\n"),
            Some(("Ann Lee".into(), "ann@example.com".into()))
        );
        assert_eq!(
            parse_ident(" <x@y> 1 +0000"),
            Some(("".into(), "x@y".into()))
        );
        assert_eq!(parse_ident("garbage"), None);
    }
}

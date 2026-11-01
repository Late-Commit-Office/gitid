//! The short list of global settings gitid manages, with labels, help text
//! and validation, so the app and the CLI describe and check them the same way.
//!
//! Everything here reads and writes the global config. Other keys can still
//! be changed with [`crate::config::set`]; they just get no special treatment.

use crate::config::{self, Origin, Target};
use crate::error::{Error, Result};
use crate::git::Git;

/// How a setting is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(tag = "kind", rename_all = "camelCase")
)]
pub enum Kind {
    /// On/off. Setting it back to git's default removes the line.
    Bool { default: bool },
    /// Free text; empty removes the line.
    Text { placeholder: &'static str },
    /// One of a fixed set of values.
    Choice {
        choices: &'static [Choice],
        default: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// Where a setting is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub enum Group {
    Identity,
    Defaults,
    Signing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub group: Group,
    pub kind: Kind,
}

const PULL_CHOICES: &[Choice] = &[
    Choice {
        value: "false",
        label: "Merge",
    },
    Choice {
        value: "true",
        label: "Rebase",
    },
    Choice {
        value: "merges",
        label: "Rebase, keeping merge commits",
    },
];

const GPG_FORMATS: &[Choice] = &[
    Choice {
        value: "openpgp",
        label: "GPG",
    },
    Choice {
        value: "ssh",
        label: "SSH key",
    },
    Choice {
        value: "x509",
        label: "X.509 certificate",
    },
];

/// The settings gitid manages, in display order.
pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "user.name",
        label: "Name",
        help: "Used for commits wherever no rule or repository sets one.",
        group: Group::Identity,
        kind: Kind::Text { placeholder: "Your Name" },
    },
    Setting {
        key: "user.email",
        label: "Email",
        help: "Used for commits wherever no rule or repository sets one.",
        group: Group::Identity,
        kind: Kind::Text { placeholder: "you@example.com" },
    },
    Setting {
        key: "user.useConfigOnly",
        label: "Never guess my identity",
        help: "When no name or email is configured, git refuses to commit instead of making one up from your computer's login.",
        group: Group::Identity,
        kind: Kind::Bool { default: false },
    },
    Setting {
        key: "init.defaultBranch",
        label: "Default branch name",
        help: "The first branch of new repositories. git's own default is master.",
        group: Group::Defaults,
        kind: Kind::Text { placeholder: "main" },
    },
    Setting {
        key: "pull.rebase",
        label: "When pulling",
        help: "How git pull combines your local commits with new ones from the remote.",
        group: Group::Defaults,
        kind: Kind::Choice { choices: PULL_CHOICES, default: "false" },
    },
    Setting {
        key: "core.editor",
        label: "Editor",
        help: "The program git opens for commit messages, for example nano, vim or code --wait.",
        group: Group::Defaults,
        kind: Kind::Text { placeholder: "nano" },
    },
    Setting {
        key: "commit.gpgSign",
        label: "Sign every commit",
        help: "Adds a signature to each commit, which Gitea and GitHub show as verified.",
        group: Group::Signing,
        kind: Kind::Bool { default: false },
    },
    Setting {
        key: "gpg.format",
        label: "Signature type",
        help: "What kind of key signs your commits.",
        group: Group::Signing,
        kind: Kind::Choice { choices: GPG_FORMATS, default: "openpgp" },
    },
    Setting {
        key: "user.signingKey",
        label: "Signing key",
        help: "A GPG key ID, or for SSH signing, the path to your public key (e.g. ~/.ssh/id_ed25519.pub).",
        group: Group::Signing,
        kind: Kind::Text { placeholder: "" },
    },
];

pub fn find(key: &str) -> Option<&'static Setting> {
    let key = config::normalize_key(key);
    SETTINGS
        .iter()
        .find(|s| config::normalize_key(s.key) == key)
}

/// A setting and its current value in the global config.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SettingValue {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub setting: Setting,
    /// `None` when the global config doesn't set it (git's default applies).
    pub value: Option<String>,
    pub origin: Option<Origin>,
}

/// Current global values of every managed setting.
pub fn read(git: &dyn Git) -> Result<Vec<SettingValue>> {
    let entries = config::read_target(git, &Target::Global)?;
    Ok(SETTINGS
        .iter()
        .map(|s| {
            let last = entries.iter().rev().find(|e| e.key_matches(s.key));
            SettingValue {
                setting: *s,
                // A bare `key` line (no `=`) means true for booleans.
                value: last.map(|e| e.value.clone().unwrap_or_else(|| "true".into())),
                origin: last.map(|e| e.origin.clone()),
            }
        })
        .collect())
}

/// Check a value and put it in the form that gets written.
/// `Ok(None)` means "remove the line" (empty text, or git's default).
pub fn normalize(git: &dyn Git, setting: &Setting, value: &str) -> Result<Option<String>> {
    let v = value.trim();
    let bad = |msg: String| Err(Error::InvalidInput(msg));
    match setting.kind {
        Kind::Bool { default } => {
            let b = match config::parse_bool(v) {
                Some(b) if !v.is_empty() => b,
                _ => return bad(format!("{} must be true or false", setting.key)),
            };
            Ok((b != default).then(|| b.to_string()))
        }
        Kind::Choice { choices, default } => {
            if !choices.iter().any(|c| c.value == v) {
                let allowed: Vec<&str> = choices.iter().map(|c| c.value).collect();
                return bad(format!(
                    "{} must be one of: {}",
                    setting.key,
                    allowed.join(", ")
                ));
            }
            Ok((v != default).then(|| v.to_string()))
        }
        Kind::Text { .. } => {
            if v.is_empty() {
                return Ok(None);
            }
            if v.contains(['\n', '\r']) {
                return bad(format!("{} can't contain line breaks", setting.label));
            }
            match setting.key {
                "user.name" if v.contains(['<', '>']) => bad("Name can't contain < or >".into()),
                "user.email"
                    if !v.contains('@')
                        || v.contains(char::is_whitespace)
                        || v.contains(['<', '>']) =>
                {
                    bad(format!("{v:?} doesn't look like an email address"))
                }
                "init.defaultBranch" => {
                    let ok = git
                        .run(None, &["check-ref-format", "--branch", v])?
                        .success();
                    if ok {
                        Ok(Some(v.to_string()))
                    } else {
                        bad(format!("{v:?} isn't a valid branch name"))
                    }
                }
                _ => Ok(Some(v.to_string())),
            }
        }
    }
}

/// Change a managed setting in the global config. `None` (or a value that
/// normalizes to "remove") deletes it, so git's default applies again.
pub fn apply(git: &dyn Git, key: &str, value: Option<&str>) -> Result<()> {
    let setting = find(key).ok_or_else(|| {
        Error::InvalidInput(format!("{key} is not one of the settings gitid manages"))
    })?;
    let normalized = match value {
        Some(v) => normalize(git, setting, v)?,
        None => None,
    };
    match normalized {
        // unset_all: a hand-edited file might repeat the key.
        None => config::unset_all(git, &Target::Global, setting.key).map(|_| ()),
        Some(v) => match config::set(git, &Target::Global, setting.key, &v) {
            Err(Error::MultipleValues { .. }) => {
                config::unset_all(git, &Target::Global, setting.key)?;
                config::set(git, &Target::Global, setting.key, &v)
            }
            other => other,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_findable() {
        for (i, s) in SETTINGS.iter().enumerate() {
            assert!(
                SETTINGS[i + 1..].iter().all(|o| o.key != s.key),
                "{}",
                s.key
            );
            assert_eq!(find(&s.key.to_uppercase()).map(|f| f.key), Some(s.key));
        }
        assert!(find("core.nope").is_none());
    }

    #[test]
    fn choice_defaults_are_choices() {
        for s in SETTINGS {
            if let Kind::Choice { choices, default } = s.kind {
                assert!(choices.iter().any(|c| c.value == default), "{}", s.key);
            }
        }
    }
}

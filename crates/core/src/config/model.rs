use std::fmt;
use std::path::{Path, PathBuf};

/// Which layer a config value comes from. Later layers win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "lowercase")
)]
pub enum Scope {
    System,
    Global,
    Local,
    Worktree,
    /// `git -c`, or injected through `GIT_CONFIG_COUNT`/`GIT_CONFIG_PARAMETERS`.
    Command,
    /// Older git without `--show-scope`, or something new we don't know.
    Unknown,
}

impl Scope {
    pub fn parse(s: &str) -> Scope {
        match s {
            "system" => Scope::System,
            "global" => Scope::Global,
            "local" => Scope::Local,
            "worktree" => Scope::Worktree,
            "command" => Scope::Command,
            _ => Scope::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Scope::System => "system",
            Scope::Global => "global",
            Scope::Local => "local",
            Scope::Worktree => "worktree",
            Scope::Command => "command",
            Scope::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Where exactly a value was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(tag = "kind", content = "value", rename_all = "camelCase")
)]
pub enum Origin {
    /// A config file. Always absolute once parsed.
    File(PathBuf),
    /// A blob in the repository (`include.path` pointing at a ref).
    Blob(String),
    /// `git -c` or the `GIT_CONFIG_*` environment variables.
    CommandLine,
    Stdin,
    Other(String),
}

impl Origin {
    /// Parse a `--show-origin` field. Relative file paths are resolved
    /// against `base` (the folder git ran in after repository discovery).
    pub fn parse(s: &str, base: Option<&Path>) -> Origin {
        if let Some(path) = s.strip_prefix("file:") {
            let p = PathBuf::from(path);
            let p = match base {
                Some(base) if p.is_relative() => base.join(p),
                _ => p,
            };
            Origin::File(p)
        } else if let Some(blob) = s.strip_prefix("blob:") {
            Origin::Blob(blob.to_string())
        } else if s.starts_with("command line:") {
            Origin::CommandLine
        } else if s.starts_with("standard input:") {
            Origin::Stdin
        } else {
            Origin::Other(s.to_string())
        }
    }

    pub fn file(&self) -> Option<&Path> {
        match self {
            Origin::File(p) => Some(p),
            _ => None,
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Origin::File(p) => write!(f, "{}", p.display()),
            Origin::Blob(b) => write!(f, "blob {b}"),
            Origin::CommandLine => f.write_str("command line / GIT_CONFIG_* environment"),
            Origin::Stdin => f.write_str("standard input"),
            Origin::Other(s) => f.write_str(s),
        }
    }
}

/// One `key = value` line as git resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Entry {
    /// As git prints it: section and name lowercased, subsection kept as-is.
    pub key: String,
    /// `None` for a bare `key` line with no `=` (means "true" for booleans).
    pub value: Option<String>,
    pub scope: Scope,
    pub origin: Origin,
}

impl Entry {
    pub fn key_matches(&self, key: &str) -> bool {
        self.key == normalize_key(key)
    }

    /// Interpret the value as a git boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self.value.as_deref() {
            None => Some(true),
            Some(v) => parse_bool(v),
        }
    }
}

/// Normalize a key the way git prints it: `Section.Sub.Name` -> `section.Sub.name`.
pub fn normalize_key(key: &str) -> String {
    match (key.find('.'), key.rfind('.')) {
        (Some(first), Some(last)) if first != last => format!(
            "{}{}{}",
            key[..first].to_ascii_lowercase(),
            &key[first..last],
            key[last..].to_ascii_lowercase()
        ),
        _ => key.to_ascii_lowercase(),
    }
}

pub fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" | "" => Some(false),
        _ => None,
    }
}

/// Every config entry visible from one folder, in git's order (later wins).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ConfigSnapshot {
    pub entries: Vec<Entry>,
}

impl ConfigSnapshot {
    /// The value git would use for a single-valued key: the last one.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        let key = normalize_key(key);
        self.entries.iter().rev().find(|e| e.key == key)
    }

    /// All values for a key, in order. The last is the effective one.
    pub fn get_all(&self, key: &str) -> Vec<&Entry> {
        let key = normalize_key(key);
        self.entries.iter().filter(|e| e.key == key).collect()
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Entry::as_bool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_keys_like_git() {
        assert_eq!(normalize_key("User.Name"), "user.name");
        assert_eq!(normalize_key("Remote.Origin.URL"), "remote.Origin.url");
        assert_eq!(
            normalize_key("includeIf.gitdir:~/Work/.path"),
            "includeif.gitdir:~/Work/.path"
        );
        assert_eq!(normalize_key("weird"), "weird");
    }

    #[test]
    fn origin_resolves_relative_files() {
        let base = Path::new("/repo");
        assert_eq!(
            Origin::parse("file:.git/config", Some(base)),
            Origin::File(PathBuf::from("/repo/.git/config"))
        );
        assert_eq!(
            Origin::parse("file:/home/a/.gitconfig", Some(base)),
            Origin::File(PathBuf::from("/home/a/.gitconfig"))
        );
        assert_eq!(Origin::parse("command line:", None), Origin::CommandLine);
        assert_eq!(
            Origin::parse("blob:HEAD:x", None),
            Origin::Blob("HEAD:x".into())
        );
    }

    #[test]
    fn last_value_wins() {
        let e = |v: &str, scope| Entry {
            key: "user.name".into(),
            value: Some(v.into()),
            scope,
            origin: Origin::Other(String::new()),
        };
        let snap = ConfigSnapshot {
            entries: vec![e("global", Scope::Global), e("local", Scope::Local)],
        };
        assert_eq!(
            snap.get("User.Name").unwrap().value.as_deref(),
            Some("local")
        );
        assert_eq!(snap.get_all("user.name").len(), 2);
    }
}

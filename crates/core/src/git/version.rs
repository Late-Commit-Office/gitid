use std::fmt;

use super::Git;
use crate::error::{Error, Result};

/// A git version, compared numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    /// Parse `git --version` output, e.g.
    /// `git version 2.43.0`, `git version 2.39.3 (Apple Git-146)`,
    /// `git version 2.45.1.windows.1`, `git version 2.50.0.rc1`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.strip_prefix("git version ").unwrap_or(s);
        let token = s.split_whitespace().next()?;
        let mut nums = token.split('.').map(leading_number);
        let major = nums.next()??;
        let minor = nums.next()??;
        let patch = nums.next().flatten().unwrap_or(0);
        Some(GitVersion::new(major, minor, patch))
    }

    pub fn detect(git: &dyn Git) -> Result<Self> {
        let out = git.run_ok(None, &["--version"])?;
        let text = out.stdout_str();
        GitVersion::parse(&text).ok_or_else(|| Error::Parse(format!("git --version said {text:?}")))
    }

    /// `git config --show-scope` (git 2.26).
    pub fn supports_show_scope(&self) -> bool {
        *self >= GitVersion::new(2, 26, 0)
    }

    /// `includeIf "hasconfig:remote.*.url:..."` (git 2.36).
    pub fn supports_hasconfig(&self) -> bool {
        *self >= GitVersion::new(2, 36, 0)
    }

    /// `author.name` / `committer.name` etc. (git 2.22).
    pub fn supports_role_identity(&self) -> bool {
        *self >= GitVersion::new(2, 22, 0)
    }
}

fn leading_number(part: &str) -> Option<u32> {
    let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_world_strings() {
        let cases = [
            ("git version 2.43.0\n", (2, 43, 0)),
            ("git version 2.39.3 (Apple Git-146)", (2, 39, 3)),
            ("git version 2.45.1.windows.1", (2, 45, 1)),
            ("git version 2.50.0.rc1", (2, 50, 0)),
            ("git version 2.7", (2, 7, 0)),
        ];
        for (s, (a, b, c)) in cases {
            assert_eq!(GitVersion::parse(s), Some(GitVersion::new(a, b, c)), "{s}");
        }
        assert_eq!(GitVersion::parse("hello"), None);
    }

    #[test]
    fn feature_gates() {
        let old = GitVersion::new(2, 25, 9);
        let mid = GitVersion::new(2, 30, 0);
        let new = GitVersion::new(2, 43, 0);
        assert!(!old.supports_show_scope());
        assert!(mid.supports_show_scope() && !mid.supports_hasconfig());
        assert!(new.supports_hasconfig());
        assert!(GitVersion::new(3, 0, 0) > new);
    }
}

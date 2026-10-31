use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// Raw result of one git invocation.
#[derive(Debug, Clone)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

/// The one seam between the core and the outside world.
///
/// Every module takes `&dyn Git`, so tests (and later the Tauri app) can run
/// the same code against an isolated environment.
pub trait Git {
    /// Run `git [-C dir] args...`. A non-zero exit is NOT an error here;
    /// use [`Git::run_ok`] when it should be.
    fn run(&self, dir: Option<&Path>, args: &[&str]) -> Result<Output>;

    /// The value of an environment variable *as git will see it*.
    /// Used to explain identities that come from the environment.
    fn env_var(&self, name: &str) -> Option<String>;

    fn run_ok(&self, dir: Option<&Path>, args: &[&str]) -> Result<Output> {
        let out = self.run(dir, args)?;
        if out.success() {
            Ok(out)
        } else {
            Err(Error::GitFailed {
                args: args.iter().map(|s| s.to_string()).collect(),
                code: out.code,
                stderr: out.stderr,
            })
        }
    }
}

/// Runs the real git binary.
#[derive(Debug, Clone)]
pub struct SystemGit {
    path: PathBuf,
    set: Vec<(OsString, OsString)>,
    removed: Vec<OsString>,
}

impl SystemGit {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        SystemGit {
            path: path.into(),
            set: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Find git (honoring an optional user override) and wrap it.
    pub fn locate(override_path: Option<&Path>) -> Result<Self> {
        super::locate(override_path).map(SystemGit::new)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Set an environment variable for every git call.
    pub fn with_env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        let key = key.as_ref().to_os_string();
        self.removed.retain(|k| *k != key);
        self.set.retain(|(k, _)| *k != key);
        self.set.push((key, value.as_ref().to_os_string()));
        self
    }

    /// Remove an environment variable for every git call.
    pub fn without_env(mut self, key: impl AsRef<OsStr>) -> Self {
        let key = key.as_ref().to_os_string();
        self.set.retain(|(k, _)| *k != key);
        if !self.removed.contains(&key) {
            self.removed.push(key);
        }
        self
    }
}

impl Git for SystemGit {
    fn run(&self, dir: Option<&Path>, args: &[&str]) -> Result<Output> {
        let mut cmd = Command::new(&self.path);
        if let Some(dir) = dir {
            cmd.arg("-C").arg(dir);
        }
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Stable English messages, never prompt, never take the index lock on reads.
            .env("LC_ALL", "C")
            .env("LANGUAGE", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0");

        for key in &self.removed {
            cmd.env_remove(key);
        }
        for (k, v) in &self.set {
            cmd.env(k, v);
        }

        #[cfg(windows)]
        {
            // Don't flash a console window when called from the GUI.
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let out = cmd.output().map_err(|source| Error::Spawn {
            git: self.path.clone(),
            source,
        })?;

        Ok(Output {
            code: out.status.code(),
            stdout: out.stdout,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn env_var(&self, name: &str) -> Option<String> {
        let key = OsStr::new(name);
        if self.removed.iter().any(|k| k == key) {
            return None;
        }
        if let Some((_, v)) = self.set.iter().rev().find(|(k, _)| k == key) {
            return Some(v.to_string_lossy().into_owned());
        }
        std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_overrides_win_and_removal_hides() {
        let g = SystemGit::new("git")
            .with_env("GITID_TEST_A", "1")
            .without_env("PATH");
        assert_eq!(g.env_var("GITID_TEST_A").as_deref(), Some("1"));
        assert_eq!(g.env_var("PATH"), None);

        let g = g.without_env("GITID_TEST_A").with_env("PATH", "x");
        assert_eq!(g.env_var("GITID_TEST_A"), None);
        assert_eq!(g.env_var("PATH").as_deref(), Some("x"));
    }
}

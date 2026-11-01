//! App-wide state shared by commands.

use std::path::PathBuf;
use std::sync::Mutex;

use gitid_core::{GitVersion, Result, SystemGit};

#[derive(Default)]
pub struct AppState {
    /// A git executable chosen in settings (step 4). `None` searches the
    /// usual places for this OS.
    git_override: Mutex<Option<PathBuf>>,
}

impl AppState {
    /// Find git and its version. Done per call: it's cheap, and it picks up
    /// a git installed or upgraded while the app is open.
    pub fn git(&self) -> Result<(SystemGit, GitVersion)> {
        let override_path = self
            .git_override
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let git = SystemGit::locate(override_path.as_deref())?;
        let version = GitVersion::detect(&git)?;
        Ok((git, version))
    }
}

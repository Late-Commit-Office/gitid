//! App-wide state shared by commands, and the app's own small settings file
//! (separate from git config: it only holds which git executable to use).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gitid_core::{GitVersion, Result, SystemGit};
use serde::{Deserialize, Serialize};

/// What's saved in `settings.json` in the app's config folder.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    /// A git executable chosen in Settings. `None` searches the usual places.
    git_path: Option<PathBuf>,
}

#[derive(Default)]
pub struct AppState {
    settings: Mutex<AppSettings>,
    /// Where `settings` is saved; set once at startup.
    file: Mutex<Option<PathBuf>>,
    /// Stop flag of the scan in progress, if any.
    scan: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    /// Read the settings file, if any. A missing or unreadable file just
    /// means defaults: it must never stop the app from starting.
    pub fn load(&self, file: PathBuf) {
        let loaded = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<AppSettings>(&text).ok())
            .unwrap_or_default();
        *lock(&self.settings) = loaded;
        *lock(&self.file) = Some(file);
    }

    /// The git executable chosen in Settings, if any.
    pub fn git_override(&self) -> Option<PathBuf> {
        lock(&self.settings).git_path.clone()
    }

    /// Use this git from now on (`None`: search again), and save the choice.
    pub fn set_git_override(&self, path: Option<PathBuf>) -> std::io::Result<()> {
        let snapshot = {
            let mut s = lock(&self.settings);
            s.git_path = path;
            s.clone()
        };
        match lock(&self.file).as_deref() {
            Some(file) => save(file, &snapshot),
            None => Ok(()),
        }
    }

    /// A fresh stop flag for a new scan. Starting a scan stops any earlier one.
    pub fn start_scan(&self) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Some(old) = lock(&self.scan).replace(flag.clone()) {
            old.store(true, Ordering::Relaxed);
        }
        flag
    }

    /// Ask the scan in progress (if any) to stop.
    pub fn cancel_scan(&self) {
        if let Some(flag) = lock(&self.scan).as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Find git and its version. Done per call: it's cheap, and it picks up
    /// a git installed or upgraded while the app is open.
    pub fn git(&self) -> Result<(SystemGit, GitVersion)> {
        let git = SystemGit::locate(self.git_override().as_deref())?;
        let version = GitVersion::detect(&git)?;
        Ok((git, version))
    }
}

fn save(file: &Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    // Write a sibling file, then rename over the old one, so a crash
    // mid-write can't leave a half-written settings file.
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, file)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

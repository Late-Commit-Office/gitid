//! Isolated git environment for integration tests.
//!
//! Every test gets its own temp folder, its own global config file, no system
//! config and a fake HOME, so the developer's real ~/.gitconfig is never read
//! or touched.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gitid_core::{Git, SystemGit};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct TestEnv {
    pub root: PathBuf,
    pub home: PathBuf,
    pub global: PathBuf,
    git: SystemGit,
}

/// Variables from the developer's (or CI's) shell that would leak into tests.
const SCRUB: &[&str] = &[
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_AUTHOR_DATE",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_COMMITTER_DATE",
    "EMAIL",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_SYSTEM",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_CEILING_DIRECTORIES",
    "XDG_CONFIG_HOME",
];

impl TestEnv {
    pub fn new(name: &str) -> TestEnv {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir()
            .join("gitid-tests")
            .join(format!("{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // Canonicalize so includeIf "gitdir:" matches (macOS /var -> /private/var).
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let global = home.join(".gitconfig");
        std::fs::write(&global, "").unwrap();

        let mut git = SystemGit::locate(None)
            .expect("git must be installed to run tests")
            .with_env("GIT_CONFIG_GLOBAL", &global)
            .with_env("GIT_CONFIG_NOSYSTEM", "1")
            .with_env("HOME", &home)
            .with_env("USERPROFILE", &home)
            // Stop discovery from walking above the temp root into some outer repo.
            .with_env("GIT_CEILING_DIRECTORIES", root.parent().unwrap());
        for var in SCRUB {
            if *var != "GIT_CEILING_DIRECTORIES" {
                git = git.without_env(var);
            }
        }

        TestEnv {
            root,
            home,
            global,
            git,
        }
    }

    pub fn git(&self) -> &SystemGit {
        &self.git
    }

    /// Same environment plus one extra variable.
    pub fn git_with(&self, key: &str, value: &str) -> SystemGit {
        self.git.clone().with_env(key, value)
    }

    /// Run git for test setup; panics with stderr on failure.
    pub fn sh(&self, dir: Option<&Path>, args: &[&str]) -> String {
        match self.git.run_ok(dir, args) {
            Ok(out) => out.stdout_str(),
            Err(e) => panic!("setup command failed: {e}"),
        }
    }

    pub fn global_set(&self, key: &str, value: &str) {
        self.sh(None, &["config", "--global", key, value]);
    }

    pub fn mkdir(&self, rel: &str) -> PathBuf {
        let p = self.root.join(rel);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    pub fn init_repo(&self, rel: &str) -> PathBuf {
        let p = self.mkdir(rel);
        self.sh(Some(&p), &["init", "-q"]);
        p
    }

    /// Write a standalone config file (for includeIf targets).
    pub fn write_file(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, contents).unwrap();
        p
    }

    /// Path in the form includeIf expects: forward slashes, trailing slash.
    pub fn gitdir_pattern(&self, dir: &Path) -> String {
        let mut s = dir.to_string_lossy().replace('\\', "/");
        if !s.ends_with('/') {
            s.push('/');
        }
        s
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

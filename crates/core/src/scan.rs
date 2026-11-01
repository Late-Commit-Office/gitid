//! Find repositories under a folder and sort them by identity: problems
//! (git would refuse, or guess), a different identity than your default, or
//! the same.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::config::{self, Target};
use crate::error::Result;
use crate::git::Git;
use crate::identity::{effective_identity, Part, RoleIdentity, Source};
use crate::paths;
use crate::rules::{self, Rule};

/// Folders never worth walking into: dependencies, build output, caches.
const SKIP: &[&str] = &[
    "node_modules",
    "target",
    "vendor",
    "venv",
    "__pycache__",
    "Pods",
    "DerivedData",
    "build",
    "dist",
    "Library",
    "AppData",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    /// How many folders deep to look below the root.
    pub max_depth: usize,
    /// Walk into hidden folders (names starting with `.`) too.
    pub include_hidden: bool,
    /// Repos checked at the same time.
    pub threads: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            max_depth: 6,
            include_hidden: false,
            threads: 4,
        }
    }
}

/// How a repo's identity compares with your default (global) one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Verdict {
    /// git would refuse to commit here.
    Missing { message: String },
    /// git would make up a name or email from the OS login.
    Guessed,
    /// A different identity than your default, and why.
    Different { because: Vec<Reason> },
    /// Same as your default.
    Same,
}

/// Why a repo's identity differs from your default.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Reason {
    /// An identity rule applies (by id).
    Rule { id: String },
    /// The repository's own config sets it.
    RepoConfig,
    /// An environment variable this process sees.
    Env { var: String },
    /// Some other config file (system, or an include that isn't a rule).
    OtherConfig { file: Option<PathBuf> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Finding {
    /// The repository's top folder.
    pub path: PathBuf,
    /// `Name <email>` as commits would show it, when git can produce one.
    pub identity: Option<String>,
    pub verdict: Verdict,
}

impl Finding {
    pub fn is_problem(&self) -> bool {
        matches!(self.verdict, Verdict::Missing { .. } | Verdict::Guessed)
    }
}

/// Progress for a UI. `found` grows while walking; `checked` while checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct Progress {
    pub folders_seen: usize,
    pub found: usize,
    pub checked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScanReport {
    pub root: PathBuf,
    /// Problems first, then different, then same; by path within each.
    pub findings: Vec<Finding>,
    /// Stopped early by the cancel flag.
    pub cancelled: bool,
    /// Folders that couldn't be read (permissions), skipped.
    pub unreadable: usize,
}

/// Find every repository below `root` (the root itself included).
/// Doesn't follow symlinks or descend into a repository once found.
pub fn find_repos(
    root: &Path,
    opts: &ScanOptions,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(Progress),
) -> (Vec<PathBuf>, usize, usize) {
    let mut repos = Vec::new();
    let mut unreadable = 0;
    let mut seen = 0;
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        seen += 1;
        if dir.join(".git").exists() {
            repos.push(dir);
            on_progress(Progress {
                folders_seen: seen,
                found: repos.len(),
                checked: 0,
            });
            continue;
        }
        if seen.is_multiple_of(200) {
            on_progress(Progress {
                folders_seen: seen,
                found: repos.len(),
                checked: 0,
            });
        }
        if depth >= opts.max_depth {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            unreadable += 1;
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) // not symlinks
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                !SKIP.contains(&name.as_ref()) && (opts.include_hidden || !name.starts_with('.'))
            })
            .map(|e| e.path())
            .collect();
        // Reverse-sorted onto the stack, so folders are visited in name order.
        children.sort();
        stack.extend(children.into_iter().rev().map(|c| (c, depth + 1)));
    }
    (repos, seen, unreadable)
}

/// Your default identity: what the global config sets for user.name/email.
fn global_identity(git: &dyn Git) -> Result<(Option<String>, Option<String>)> {
    let entries = config::read_target(git, &Target::Global)?;
    let get = |k: &str| {
        entries
            .iter()
            .rev()
            .find(|e| e.key_matches(k))
            .and_then(|e| e.value.clone())
    };
    Ok((get("user.name"), get("user.email")))
}

fn real(p: &Path) -> PathBuf {
    paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn reason_for(role: &RoleIdentity, part: Part, rules: &[Rule], repo: &Path) -> Option<Reason> {
    let field = role.field(part)?;
    Some(match &field.source {
        Source::Env { var } => Reason::Env { var: var.clone() },
        Source::Guessed => return None,
        Source::Config(e) => {
            let file = e.origin.file().map(real);
            if let Some(rule) = file
                .as_ref()
                .and_then(|f| rules.iter().find(|r| real(&r.file) == *f))
            {
                Reason::Rule {
                    id: rule.id.clone(),
                }
            } else if file.as_ref().is_some_and(|f| f.starts_with(real(repo))) {
                Reason::RepoConfig
            } else {
                Reason::OtherConfig { file }
            }
        }
    })
}

/// Check one repository against your default identity.
pub fn check_repo(
    git: &dyn Git,
    repo: &Path,
    default: &(Option<String>, Option<String>),
    rules: &[Rule],
) -> Result<Finding> {
    let id = effective_identity(git, repo)?;
    let a = &id.author;
    let path = id
        .repo
        .as_ref()
        .and_then(|r| r.work_tree.clone())
        .unwrap_or_else(|| repo.to_path_buf());

    let verdict = if let Some(message) = &a.error {
        Verdict::Missing {
            message: message.clone(),
        }
    } else if [Part::Name, Part::Email]
        .iter()
        .any(|p| a.field(*p).is_some_and(|f| f.source == Source::Guessed))
    {
        Verdict::Guessed
    } else {
        let value = |p| a.field(p).map(|f| f.value.clone());
        let mut because = Vec::new();
        for (part, default_value) in [(Part::Name, &default.0), (Part::Email, &default.1)] {
            if value(part) != *default_value {
                if let Some(r) = reason_for(a, part, rules, &path) {
                    if !because.contains(&r) {
                        because.push(r);
                    }
                }
            }
        }
        if because.is_empty() {
            Verdict::Same
        } else {
            Verdict::Different { because }
        }
    };
    Ok(Finding {
        identity: a.display(),
        path,
        verdict,
    })
}

/// Find repos under `root` and check each one. Stops early (and says so)
/// when `cancel` is set. Repos that fail to check are reported as missing
/// an identity, with git's error.
pub fn scan(
    git: &(dyn Git + Sync),
    root: &Path,
    opts: &ScanOptions,
    cancel: &AtomicBool,
    on_progress: &(dyn Fn(Progress) + Sync),
) -> Result<ScanReport> {
    let root = real(root);
    let (repos, seen, unreadable) = find_repos(&root, opts, cancel, on_progress);
    let default = global_identity(git)?;
    let all_rules = rules::list_rules(git).unwrap_or_default();

    let next = AtomicUsize::new(0);
    let checked = AtomicUsize::new(0);
    let results: Mutex<Vec<Finding>> = Mutex::new(Vec::with_capacity(repos.len()));
    std::thread::scope(|s| {
        for _ in 0..opts.threads.clamp(1, 16) {
            s.spawn(|| loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(repo) = repos.get(i) else { break };
                let finding =
                    check_repo(git, repo, &default, &all_rules).unwrap_or_else(|e| Finding {
                        path: repo.clone(),
                        identity: None,
                        verdict: Verdict::Missing {
                            message: e.to_string(),
                        },
                    });
                results
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(finding);
                let done = checked.fetch_add(1, Ordering::Relaxed) + 1;
                on_progress(Progress {
                    folders_seen: seen,
                    found: repos.len(),
                    checked: done,
                });
            });
        }
    });

    let mut findings = results.into_inner().unwrap_or_else(|p| p.into_inner());
    let rank = |f: &Finding| match f.verdict {
        Verdict::Missing { .. } | Verdict::Guessed => 0,
        Verdict::Different { .. } => 1,
        Verdict::Same => 2,
    };
    findings.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.path.cmp(&b.path)));
    Ok(ScanReport {
        root,
        findings,
        cancelled: cancel.load(Ordering::Relaxed),
        unreadable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_list_has_no_duplicates() {
        for (i, s) in SKIP.iter().enumerate() {
            assert!(!SKIP[i + 1..].contains(s), "{s}");
        }
    }
}

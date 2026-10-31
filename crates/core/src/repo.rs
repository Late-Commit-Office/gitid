use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::git::Git;

/// Where a folder's repository lives, if it is inside one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLocation {
    /// Top of the working tree. `None` for bare repos or when the folder is
    /// inside the `.git` directory itself.
    pub work_tree: Option<PathBuf>,
    /// Absolute path of the git directory (`.git`, or the worktree's own dir).
    pub git_dir: PathBuf,
}

impl RepoLocation {
    /// The folder git runs in after discovery; relative paths in
    /// `--show-origin` output (like `file:.git/config`) are relative to it.
    pub fn git_cwd(&self) -> &Path {
        self.work_tree.as_deref().unwrap_or(&self.git_dir)
    }
}

/// Find the repository containing `dir`. `Ok(None)` if it is not in one.
pub fn discover(git: &dyn Git, dir: &Path) -> Result<Option<RepoLocation>> {
    let git_dir = git.run(Some(dir), &["rev-parse", "--absolute-git-dir"])?;
    if !git_dir.success() {
        // "not a git repository" is a normal answer, not an error.
        return Ok(None);
    }
    let git_dir = PathBuf::from(git_dir.stdout_str().trim_end_matches(['\n', '\r']));

    let top = git.run(Some(dir), &["rev-parse", "--show-toplevel"])?;
    let work_tree = top
        .success()
        .then(|| top.stdout_str().trim_end_matches(['\n', '\r']).to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    Ok(Some(RepoLocation { work_tree, git_dir }))
}

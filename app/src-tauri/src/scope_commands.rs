//! The Scopes view: every key visible from a folder, each value it has in
//! system, global and local config (and included files), and which one wins.
//!
//! Secrets are masked here, before anything reaches the webview. Edits and
//! removals therefore point at a value by position (file, key, n-th), and are
//! checked against a fresh read of the config before anything is written.

use std::path::{Path, PathBuf};

use gitid_core::config::{self, ConfigSnapshot, EntryRef, Origin, Scope, Status, Target};
use gitid_core::secrets::mask_value;
use gitid_core::{paths, platform, repo, Error, Git, Invalid, RepoLocation};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// Identifies one value for editing: the n-th value of `key` in `file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueRef {
    file: String,
    key: String,
    nth: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeValue {
    scope: Scope,
    status: Status,
    /// Masked when it looks like a secret. `None`: a bare `key` line (true).
    value: Option<String>,
    masked: bool,
    origin: Origin,
    /// Read from a file included by the scope's main file (e.g. a rule's file).
    included: bool,
    /// Present when the value lives in a file and can be edited or removed.
    at: Option<ValueRef>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyView {
    key: String,
    multi_valued: bool,
    values: Vec<ScopeValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesView {
    /// `None`: outside any folder (system and global only).
    folder: Option<PathBuf>,
    repo: Option<RepoLocation>,
    keys: Vec<KeyView>,
}

fn real(p: &Path) -> PathBuf {
    paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn folder_of(path: Option<String>) -> Result<Option<PathBuf>, Error> {
    match path.filter(|p| !p.trim().is_empty()) {
        None => Ok(None),
        Some(p) => {
            let given = PathBuf::from(p);
            paths::canonicalize(&given)
                .map(Some)
                .map_err(|_| Error::NotADirectory(given))
        }
    }
}

fn snapshot(
    git: &dyn Git,
    folder: Option<&Path>,
) -> Result<(ConfigSnapshot, Option<RepoLocation>), Error> {
    let repo = match folder {
        Some(f) => repo::discover(git, f)?,
        None => None,
    };
    Ok((config::read_all_in(git, folder, repo.as_ref())?, repo))
}

fn build(git: &dyn Git, folder: Option<PathBuf>) -> Result<ScopesView, Error> {
    let (snap, repo) = snapshot(git, folder.as_deref())?;
    let global_file = platform::global_config_file(git).map(|p| real(&p));
    let local_file = repo.as_ref().map(|r| real(&r.git_dir.join("config")));

    let keys = config::layers(&snap)
        .into_iter()
        .map(|k| KeyView {
            values: k
                .layers
                .into_iter()
                .map(|l| {
                    let raw = l.entry.value.clone();
                    let shown = raw.as_deref().map(|v| mask_value(&k.key, v));
                    let masked = shown != raw;
                    let file = l.entry.origin.file().map(real);
                    let main = match l.entry.scope {
                        Scope::Global => global_file.as_ref(),
                        Scope::Local => local_file.as_ref(),
                        _ => None,
                    };
                    ScopeValue {
                        included: matches!((&file, main), (Some(f), Some(m)) if f != m),
                        at: file.map(|f| ValueRef {
                            file: f.to_string_lossy().into_owned(),
                            key: l.at.key.clone(),
                            nth: l.at.nth,
                        }),
                        scope: l.entry.scope,
                        status: l.status,
                        value: shown,
                        masked,
                        origin: l.entry.origin,
                    }
                })
                .collect(),
            key: k.key,
            multi_valued: k.multi_valued,
        })
        .collect();

    Ok(ScopesView { folder, repo, keys })
}

/// Find the value a [`ValueRef`] points at, in a fresh read of the config
/// visible from `folder`. Refuses anything that isn't there, so the webview
/// can't make gitid write to an arbitrary file.
fn locate(
    git: &dyn Git,
    folder: Option<&Path>,
    at: &ValueRef,
) -> Result<(PathBuf, Option<String>), Error> {
    let (snap, _) = snapshot(git, folder)?;
    let wanted = real(Path::new(&at.file));
    let origin = snap
        .entries
        .iter()
        .find(|e| e.key == at.key && e.origin.file().map(real).as_ref() == Some(&wanted))
        .map(|e| e.origin.clone());
    let entry = origin.and_then(|origin| {
        config::resolve(
            &snap,
            &EntryRef {
                origin,
                key: at.key.clone(),
                nth: at.nth,
            },
        )
    });
    match entry {
        Some(e) => Ok((wanted, e.value.clone())),
        None => Err(Invalid::StaleValue.into()),
    }
}

#[tauri::command]
pub async fn get_scopes(state: State<'_, AppState>, path: Option<String>) -> CmdResult<ScopesView> {
    let (git, _) = state.git()?;
    blocking(move || build(&git, folder_of(path)?).map_err(Into::into)).await
}

/// Change one value in place. Returns the refreshed view.
#[tauri::command]
pub async fn edit_scope_value(
    state: State<'_, AppState>,
    path: Option<String>,
    at: ValueRef,
    value: String,
) -> CmdResult<ScopesView> {
    let (git, _) = state.git()?;
    blocking(move || {
        let folder = folder_of(path)?;
        let (file, old) = locate(&git, folder.as_deref(), &at)?;
        let target = Target::File(file);
        match old {
            Some(old) => {
                config::replace_value(&git, &target, &at.key, &old, &value)?;
            }
            // A bare `key` line can't be matched by value; replace it only
            // when it is the key's single value in that file.
            None => config::set(&git, &target, &at.key, &value)?,
        }
        build(&git, folder).map_err(Into::into)
    })
    .await
}

/// Remove one value. Returns the refreshed view.
#[tauri::command]
pub async fn remove_scope_value(
    state: State<'_, AppState>,
    path: Option<String>,
    at: ValueRef,
) -> CmdResult<ScopesView> {
    let (git, _) = state.git()?;
    blocking(move || {
        let folder = folder_of(path)?;
        let (file, old) = locate(&git, folder.as_deref(), &at)?;
        let target = Target::File(file);
        match old {
            Some(old) => config::unset_value(&git, &target, &at.key, &old)?,
            None => config::unset(&git, &target, &at.key)?,
        };
        build(&git, folder).map_err(Into::into)
    })
    .await
}

/// Where a new value goes.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddTo {
    System,
    Global,
    /// The repository containing the view's folder.
    Local,
}

/// Add a value (or set it, for a single-valued key). Returns the refreshed view.
#[tauri::command]
pub async fn add_scope_value(
    state: State<'_, AppState>,
    path: Option<String>,
    scope: AddTo,
    key: String,
    value: String,
) -> CmdResult<ScopesView> {
    let (git, _) = state.git()?;
    blocking(move || {
        let folder = folder_of(path)?;
        let target = match scope {
            AddTo::System => Target::System,
            AddTo::Global => Target::Global,
            AddTo::Local => Target::Local(folder.clone().ok_or(Invalid::NeedsRepoFolder)?),
        };
        let key = key.trim();
        if config::is_multi_valued(key) {
            config::add(&git, &target, key, &value)?;
        } else {
            config::set(&git, &target, key, &value)?;
        }
        build(&git, folder).map_err(Into::into)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn value_ref_and_add_to_json() {
        let r: ValueRef = serde_json::from_value(
            json!({ "file": "/h/.gitconfig", "key": "user.name", "nth": 0 }),
        )
        .unwrap();
        assert_eq!(r.nth, 0);
        assert_eq!(serde_json::to_value(&r).unwrap()["key"], "user.name");
        for (s, want) in [
            ("global", "Global"),
            ("local", "Local"),
            ("system", "System"),
        ] {
            let got: AddTo = serde_json::from_value(json!(s)).unwrap();
            assert_eq!(format!("{got:?}"), want);
        }
    }
}

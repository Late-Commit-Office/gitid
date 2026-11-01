//! The only things the frontend can ask for. Each is a thin wrapper over
//! `gitid-core`; git work runs on a blocking thread so the UI stays responsive.

use std::path::{Path, PathBuf};

use gitid_core::rules::{self, Rule};
use gitid_core::settings::{self, SettingValue};
use gitid_core::{
    effective_identity, paths, platform, Error, GitVersion, Identity, Part, RoleIdentity, Source,
    SystemGit, WarningReport,
};
use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;

/// Errors as the frontend sees them: a stable `kind` to branch on, and a
/// sentence to show.
#[derive(Debug, Serialize)]
pub struct CommandError {
    kind: &'static str,
    message: String,
}

impl From<Error> for CommandError {
    fn from(e: Error) -> Self {
        let kind = match &e {
            Error::GitNotFound { .. } => "gitNotFound",
            Error::Spawn { .. } => "gitNotRunnable",
            Error::NotADirectory(_) => "notADirectory",
            Error::NotARepository(_) => "notARepository",
            Error::Unsupported { .. } => "unsupported",
            Error::InvalidInput(_) => "invalidInput",
            Error::MultipleValues { .. } => "multipleValues",
            Error::GitFailed { .. } => "gitFailed",
            Error::Parse(_) | Error::Io { .. } => "internal",
        };
        CommandError {
            kind,
            message: e.to_string(),
        }
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        CommandError {
            kind: "internal",
            message: e.to_string(),
        }
    }
}

type CmdResult<T> = Result<T, CommandError>;

/// Run blocking git work off the async runtime's worker threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError {
            kind: "internal",
            message: e.to_string(),
        })?
}

// ---------------------------------------------------------------- git_info

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    path: PathBuf,
    version: String,
    supports_hasconfig: bool,
    home: Option<PathBuf>,
}

/// Which git the app uses, and the home folder (for shortening paths to `~`).
#[tauri::command]
pub async fn git_info(state: State<'_, AppState>) -> CmdResult<GitInfo> {
    let (git, version) = state.git()?;
    blocking(move || {
        Ok(GitInfo {
            path: git.path().to_path_buf(),
            version: version.to_string(),
            supports_hasconfig: version.supports_hasconfig(),
            home: platform::home_dir(&git),
        })
    })
    .await
}

// ---------------------------------------------------------------- get_identity

/// Which rule (if any) supplied each value, by rule id.
#[derive(Serialize, Default)]
pub struct RoleRules {
    name: Option<String>,
    email: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityView {
    #[serde(flatten)]
    identity: Identity,
    warnings: Vec<WarningReport>,
    has_problems: bool,
    author_rules: RoleRules,
    committer_rules: RoleRules,
}

/// Everything the identity view shows for one folder.
#[tauri::command]
pub async fn get_identity(state: State<'_, AppState>, path: String) -> CmdResult<IdentityView> {
    let (git, _) = state.git()?;
    blocking(move || {
        let given = PathBuf::from(&path);
        let folder = paths::canonicalize(&given).map_err(|_| Error::NotADirectory(given))?;
        let identity = effective_identity(&git, &folder)?;
        // Rules only label values; failing to list them must not hide the identity.
        let all_rules = rules::list_rules(&git).unwrap_or_default();

        let (warnings, has_problems) = {
            let report = identity.report();
            (report.warnings, report.has_problems)
        };
        Ok(IdentityView {
            author_rules: role_rules(&identity.author, &all_rules),
            committer_rules: role_rules(&identity.committer, &all_rules),
            identity,
            warnings,
            has_problems,
        })
    })
    .await
}

fn role_rules(role: &RoleIdentity, all: &[Rule]) -> RoleRules {
    let rule_id = |part| {
        let Source::Config(entry) = &role.field(part)?.source else {
            return None;
        };
        let file = real(entry.origin.file()?);
        all.iter()
            .find(|r| real(&r.file) == file)
            .map(|r| r.id.clone())
    };
    RoleRules {
        name: rule_id(Part::Name),
        email: rule_id(Part::Email),
    }
}

fn real(p: &Path) -> PathBuf {
    paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

// ---------------------------------------------------------------- pick_folder

/// Show the native folder picker. `None` if the user cancels.
///
/// Done from Rust rather than the dialog plugin's JS API, so the webview
/// itself needs no dialog permissions.
#[tauri::command]
pub async fn pick_folder(app: AppHandle, window: WebviewWindow) -> CmdResult<Option<String>> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose a folder")
        .set_parent(&window)
        .blocking_pick_folder();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

// ---------------------------------------------------------------- settings

/// Which git the app uses, and whether it works.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLocation {
    /// The executable chosen in Settings; `None` means "search for it".
    chosen: Option<PathBuf>,
    /// The git in use, when one was found and runs.
    path: Option<PathBuf>,
    version: Option<String>,
    /// Why no usable git was found.
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    git: GitLocation,
    /// The file global settings are saved to (`None` without a working git).
    global_file: Option<PathBuf>,
    /// Empty without a working git: values can only be read through git.
    settings: Vec<SettingValue>,
}

/// Build the settings view. Never fails just because git is missing:
/// that's exactly when the user needs this screen to point the app at git.
fn view_for(chosen: Option<PathBuf>) -> CmdResult<SettingsView> {
    let found =
        SystemGit::locate(chosen.as_deref()).and_then(|g| GitVersion::detect(&g).map(|v| (g, v)));
    Ok(match found {
        Ok((git, version)) => SettingsView {
            global_file: platform::global_config_file(&git),
            settings: settings::read(&git)?,
            git: GitLocation {
                chosen,
                path: Some(git.path().to_path_buf()),
                version: Some(version.to_string()),
                error: None,
            },
        },
        Err(e) => SettingsView {
            git: GitLocation {
                chosen,
                path: None,
                version: None,
                error: Some(e.to_string()),
            },
            global_file: None,
            settings: Vec::new(),
        },
    })
}

/// The managed global settings and the git location.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CmdResult<SettingsView> {
    let chosen = state.git_override();
    blocking(move || view_for(chosen)).await
}

/// Change one managed setting (`None` or git's default removes it).
/// Returns the updated view.
#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: Option<String>,
) -> CmdResult<SettingsView> {
    let (git, _) = state.git()?;
    let chosen = state.git_override();
    blocking(move || {
        settings::apply(&git, &key, value.as_deref())?;
        view_for(chosen)
    })
    .await
}

/// Use this git executable (`None` or empty: search for git again).
/// A path that isn't a working git is refused and not saved.
#[tauri::command]
pub async fn set_git_path(
    state: State<'_, AppState>,
    path: Option<String>,
) -> CmdResult<SettingsView> {
    let chosen = path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    if let Some(p) = chosen.clone() {
        blocking(move || {
            let git = SystemGit::locate(Some(&p))?;
            GitVersion::detect(&git).map_err(|_| {
                Error::InvalidInput(format!("{} doesn't look like git", p.display()))
            })?;
            Ok(())
        })
        .await?;
    }
    state
        .set_git_override(chosen.clone())
        .map_err(|e| CommandError {
            kind: "internal",
            message: format!("couldn't save the app's settings: {e}"),
        })?;
    blocking(move || view_for(chosen)).await
}

/// Native file picker for the git program. `None` if cancelled.
#[tauri::command]
pub async fn pick_git_executable(
    app: AppHandle,
    window: WebviewWindow,
) -> CmdResult<Option<String>> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose the git program")
        .set_parent(&window)
        .blocking_pick_file();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

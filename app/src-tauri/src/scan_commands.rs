//! The repo scanner: find repos under a folder and sort them by identity.
//! Progress streams to the UI over a channel; `cancel_scan` stops it.

use std::path::PathBuf;

use gitid_core::scan::{self, Progress, ScanOptions, ScanReport};
use gitid_core::{paths, Error};
use tauri::ipc::Channel;
use tauri::State;

use crate::commands::{blocking, CmdResult};
use crate::state::AppState;

/// Scan `path`. Sends progress on `on_progress` and returns the full report
/// (marked `cancelled` if stopped early).
#[tauri::command]
pub async fn scan_folder(
    state: State<'_, AppState>,
    path: String,
    depth: Option<usize>,
    include_hidden: bool,
    on_progress: Channel<Progress>,
) -> CmdResult<ScanReport> {
    let (git, _) = state.git()?;
    let cancel = state.start_scan();
    blocking(move || {
        let given = PathBuf::from(&path);
        let root = paths::canonicalize(&given).map_err(|_| Error::NotADirectory(given))?;
        let opts = ScanOptions {
            max_depth: depth.unwrap_or(ScanOptions::default().max_depth),
            include_hidden,
            ..ScanOptions::default()
        };
        let send = |p: Progress| {
            // A closed window just means nobody is listening any more.
            let _ = on_progress.send(p);
        };
        scan::scan(&git, &root, &opts, &cancel, &send).map_err(Into::into)
    })
    .await
}

/// Stop the scan in progress. Its `scan_folder` call returns what it found so far.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    state.cancel_scan();
}

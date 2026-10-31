use std::path::Path;

use gitid_core::config::{normalize_key, read_all};
use gitid_core::secrets::mask_value;
use gitid_core::{Entry, Result, SystemGit};

use crate::style;
use crate::EXIT_OK;

pub fn run(git: &SystemGit, path: &Path, key: Option<&str>) -> Result<u8> {
    let folder = gitid_core::paths::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !folder.is_dir() {
        return Err(gitid_core::Error::NotADirectory(folder));
    }
    let snap = read_all(git, Some(&folder))?;

    match key {
        Some(key) => show_key(&snap.get_all(key), key),
        None => show_all(&snap.entries),
    }
    Ok(EXIT_OK)
}

fn value(e: &Entry) -> String {
    match &e.value {
        None => "(no value: true)".into(),
        Some(v) => mask_value(&e.key, v).replace('\n', "\\n"),
    }
}

/// Entries grouped by the file they came from, in git's reading order.
fn show_all(entries: &[Entry]) {
    if entries.is_empty() {
        println!("No git config is set for this folder.");
        return;
    }
    let mut last: Option<(&gitid_core::Scope, &gitid_core::Origin)> = None;
    for e in entries {
        if last != Some((&e.scope, &e.origin)) {
            if last.is_some() {
                println!();
            }
            println!("{:<9} {}", e.scope.label(), style::origin(&e.origin));
            last = Some((&e.scope, &e.origin));
        }
        println!("  {} = {}", e.key, value(e));
    }
}

/// Every value of one key; the last one is what git uses for single-valued keys.
fn show_key(entries: &[&Entry], key: &str) {
    let key = normalize_key(key);
    if entries.is_empty() {
        println!("{key} is not set here.");
        return;
    }
    for (i, e) in entries.iter().enumerate() {
        let mark = if i + 1 == entries.len() {
            "  <- used"
        } else {
            ""
        };
        println!(
            "{:<9} {:<40} {}{mark}",
            e.scope.label(),
            style::origin(&e.origin),
            value(e)
        );
    }
    if entries.len() > 1 {
        println!("\n(For multi-valued keys like remote.*.fetch, git uses all of them.)");
    }
}

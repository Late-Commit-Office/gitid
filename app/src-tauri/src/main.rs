// Prevents an extra console window on Windows in release builds. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The gitid desktop app: a thin Tauri shell over `gitid-core`.
//!
//! The webview never touches the shell or config files. It can only call the
//! commands registered below, and each of those goes through the core, which
//! goes through git.

mod commands;
mod rule_commands;
mod scope_commands;
mod state;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        // Registered so Rust can show the native folder picker. The webview is
        // granted no dialog permissions (see capabilities/default.json).
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .setup(|app| {
            // The app's own settings (which git to use) live in its config folder.
            let dir = app.path().app_config_dir()?;
            app.state::<state::AppState>()
                .load(dir.join("settings.json"));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::git_info,
            commands::get_identity,
            commands::pick_folder,
            commands::get_settings,
            commands::set_setting,
            commands::set_git_path,
            commands::pick_git_executable,
            rule_commands::list_rules,
            rule_commands::save_rule,
            rule_commands::remove_rule,
            rule_commands::check_rule,
            scope_commands::get_scopes,
            scope_commands::edit_scope_value,
            scope_commands::remove_scope_value,
            scope_commands::add_scope_value,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gitid");
}

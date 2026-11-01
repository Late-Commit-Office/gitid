//! `gitid settings`.

use gitid_core::settings::{self, Group, Kind, SettingValue};
use gitid_core::{platform, Result, SystemGit};

use crate::style;
use crate::EXIT_OK;

/// How a value reads in the list: "on"/"off", a choice's label, or the text.
fn shown(s: &SettingValue) -> String {
    match (s.setting.kind, s.value.as_deref()) {
        (Kind::Bool { default }, None) => format!("{} (git's default)", on_off(default)),
        (Kind::Bool { .. }, Some(v)) => {
            on_off(gitid_core::config::parse_bool(v).unwrap_or(false)).into()
        }
        (Kind::Choice { choices, default }, v) => {
            let value = v.unwrap_or(default);
            let label = choices
                .iter()
                .find(|c| c.value == value)
                .map_or(value, |c| c.label);
            if v.is_none() {
                format!("{label} (git's default)")
            } else {
                label.to_string()
            }
        }
        (Kind::Text { .. }, Some(v)) => v.to_string(),
        (Kind::Text { .. }, None) => "(not set)".into(),
    }
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

fn group_title(g: Group) -> &'static str {
    match g {
        Group::Identity => "Identity",
        Group::Defaults => "Defaults",
        Group::Signing => "Commit signing",
    }
}

pub fn list(git: &SystemGit, json: bool) -> Result<u8> {
    let all = settings::read(git)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&all).expect("settings serialize")
        );
        return Ok(EXIT_OK);
    }
    if let Some(file) = platform::global_config_file(git) {
        println!("Your global config: {}", style::tilde(&file));
    }
    let mut group = None;
    for s in &all {
        if group != Some(s.setting.group) {
            group = Some(s.setting.group);
            println!("\n{}", group_title(s.setting.group));
        }
        println!(
            "  {:<26} {:<34} {}",
            s.setting.label,
            shown(s),
            s.setting.key
        );
    }
    println!("\nChange one with: gitid set KEY VALUE");
    Ok(EXIT_OK)
}

//! `gitid set` and `gitid unset`.

use gitid_core::config::{self, Target};
use gitid_core::secrets::mask_value;
use gitid_core::settings;
use gitid_core::{paths, Result, SystemGit};

use crate::args::Scope;
use crate::style;
use crate::EXIT_OK;

pub fn target(scope: &Scope) -> Target {
    let real = |p: &std::path::Path| paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    match scope {
        Scope::Global => Target::Global,
        Scope::System => Target::System,
        Scope::Repo(dir) => Target::Local(real(dir)),
        // The file may not exist yet, so don't canonicalize it.
        Scope::File(f) => Target::File(f.clone()),
    }
}

pub fn where_(target: &Target) -> String {
    match target {
        Target::System => "the system config".into(),
        Target::Global => "your global config".into(),
        Target::Local(dir) => format!("the repo config of {}", style::tilde(dir)),
        Target::File(f) => style::tilde(f),
    }
}

pub fn set(git: &SystemGit, key: &str, value: &str, scope: &Scope) -> Result<u8> {
    let t = target(scope);
    // Settings gitid manages get the same checks as in the app, and git's
    // default value removes the line instead of writing it.
    if let (Target::Global, Some(setting)) = (&t, settings::find(key)) {
        settings::apply(git, setting.key, Some(value))?;
        let now = settings::read(git)?
            .into_iter()
            .find(|s| s.setting.key == setting.key)
            .and_then(|s| s.value);
        match now {
            Some(v) => println!(
                "Set {} = {} in {}",
                setting.key,
                mask_value(key, &v),
                where_(&t)
            ),
            None => println!(
                "Set {} back to git's default (removed it from {})",
                setting.key,
                where_(&t)
            ),
        }
        return Ok(EXIT_OK);
    }
    config::set(git, &t, key, value)?;
    println!("Set {key} = {} in {}", mask_value(key, value), where_(&t));
    Ok(EXIT_OK)
}

pub fn unset(git: &SystemGit, key: &str, scope: &Scope, all: bool) -> Result<u8> {
    let t = target(scope);
    let removed = if all {
        config::unset_all(git, &t, key)?
    } else {
        config::unset(git, &t, key)?
    };
    if removed {
        println!("Removed {key} from {}", where_(&t));
    } else {
        println!("{key} was not set in {}", where_(&t));
    }
    Ok(EXIT_OK)
}

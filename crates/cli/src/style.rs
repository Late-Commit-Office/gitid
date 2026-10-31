//! Small display helpers shared by the commands.

use std::path::{Path, PathBuf};

use gitid_core::{Origin, Source};

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// `/home/ann/work` -> `~/work`.
pub fn tilde(p: &Path) -> String {
    if let Some(home) = home() {
        if let Ok(rest) = p.strip_prefix(&home) {
            return if rest.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
            };
        }
    }
    p.display().to_string()
}

pub fn origin(o: &Origin) -> String {
    match o {
        Origin::File(p) => tilde(p),
        other => other.to_string(),
    }
}

/// One line explaining a [`Source`], e.g. `global    ~/.gitconfig`.
pub fn source(s: &Source, expected_key: &str) -> String {
    match s {
        Source::Env { var } => format!("{:<9} ${var}", "env"),
        Source::Guessed => format!(
            "{:<9} made up from your OS login (not configured)",
            "guessed"
        ),
        Source::Config(e) => {
            let mut line = format!("{:<9} {}", e.scope.label(), origin(&e.origin));
            if e.key != expected_key {
                line.push_str(&format!("  ({})", e.key));
            }
            line
        }
    }
}

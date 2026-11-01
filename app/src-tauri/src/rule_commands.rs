//! Identity rules for the Rules view: list, save (add or edit), remove, check.

use std::path::{Path, PathBuf};

use gitid_core::rules::{self, Condition, Described, NewRule, Rule, When};
use gitid_core::{paths, platform, Error, Invalid};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// One "applies to" line of the rule form. The same shape goes to the
/// frontend and comes back, so editing round-trips exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WhenDto {
    Folder {
        path: String,
    },
    RemoteHost {
        host: String,
        ssh_port: Option<u16>,
    },
    RemotePattern {
        pattern: String,
    },
    /// A condition the form can't edit (e.g. `onbranch:`); kept as written.
    Raw {
        raw: String,
    },
}

impl From<When> for WhenDto {
    fn from(w: When) -> Self {
        match w {
            When::Folder(p) => WhenDto::Folder {
                path: p.to_string_lossy().into_owned(),
            },
            When::RemoteHost { host, ssh_port } => WhenDto::RemoteHost { host, ssh_port },
            When::RemotePattern(pattern) => WhenDto::RemotePattern { pattern },
            When::Raw(c) => WhenDto::Raw { raw: c.raw() },
        }
    }
}

impl From<WhenDto> for When {
    fn from(w: WhenDto) -> Self {
        match w {
            WhenDto::Folder { path } => When::Folder(PathBuf::from(path.trim())),
            WhenDto::RemoteHost { host, ssh_port } => When::RemoteHost { host, ssh_port },
            WhenDto::RemotePattern { pattern } => When::RemotePattern(pattern),
            WhenDto::Raw { raw } => When::Raw(Condition::parse(&raw)),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    #[serde(flatten)]
    rule: Rule,
    /// One line per folder or server, as data the frontend phrases.
    lines: Vec<Described>,
    /// The form inputs that recreate this rule.
    when: Vec<WhenDto>,
}

fn view(rule: Rule, home: Option<&Path>) -> RuleView {
    RuleView {
        lines: rules::described(&rule.conditions),
        when: rules::when_from(&rule, home)
            .into_iter()
            .map(Into::into)
            .collect(),
        rule,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesView {
    rules: Vec<RuleView>,
    /// Server rules need git 2.36+.
    supports_hasconfig: bool,
}

#[derive(Serialize)]
pub struct Check {
    path: PathBuf,
    applies: bool,
}

#[derive(Serialize)]
pub struct SaveResult {
    rule: RuleView,
    /// Repos the rule was checked against, and whether it applies there.
    checks: Vec<Check>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    id: String,
    name: Option<String>,
    email: Option<String>,
    when: Vec<WhenDto>,
    /// Editing: make the rule exactly this. Adding: false.
    replace: bool,
}

fn blank_to_none(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn expand_tilde(path: &str, home: Option<&Path>) -> String {
    match (path, home) {
        ("~", Some(h)) => h.to_string_lossy().into_owned(),
        (p, Some(h)) if p.starts_with("~/") || p.starts_with("~\\") => {
            h.join(&p[2..]).to_string_lossy().into_owned()
        }
        (p, _) => p.to_string(),
    }
}

fn real(p: &Path) -> PathBuf {
    paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[tauri::command]
pub async fn list_rules(state: State<'_, AppState>) -> CmdResult<RulesView> {
    let (git, version) = state.git()?;
    blocking(move || {
        let home = platform::home_dir(&git);
        Ok(RulesView {
            rules: rules::list_rules(&git)?
                .into_iter()
                .map(|r| view(r, home.as_deref()))
                .collect(),
            supports_hasconfig: version.supports_hasconfig(),
        })
    })
    .await
}

/// Add or edit a rule, then check it against repos found in its folders.
#[tauri::command]
pub async fn save_rule(state: State<'_, AppState>, input: RuleInput) -> CmdResult<SaveResult> {
    let (git, version) = state.git()?;
    blocking(move || {
        // Folders may be typed as ~/work; expand ~ the way git would.
        let home = platform::home_dir(&git);
        let when: Vec<When> = input
            .when
            .into_iter()
            .map(|w| match w {
                WhenDto::Folder { path } => WhenDto::Folder {
                    path: expand_tilde(path.trim(), home.as_deref()),
                },
                other => other,
            })
            .map(Into::into)
            .collect();
        let new = NewRule {
            id: input.id.trim().to_string(),
            name: blank_to_none(input.name),
            email: blank_to_none(input.email),
            when,
        };
        let rule = if input.replace {
            rules::replace_rule(&git, version, &new)?
        } else {
            rules::add_rule(&git, version, &new)?
        };

        let mut checks = Vec::new();
        for w in &new.when {
            if let When::Folder(dir) = w {
                for repo in rules::sample_repos(&real(dir), 10) {
                    let applies = rules::applies_in(&git, &rule, &repo)?;
                    checks.push(Check {
                        path: repo,
                        applies,
                    });
                }
            }
        }
        Ok(SaveResult {
            rule: view(rule, home.as_deref()),
            checks,
        })
    })
    .await
}

fn no_such_rule(all: &[Rule], id: &str) -> Error {
    Error::Invalid(Invalid::NoSuchRule {
        id: id.to_string(),
        known: all.iter().map(|r| r.id.clone()).collect(),
    })
}

/// Remove a rule; with `delete_file`, also the identity file gitid created.
#[tauri::command]
pub async fn remove_rule(
    state: State<'_, AppState>,
    id: String,
    delete_file: bool,
) -> CmdResult<RulesView> {
    {
        let (git, _) = state.git()?;
        blocking(move || {
            let all = rules::list_rules(&git)?;
            let rule = rules::find(&all, &id).ok_or_else(|| no_such_rule(&all, &id))?;
            rules::remove_rule(&git, rule, delete_file)?;
            Ok(())
        })
        .await?;
    }
    list_rules(state).await
}

/// Does rule `id` apply in `path`?
#[tauri::command]
pub async fn check_rule(state: State<'_, AppState>, id: String, path: String) -> CmdResult<Check> {
    let (git, _) = state.git()?;
    blocking(move || {
        let given = PathBuf::from(&path);
        let folder = paths::canonicalize(&given).map_err(|_| Error::NotADirectory(given))?;
        let all = rules::list_rules(&git)?;
        let rule = rules::find(&all, &id).ok_or_else(|| no_such_rule(&all, &id))?;
        Ok(Check {
            applies: rules::applies_in(&git, rule, &folder)?,
            path: folder,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Pins the JSON the frontend's RuleForm sends and receives (app/src/lib/types.ts).
    #[test]
    fn when_dto_json_shape() {
        let cases = [
            (
                WhenDto::Folder {
                    path: "/home/ann/work".into(),
                },
                json!({ "kind": "folder", "path": "/home/ann/work" }),
            ),
            (
                WhenDto::RemoteHost {
                    host: "g.example".into(),
                    ssh_port: Some(2222),
                },
                json!({ "kind": "remoteHost", "host": "g.example", "sshPort": 2222 }),
            ),
            (
                WhenDto::RemotePattern {
                    pattern: "https://x/**".into(),
                },
                json!({ "kind": "remotePattern", "pattern": "https://x/**" }),
            ),
            (
                WhenDto::Raw {
                    raw: "onbranch:main".into(),
                },
                json!({ "kind": "raw", "raw": "onbranch:main" }),
            ),
        ];
        for (dto, expected) in cases {
            assert_eq!(serde_json::to_value(&dto).unwrap(), expected);
            let back: WhenDto = serde_json::from_value(expected).unwrap();
            assert_eq!(back, dto);
        }
        // A missing port comes back as null and is accepted when absent.
        let host: WhenDto =
            serde_json::from_value(json!({ "kind": "remoteHost", "host": "h" })).unwrap();
        assert_eq!(
            host,
            WhenDto::RemoteHost {
                host: "h".into(),
                ssh_port: None
            }
        );
    }

    #[test]
    fn when_round_trips_through_dto() {
        let whens = [
            When::Folder(PathBuf::from("/srv/code")),
            When::RemoteHost {
                host: "g.example:3000".into(),
                ssh_port: None,
            },
            When::RemotePattern("https://x/**".into()),
            When::Raw(Condition::parse("onbranch:release")),
            When::Raw(Condition::parse("gitdir/i:~/a/**/b/")),
        ];
        for w in whens {
            assert_eq!(When::from(WhenDto::from(w.clone())), w);
        }
    }

    #[test]
    fn tilde_expands_only_at_the_start() {
        let home = Path::new("/home/ann");
        // Compare as paths: on Windows the joined separator is `\`.
        assert_eq!(
            PathBuf::from(expand_tilde("~/work", Some(home))),
            home.join("work")
        );
        assert_eq!(
            PathBuf::from(expand_tilde("~/a/b", Some(home))),
            home.join("a/b")
        );
        assert_eq!(expand_tilde("~", Some(home)), "/home/ann");
        assert_eq!(expand_tilde("/srv/~x", Some(home)), "/srv/~x");
        assert_eq!(expand_tilde("~/work", None), "~/work");
    }

    #[test]
    fn rule_input_parses() {
        let input: RuleInput = serde_json::from_value(json!({
            "id": "work",
            "name": null,
            "email": " ann@corp.example ",
            "when": [{ "kind": "folder", "path": "~/work" }],
            "replace": true
        }))
        .unwrap();
        assert_eq!(input.id, "work");
        assert!(input.replace);
        assert_eq!(
            blank_to_none(input.email).as_deref(),
            Some("ann@corp.example")
        );
        assert_eq!(blank_to_none(Some("  ".into())), None);
    }
}

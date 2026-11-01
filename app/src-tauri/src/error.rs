//! Errors as the frontend receives them.
//!
//! The frontend phrases every message itself, in the user's language, from
//! `code` and `params` (its catalog entry is `error.<code>`). `message` is the
//! same thing in English, for logs and as a fallback when the frontend has no
//! entry. `detail` is text from git or the operating system, which the
//! frontend shows as it is.

use gitid_core::error::last_meaningful_line;
use gitid_core::Error;
use serde::Serialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// A broad category: `invalid`, `gitNotFound`, `gitFailed`, `internal`, ...
    kind: &'static str,
    /// What exactly went wrong.
    code: String,
    /// The values the message needs, by name.
    params: Map<String, Value>,
    /// English, for logs and as a fallback.
    message: String,
    /// git's or the operating system's own words, if they help.
    detail: Option<String>,
}

pub(crate) type CmdResult<T> = Result<T, CommandError>;

impl CommandError {
    fn new(kind: &'static str, code: &str, params: Value, message: String) -> Self {
        CommandError {
            kind,
            code: code.to_string(),
            params: match params {
                Value::Object(map) => map,
                _ => Map::new(),
            },
            message,
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        self.detail = (!detail.is_empty()).then_some(detail);
        self
    }

    /// Something the user picked or typed that the app itself refuses.
    pub fn invalid(code: &str, params: Value, message: String) -> Self {
        CommandError::new("invalid", code, params, message)
    }

    /// The app's own settings file couldn't be written.
    pub fn save_settings(e: std::io::Error) -> Self {
        CommandError::new(
            "internal",
            "saveSettings",
            json!({}),
            format!("couldn't save the app's settings: {e}"),
        )
        .with_detail(e.to_string())
    }

    /// Something that shouldn't happen; only the detail can say more.
    pub fn internal(detail: impl ToString) -> Self {
        let detail = detail.to_string();
        CommandError::new("internal", "internal", json!({}), detail.clone()).with_detail(detail)
    }
}

impl From<Error> for CommandError {
    fn from(e: Error) -> Self {
        let message = e.to_string();
        match e {
            Error::GitNotFound { searched } => CommandError::new(
                "gitNotFound",
                "gitNotFound",
                json!({ "searched": searched }),
                message,
            ),
            Error::Spawn { git, source } => CommandError::new(
                "gitNotRunnable",
                "gitNotRunnable",
                json!({ "git": git }),
                message,
            )
            .with_detail(source.to_string()),
            Error::NotADirectory(path) => CommandError::new(
                "notADirectory",
                "notADirectory",
                json!({ "path": path }),
                message,
            ),
            Error::NotARepository(path) => CommandError::new(
                "notARepository",
                "notARepository",
                json!({ "path": path }),
                message,
            ),
            Error::Unsupported {
                feature,
                needs,
                have,
            } => CommandError::new(
                "unsupported",
                "unsupported",
                json!({ "feature": feature, "needs": needs.to_string(), "have": have.to_string() }),
                message,
            ),
            Error::MultipleValues { key } => CommandError::new(
                "multipleValues",
                "multipleValues",
                json!({ "key": key }),
                message,
            ),
            Error::Invalid(why) => {
                // Serializes as { "code": ..., fields... }: split the code off.
                let mut params = match serde_json::to_value(&why) {
                    Ok(Value::Object(map)) => map,
                    _ => Map::new(),
                };
                let code = match params.remove("code") {
                    Some(Value::String(code)) => code,
                    _ => "invalid".to_string(),
                };
                CommandError {
                    kind: "invalid",
                    code,
                    params,
                    message,
                    detail: None,
                }
            }
            Error::GitFailed { args, code, stderr } => CommandError::new(
                "gitFailed",
                "gitFailed",
                json!({ "command": format!("git {}", args.join(" ")), "exitCode": code }),
                message,
            )
            .with_detail(last_meaningful_line(&stderr)),
            Error::Parse(what) => {
                CommandError::new("internal", "unexpectedOutput", json!({}), message)
                    .with_detail(what)
            }
            Error::Io { path, source } => {
                CommandError::new("internal", "io", json!({ "path": path }), message)
                    .with_detail(source.to_string())
            }
        }
    }
}

impl From<gitid_core::Invalid> for CommandError {
    fn from(why: gitid_core::Invalid) -> Self {
        Error::Invalid(why).into()
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        CommandError::internal(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitid_core::{Feature, GitVersion, Invalid};

    fn json_of(e: impl Into<CommandError>) -> Value {
        serde_json::to_value(e.into()).unwrap()
    }

    /// One of every error the frontend can receive.
    fn every_error() -> Vec<CommandError> {
        let p = || std::path::PathBuf::from("/x");
        let s = || "x".to_string();
        let io = || std::io::Error::other("x");
        let v = || GitVersion::new(2, 36, 0);
        let invalid = vec![
            Invalid::EmptyKey,
            Invalid::BadKey {
                key: s(),
                detail: s(),
            },
            Invalid::PathNotUtf8 { path: p() },
            Invalid::StaleValue,
            Invalid::NeedsRepoFolder,
            Invalid::UnknownSetting { key: s() },
            Invalid::NotBool { key: s() },
            Invalid::NotOneOf {
                key: s(),
                allowed: vec![],
            },
            Invalid::LineBreak { key: s() },
            Invalid::NameAngleBrackets,
            Invalid::BadEmail { email: s() },
            Invalid::BadBranchName { name: s() },
            Invalid::BadRuleId { id: s() },
            Invalid::RuleNeedsIdentity,
            Invalid::BadRuleName,
            Invalid::RuleNeedsWhere,
            Invalid::EmptyPattern,
            Invalid::EmptyCondition,
            Invalid::BadServer { input: s() },
            Invalid::BadPort { input: s() },
            Invalid::NoSuchRule {
                id: s(),
                known: vec![],
            },
            Invalid::NoHome,
            Invalid::NotGit { path: p() },
        ];
        let mut errors = vec![
            Error::NotARepository(p()),
            Error::Unsupported {
                feature: Feature::RemoteRules,
                needs: v(),
                have: v(),
            },
            Error::MultipleValues { key: s() },
            Error::Io {
                path: p(),
                source: io(),
            },
            Error::GitNotFound { searched: vec![] },
            Error::Spawn {
                git: p(),
                source: io(),
            },
            Error::GitFailed {
                args: vec![],
                code: None,
                stderr: s(),
            },
            Error::Parse(s()),
            Error::NotADirectory(p()),
        ];
        // Not a test of anything at run time: if a variant is added to
        // Error or Invalid, this match stops compiling until it's listed
        // above too, and so until the catalog test below covers it.
        for e in &errors {
            match e {
                Error::NotARepository(_)
                | Error::Unsupported { .. }
                | Error::MultipleValues { .. }
                | Error::Invalid(_)
                | Error::Io { .. }
                | Error::GitNotFound { .. }
                | Error::Spawn { .. }
                | Error::GitFailed { .. }
                | Error::Parse(_)
                | Error::NotADirectory(_) => {}
            }
        }
        for i in &invalid {
            match i {
                Invalid::EmptyKey
                | Invalid::BadKey { .. }
                | Invalid::PathNotUtf8 { .. }
                | Invalid::StaleValue
                | Invalid::NeedsRepoFolder
                | Invalid::UnknownSetting { .. }
                | Invalid::NotBool { .. }
                | Invalid::NotOneOf { .. }
                | Invalid::LineBreak { .. }
                | Invalid::NameAngleBrackets
                | Invalid::BadEmail { .. }
                | Invalid::BadBranchName { .. }
                | Invalid::BadRuleId { .. }
                | Invalid::RuleNeedsIdentity
                | Invalid::BadRuleName
                | Invalid::RuleNeedsWhere
                | Invalid::EmptyPattern
                | Invalid::EmptyCondition
                | Invalid::BadServer { .. }
                | Invalid::BadPort { .. }
                | Invalid::NoSuchRule { .. }
                | Invalid::NoHome
                | Invalid::NotGit { .. } => {}
            }
        }
        errors.extend(invalid.into_iter().map(Error::Invalid));
        let mut out: Vec<CommandError> = errors.into_iter().map(Into::into).collect();
        // The app's own.
        out.push(CommandError::save_settings(io()));
        out.push(CommandError::internal("x"));
        out.push(CommandError::invalid(
            "badLanguageTag",
            json!({ "tag": "x" }),
            s(),
        ));
        out
    }

    /// Every error code the backend can send has an English message, and
    /// every placeholder in it is a param the backend actually sends.
    #[test]
    fn every_error_has_an_english_message() {
        let en: Map<String, Value> =
            serde_json::from_str(include_str!("../../src/locales/en.json")).unwrap();
        for e in every_error() {
            let key = format!("error.{}", e.code);
            let Some(Value::String(text)) = en.get(&key) else {
                panic!("src/locales/en.json has no \"{key}\" (for: {})", e.message);
            };
            for name in placeholders(text) {
                assert!(
                    e.params.contains_key(&name),
                    "\"{key}\" uses {{{name}}}, which the backend doesn't send"
                );
            }
        }
        match Feature::RemoteRules {
            Feature::RemoteRules => assert!(en.contains_key("feature.remoteRules")),
        }
    }

    fn placeholders(text: &str) -> Vec<String> {
        text.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}').map(|(name, _)| name.to_string()))
            .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric()))
            .collect()
    }

    #[test]
    fn invalid_input_becomes_code_and_params() {
        let v = json_of(Error::Invalid(Invalid::BadEmail {
            email: "ann".into(),
        }));
        assert_eq!(
            v,
            json!({
                "kind": "invalid",
                "code": "badEmail",
                "params": { "email": "ann" },
                "message": "\"ann\" doesn't look like an email address",
                "detail": null,
            })
        );
    }

    #[test]
    fn git_failure_keeps_gits_words_as_detail() {
        let v = json_of(Error::GitFailed {
            args: vec!["config".into(), "--global".into()],
            code: Some(4),
            stderr: "hint: x\nerror: could not lock config file\n".into(),
        });
        assert_eq!(v["code"], "gitFailed");
        assert_eq!(v["params"]["command"], "git config --global");
        assert_eq!(v["params"]["exitCode"], 4);
        assert_eq!(v["detail"], "error: could not lock config file");
    }

    #[test]
    fn unsupported_names_the_feature_by_code() {
        let v = json_of(Error::Unsupported {
            feature: Feature::RemoteRules,
            needs: GitVersion::new(2, 36, 0),
            have: GitVersion::new(2, 34, 1),
        });
        assert_eq!(
            v["params"],
            json!({ "feature": "remoteRules", "needs": "2.36.0", "have": "2.34.1" })
        );
    }
}

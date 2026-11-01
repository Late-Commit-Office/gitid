//! Pins the JSON shape the app's frontend relies on (app/src/lib/types.ts).
//! If a change here is intended, update types.ts in the same commit.
#![cfg(feature = "serde")]

mod common;

use common::TestEnv;
use gitid_core::rules::{self, NewRule, When};
use gitid_core::{effective_identity, GitVersion};
use serde_json::{json, Value};

fn at<'a>(v: &'a Value, pointer: &str) -> &'a Value {
    v.pointer(pointer)
        .unwrap_or_else(|| panic!("missing {pointer} in {v:#}"))
}

#[test]
fn identity_report_shape() {
    let t = TestEnv::new("json-identity");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.useConfigOnly", "true");
    let repo = t.init_repo("proj");
    let git = t.git_with(
        "GIT_CONFIG_PARAMETERS",
        "'http.extraheader'='Authorization: secret'",
    );

    let id = effective_identity(&git, &repo).unwrap();
    let v = serde_json::to_value(id.report()).unwrap();

    // Flattened identity fields, camelCase.
    assert!(at(&v, "/folder").is_string());
    assert!(at(&v, "/repo/workTree").is_string());
    assert!(at(&v, "/repo/gitDir").is_string());
    assert_eq!(at(&v, "/useConfigOnly"), &json!(true));
    assert_eq!(at(&v, "/hasProblems"), &json!(true));

    // Role identity with a config source (internally tagged, entry flattened in).
    assert_eq!(at(&v, "/author/role"), "author");
    assert_eq!(at(&v, "/author/name/value"), "Ann Lee");
    assert_eq!(at(&v, "/author/name/source/kind"), "config");
    assert_eq!(at(&v, "/author/name/source/key"), "user.name");
    assert_eq!(at(&v, "/author/name/source/scope"), "global");
    assert_eq!(at(&v, "/author/name/source/origin/kind"), "file");
    assert!(at(&v, "/author/name/source/origin/value").is_string());
    // Missing email: null, and the error explains why.
    assert_eq!(at(&v, "/author/email"), &Value::Null);
    assert!(at(&v, "/author/error").is_string());

    // Warnings: severity, the tagged warning, and its sentence.
    assert_eq!(at(&v, "/warnings/0/severity"), "problem");
    assert_eq!(at(&v, "/warnings/0/warning/kind"), "missing");
    assert_eq!(at(&v, "/warnings/0/warning/role"), "author");
    assert!(at(&v, "/warnings/0/message").is_string());

    // Sensitive environment values are never serialized.
    let env = at(&v, "/env").as_array().unwrap();
    let params = env
        .iter()
        .find(|e| e["name"] == "GIT_CONFIG_PARAMETERS")
        .unwrap();
    assert_eq!(params["sensitive"], json!(true));
    assert_eq!(params["value"], "");
    assert!(
        !v.to_string().contains("secret"),
        "a secret leaked into the JSON"
    );
}

#[test]
fn env_and_guessed_sources_and_warning_kinds() {
    let t = TestEnv::new("json-env");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@example.com");
    let repo = t.init_repo("proj");
    let git = t.git_with("GIT_COMMITTER_EMAIL", "bot@ci.example");

    let v = serde_json::to_value(effective_identity(&git, &repo).unwrap().report()).unwrap();

    assert_eq!(
        at(&v, "/committer/email/source"),
        &json!({ "kind": "env", "var": "GIT_COMMITTER_EMAIL" })
    );
    let kinds: Vec<&str> = at(&v, "/warnings")
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["warning"]["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"authorCommitterDiffer"), "{kinds:?}");
    assert!(kinds.contains(&"fromEnv"), "{kinds:?}");
    assert!(kinds.contains(&"guessingAllowed"), "{kinds:?}");
    let from_env = at(&v, "/warnings")
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["warning"]["kind"] == "fromEnv")
        .unwrap();
    assert_eq!(from_env["severity"], "note");
    assert_eq!(from_env["warning"]["part"], "email");
    assert_eq!(from_env["warning"]["var"], "GIT_COMMITTER_EMAIL");
}

#[test]
fn rule_and_condition_shape() {
    let t = TestEnv::new("json-rule");
    let work = t.mkdir("home/work");
    let version = GitVersion::detect(t.git()).unwrap();
    let rule = rules::add_rule(
        t.git(),
        version,
        &NewRule {
            id: "work".into(),
            name: None,
            email: Some("a@corp.example".into()),
            when: vec![When::Folder(work)],
        },
    )
    .unwrap();

    let v = serde_json::to_value(&rule).unwrap();
    assert_eq!(at(&v, "/id"), "work");
    assert!(at(&v, "/includePath").is_string());
    assert_eq!(at(&v, "/fileExists"), &json!(true));
    assert_eq!(at(&v, "/managed"), &json!(true));
    assert_eq!(at(&v, "/conditions/0/kind"), "folder");
    assert_eq!(at(&v, "/conditions/0/pattern"), "~/work/");
    assert!(at(&v, "/conditions/0/caseInsensitive").is_boolean());

    let other = serde_json::to_value(rules::Condition::parse("onbranch:main")).unwrap();
    assert_eq!(other, json!({ "kind": "other", "raw": "onbranch:main" }));
    let remote = serde_json::to_value(rules::Condition::parse(
        "hasconfig:remote.*.url:https://h/**",
    ))
    .unwrap();
    assert_eq!(
        remote,
        json!({ "kind": "remoteUrl", "pattern": "https://h/**" })
    );
}

#[test]
fn settings_shape() {
    let t = TestEnv::new("json-settings");
    t.global_set("user.name", "Ann Lee");
    t.global_set("pull.rebase", "true");
    let all = gitid_core::settings::read(t.git()).unwrap();
    let v = serde_json::to_value(&all).unwrap();

    let find = |key: &str| {
        v.as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == key)
            .unwrap_or_else(|| panic!("no {key} in {v:#}"))
            .clone()
    };
    let name = find("user.name");
    assert_eq!(name["label"], "Name");
    assert_eq!(name["group"], "identity");
    assert_eq!(name["kind"]["kind"], "text");
    assert!(name["kind"]["placeholder"].is_string());
    assert_eq!(name["value"], "Ann Lee");
    assert_eq!(name["origin"]["kind"], "file");

    let guard = find("user.useConfigOnly");
    assert_eq!(guard["kind"], json!({ "kind": "bool", "default": false }));
    assert_eq!(guard["value"], Value::Null);
    assert_eq!(guard["origin"], Value::Null);

    let pull = find("pull.rebase");
    assert_eq!(pull["kind"]["kind"], "choice");
    assert_eq!(pull["kind"]["default"], "false");
    assert_eq!(
        pull["kind"]["choices"][0],
        json!({ "value": "false", "label": "Merge" })
    );
    assert_eq!(pull["value"], "true");
    assert_eq!(pull["group"], "defaults");
}

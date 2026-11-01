mod common;

use common::TestEnv;
use gitid_core::settings::{self, Kind};
use gitid_core::{effective_identity, platform, Error};

fn value(t: &TestEnv, key: &str) -> Option<String> {
    settings::read(t.git())
        .unwrap()
        .into_iter()
        .find(|s| s.setting.key == key)
        .unwrap()
        .value
}

#[test]
fn read_reports_every_managed_setting() {
    let t = TestEnv::new("s-read");
    t.global_set("user.name", "Ann Lee");
    t.global_set("init.defaultbranch", "main"); // case differs from the key list

    let all = settings::read(t.git()).unwrap();
    assert_eq!(all.len(), settings::SETTINGS.len());
    assert_eq!(value(&t, "user.name").as_deref(), Some("Ann Lee"));
    assert_eq!(value(&t, "init.defaultBranch").as_deref(), Some("main"));
    assert_eq!(value(&t, "pull.rebase"), None);
    let name = all.iter().find(|s| s.setting.key == "user.name").unwrap();
    assert_eq!(
        name.origin.as_ref().and_then(|o| o.file()),
        Some(t.global.as_path())
    );
}

#[test]
fn guard_toggle_changes_what_git_does() {
    let t = TestEnv::new("s-guard");
    t.global_set("user.name", "Ann Lee"); // no email
    let repo = t.init_repo("proj");

    settings::apply(t.git(), "user.useConfigOnly", Some("true")).unwrap();
    assert_eq!(value(&t, "user.useConfigOnly").as_deref(), Some("true"));
    let id = effective_identity(t.git(), &repo).unwrap();
    assert!(id.use_config_only && !id.author.is_usable());

    // Back to git's default removes the line instead of writing "false".
    settings::apply(t.git(), "user.useConfigOnly", Some("false")).unwrap();
    assert_eq!(value(&t, "user.useConfigOnly"), None);
    assert!(!std::fs::read_to_string(&t.global)
        .unwrap()
        .to_lowercase()
        .contains("useconfigonly"));
}

#[test]
fn text_choice_and_removal() {
    let t = TestEnv::new("s-text");
    settings::apply(t.git(), "user.email", Some("  ann@example.com  ")).unwrap();
    assert_eq!(value(&t, "user.email").as_deref(), Some("ann@example.com"));

    settings::apply(t.git(), "pull.rebase", Some("merges")).unwrap();
    assert_eq!(value(&t, "pull.rebase").as_deref(), Some("merges"));
    settings::apply(t.git(), "pull.rebase", Some("false")).unwrap(); // the default
    assert_eq!(value(&t, "pull.rebase"), None);

    settings::apply(t.git(), "core.editor", Some("code --wait")).unwrap();
    assert_eq!(value(&t, "core.editor").as_deref(), Some("code --wait"));
    settings::apply(t.git(), "core.editor", Some("")).unwrap();
    assert_eq!(value(&t, "core.editor"), None);

    settings::apply(t.git(), "user.email", None).unwrap();
    assert_eq!(value(&t, "user.email"), None);
}

#[test]
fn repeated_lines_are_replaced_not_refused() {
    let t = TestEnv::new("s-repeat");
    t.sh(
        None,
        &["config", "--global", "--add", "init.defaultBranch", "trunk"],
    );
    t.sh(
        None,
        &["config", "--global", "--add", "init.defaultBranch", "dev"],
    );
    settings::apply(t.git(), "init.defaultBranch", Some("main")).unwrap();
    assert_eq!(value(&t, "init.defaultBranch").as_deref(), Some("main"));
    let text = std::fs::read_to_string(&t.global).unwrap();
    assert_eq!(text.matches("defaultBranch").count(), 1, "{text}");
}

#[test]
fn bad_values_are_rejected_before_writing() {
    let t = TestEnv::new("s-bad");
    let cases = [
        ("user.email", "not an email"),
        ("user.name", "Ann <x>"),
        ("user.name", "two\nlines"),
        ("user.useConfigOnly", "maybe"),
        ("user.useConfigOnly", ""),
        ("pull.rebase", "sometimes"),
        ("init.defaultBranch", "bad..name"),
        ("init.defaultBranch", "-x"),
    ];
    for (key, v) in cases {
        let err = settings::apply(t.git(), key, Some(v)).unwrap_err();
        assert!(matches!(err, Error::Invalid(_)), "{key}={v:?}: {err:?}");
    }
    let err = settings::apply(t.git(), "core.pager", Some("less")).unwrap_err();
    assert!(matches!(err, Error::Invalid(_)));
    assert_eq!(std::fs::read_to_string(&t.global).unwrap(), "");
}

#[test]
fn every_setting_accepts_a_sensible_value() {
    let t = TestEnv::new("s-all");
    for s in settings::SETTINGS {
        let v = match s.kind {
            Kind::Bool { default } => (!default).to_string(),
            Kind::Choice { choices, default } => choices
                .iter()
                .find(|c| c.value != default)
                .unwrap()
                .value
                .to_string(),
            Kind::Text { .. } if s.key == "user.email" => "a@b.example".into(),
            Kind::Text { .. } => "main".into(),
        };
        settings::apply(t.git(), s.key, Some(&v)).unwrap_or_else(|e| panic!("{}: {e}", s.key));
        assert_eq!(value(&t, s.key), Some(v), "{}", s.key);
    }
}

#[test]
fn global_file_follows_gits_rule() {
    let t = TestEnv::new("s-globalfile");
    // With GIT_CONFIG_GLOBAL set (as in every test), that's the file.
    assert_eq!(
        platform::global_config_file(t.git()),
        Some(t.global.clone())
    );

    // Without it: ~/.gitconfig if present...
    let plain = t.git().clone().without_env("GIT_CONFIG_GLOBAL");
    assert_eq!(
        platform::global_config_file(&plain),
        Some(t.home.join(".gitconfig"))
    );

    // ...otherwise the XDG file if *it* exists, and git really writes there.
    std::fs::remove_file(t.home.join(".gitconfig")).unwrap();
    let xdg = t.home.join(".config/git/config");
    std::fs::create_dir_all(xdg.parent().unwrap()).unwrap();
    std::fs::write(&xdg, "").unwrap();
    assert_eq!(platform::global_config_file(&plain), Some(xdg.clone()));
    gitid_core::config::set(
        &plain,
        &gitid_core::config::Target::Global,
        "user.name",
        "Ann",
    )
    .unwrap();
    assert!(std::fs::read_to_string(&xdg).unwrap().contains("Ann"));
    assert!(!t.home.join(".gitconfig").exists());
}

mod common;

use common::TestEnv;
use gitid_core::{effective_identity, Error, GitVersion, Part, Scope, Source, Warning};

fn config_source(src: &Source) -> &gitid_core::Entry {
    match src {
        Source::Config(e) => e,
        other => panic!("expected a config source, got {other:?}"),
    }
}

#[test]
fn global_identity_is_reported_with_its_file() {
    let t = TestEnv::new("global");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@example.com");
    let repo = t.init_repo("proj");

    let id = effective_identity(t.git(), &repo).unwrap();

    assert_eq!(
        id.author.display().as_deref(),
        Some("Ann Lee <ann@example.com>")
    );
    assert!(id.author_matches_committer());
    let name = config_source(&id.author.name.as_ref().unwrap().source);
    assert_eq!(name.scope, Scope::Global);
    assert_eq!(name.origin.file(), Some(t.global.as_path()));
    assert_eq!(id.repo.unwrap().work_tree.as_deref(), Some(repo.as_path()));
}

#[test]
fn local_overrides_global_and_path_is_absolute() {
    let t = TestEnv::new("local");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@home.example");
    let repo = t.init_repo("proj");
    t.sh(Some(&repo), &["config", "user.email", "ann@work.example"]);
    let deep = t.mkdir("proj/src/deep");

    // Ask from a subfolder: the local file must still resolve to an absolute path.
    let id = effective_identity(t.git(), &deep).unwrap();

    let email = id.author.email.as_ref().unwrap();
    assert_eq!(email.value, "ann@work.example");
    let entry = config_source(&email.source);
    assert_eq!(entry.scope, Scope::Local);
    let file = entry.origin.file().unwrap();
    assert!(file.is_absolute(), "{}", file.display());
    assert!(file.is_file(), "{} should exist", file.display());
    assert_eq!(file, repo.join(".git").join("config"));
}

#[test]
fn include_if_gitdir_applies_only_inside_its_folder() {
    let t = TestEnv::new("gitdir");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@home.example");
    let work_cfg = t.write_file(
        "home/.config/git/work.gitconfig",
        "[user]\n\temail = ann@corp.example\n",
    );
    let work = t.mkdir("work");
    t.global_set(
        &format!("includeIf.gitdir:{}.path", t.gitdir_pattern(&work)),
        &work_cfg.to_string_lossy(),
    );
    let inside = t.init_repo("work/api");
    let outside = t.init_repo("personal/blog");

    let in_id = effective_identity(t.git(), &inside).unwrap();
    let email = in_id.author.email.as_ref().unwrap();
    assert_eq!(email.value, "ann@corp.example");
    let entry = config_source(&email.source);
    assert_eq!(entry.origin.file(), Some(work_cfg.as_path()));
    // Included files report the scope of the file that included them.
    assert_eq!(entry.scope, Scope::Global);

    let out_id = effective_identity(t.git(), &outside).unwrap();
    assert_eq!(
        out_id.author.email.as_ref().unwrap().value,
        "ann@home.example"
    );
}

#[test]
fn include_if_hasconfig_matches_remote_url() {
    let t = TestEnv::new("hasconfig");
    let v = GitVersion::detect(t.git()).unwrap();
    if !v.supports_hasconfig() {
        eprintln!("skipping: git {v} has no hasconfig");
        return;
    }
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@home.example");
    let gitea_cfg = t.write_file(
        "home/gitea.gitconfig",
        "[user]\n\tname = annlee\n\temail = annlee@gitea.example\n",
    );
    t.global_set(
        "includeIf.hasconfig:remote.*.url:https://gitea.example/**.path",
        &gitea_cfg.to_string_lossy(),
    );
    let repo = t.init_repo("code/app");
    t.sh(
        Some(&repo),
        &[
            "remote",
            "add",
            "origin",
            "https://gitea.example/ann/app.git",
        ],
    );
    let other = t.init_repo("code/other");

    let id = effective_identity(t.git(), &repo).unwrap();
    assert_eq!(
        id.author.display().as_deref(),
        Some("annlee <annlee@gitea.example>")
    );

    let id = effective_identity(t.git(), &other).unwrap();
    assert_eq!(
        id.author.display().as_deref(),
        Some("Ann Lee <ann@home.example>")
    );
}

#[test]
fn use_config_only_makes_missing_identity_an_explicit_problem() {
    let t = TestEnv::new("configonly");
    t.global_set("user.useConfigOnly", "true");
    t.global_set("user.name", "Ann Lee"); // email deliberately missing
    let repo = t.init_repo("proj");

    let id = effective_identity(t.git(), &repo).unwrap();

    assert!(id.use_config_only);
    assert!(!id.author.is_usable());
    assert!(
        id.author.error.as_deref().unwrap().contains("auto-detect"),
        "{:?}",
        id.author.error
    );
    // The configured half is still shown, the missing half is None.
    assert_eq!(id.author.name.as_ref().unwrap().value, "Ann Lee");
    assert!(id.author.email.is_none());
    assert!(id.has_problems());
    assert!(matches!(id.warnings()[0], Warning::Missing { .. }));
    assert!(!id.warnings().contains(&Warning::GuessingAllowed));
}

#[test]
fn nothing_configured_is_flagged_as_guessed_or_missing() {
    // Whether git can guess depends on the machine (hostname, passwd entry),
    // so accept either outcome; both must be reported as problems.
    let t = TestEnv::new("guess");
    let repo = t.init_repo("proj");

    let id = effective_identity(t.git(), &repo).unwrap();
    let ws = id.warnings();
    assert!(id.has_problems(), "{ws:?}");
    if id.author.is_usable() {
        assert_eq!(id.author.name.as_ref().unwrap().source, Source::Guessed);
        assert!(ws.contains(&Warning::Guessed {
            role: gitid_core::Role::Author,
            part: Part::Name
        }));
    } else {
        assert!(ws.iter().any(|w| matches!(w, Warning::Missing { .. })));
    }
    assert!(ws.contains(&Warning::GuessingAllowed));
}

#[test]
fn environment_override_is_attributed_to_the_variable() {
    let t = TestEnv::new("env");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@example.com");
    let repo = t.init_repo("proj");
    let git = t.git_with("GIT_AUTHOR_NAME", "Script Bot");

    let id = effective_identity(&git, &repo).unwrap();

    let name = id.author.name.as_ref().unwrap();
    assert_eq!(name.value, "Script Bot");
    assert_eq!(
        name.source,
        Source::Env {
            var: "GIT_AUTHOR_NAME".into()
        }
    );
    // Committer is unaffected.
    assert_eq!(id.committer.name.as_ref().unwrap().value, "Ann Lee");
    assert!(!id.author_matches_committer());
    let ws = id.warnings();
    assert!(ws.contains(&Warning::AuthorCommitterDiffer));
    assert!(ws
        .iter()
        .any(|w| matches!(w, Warning::FromEnv { var, .. } if var == "GIT_AUTHOR_NAME")));
    assert!(id.env.iter().any(|v| v.name == "GIT_AUTHOR_NAME"));
}

#[test]
fn injected_command_config_is_flagged() {
    let t = TestEnv::new("injected");
    t.global_set("user.name", "Ann Lee");
    let repo = t.init_repo("proj");
    let git = t
        .git()
        .clone()
        .with_env("GIT_CONFIG_COUNT", "1")
        .with_env("GIT_CONFIG_KEY_0", "user.email")
        .with_env("GIT_CONFIG_VALUE_0", "ci@example.com");

    let id = effective_identity(&git, &repo).unwrap();

    let email = id.author.email.as_ref().unwrap();
    assert_eq!(email.value, "ci@example.com");
    assert_eq!(config_source(&email.source).scope, Scope::Command);
    assert!(id
        .warnings()
        .iter()
        .any(|w| matches!(w, Warning::Injected { .. })));
}

#[test]
fn committer_specific_config_wins_for_committer_only() {
    let t = TestEnv::new("committer");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@example.com");
    t.global_set("committer.name", "Ann (laptop)");
    let repo = t.init_repo("proj");

    let id = effective_identity(t.git(), &repo).unwrap();

    assert_eq!(id.author.name.as_ref().unwrap().value, "Ann Lee");
    let c = id.committer.name.as_ref().unwrap();
    assert_eq!(c.value, "Ann (laptop)");
    assert_eq!(config_source(&c.source).key, "committer.name");
}

#[test]
fn works_outside_any_repository() {
    let t = TestEnv::new("norepo");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@example.com");
    t.global_set("user.useConfigOnly", "true");
    let plain = t.mkdir("downloads");

    let id = effective_identity(t.git(), &plain).unwrap();

    assert!(id.repo.is_none());
    assert_eq!(
        id.author.display().as_deref(),
        Some("Ann Lee <ann@example.com>")
    );
    assert!(!id.has_problems(), "{:?}", id.warnings());
    assert!(id.warnings().is_empty(), "{:?}", id.warnings());
}

#[test]
fn missing_folder_is_an_error() {
    let t = TestEnv::new("nofolder");
    let err = effective_identity(t.git(), &t.root.join("nope")).unwrap_err();
    assert!(matches!(err, Error::NotADirectory(_)));
}

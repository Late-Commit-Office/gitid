mod common;

use common::TestEnv;
use gitid_core::rules::{self, Condition, NewRule, When};
use gitid_core::{effective_identity, Error, GitVersion};

fn version(t: &TestEnv) -> GitVersion {
    GitVersion::detect(t.git()).unwrap()
}

fn email_in(t: &TestEnv, dir: &std::path::Path) -> Option<String> {
    let id = effective_identity(t.git(), dir).unwrap();
    id.author.email.map(|f| f.value)
}

fn base_identity(t: &TestEnv) {
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@home.example");
}

#[test]
fn folder_rule_applies_inside_only_and_lists_back() {
    let t = TestEnv::new("r-folder");
    base_identity(&t);
    let work = t.mkdir("home/work");
    let inside = t.init_repo("home/work/api");
    let outside = t.init_repo("home/personal/blog");

    let rule = rules::add_rule(
        t.git(),
        version(&t),
        &NewRule {
            id: "work".into(),
            name: None,
            email: Some("ann@corp.example".into()),
            when: vec![When::Folder(work.clone())],
        },
    )
    .unwrap();

    // Written under home, as ~/ paths.
    assert_eq!(rule.id, "work");
    assert!(rule.managed && rule.file_exists);
    assert_eq!(rule.include_path, "~/.config/git/identities/work.gitconfig");
    assert_eq!(
        rule.file,
        t.home.join(".config/git/identities/work.gitconfig")
    );
    match &rule.conditions[..] {
        [Condition::Folder { pattern, .. }] => assert_eq!(pattern, "~/work/"),
        other => panic!("{other:?}"),
    }
    assert_eq!(rule.email.as_deref(), Some("ann@corp.example"));
    assert_eq!(rule.name, None);

    assert_eq!(email_in(&t, &inside).as_deref(), Some("ann@corp.example"));
    assert_eq!(email_in(&t, &outside).as_deref(), Some("ann@home.example"));
    assert!(rules::applies_in(t.git(), &rule, &inside).unwrap());
    assert!(!rules::applies_in(t.git(), &rule, &outside).unwrap());

    assert_eq!(rules::list_rules(t.git()).unwrap(), vec![rule]);
}

#[test]
fn folder_rule_on_symlink_uses_real_path() {
    #[cfg(unix)]
    {
        let t = TestEnv::new("r-symlink");
        base_identity(&t);
        let real = t.mkdir("real-code");
        let link = t.home.join("code");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let via_link = t.init_repo("home/code/app");

        rules::add_rule(
            t.git(),
            version(&t),
            &NewRule {
                id: "linked".into(),
                name: None,
                email: Some("ann@linked.example".into()),
                when: vec![When::Folder(link)],
            },
        )
        .unwrap();

        assert_eq!(
            email_in(&t, &via_link).as_deref(),
            Some("ann@linked.example")
        );
        assert_eq!(
            email_in(&t, &real.join("app")).as_deref(),
            Some("ann@linked.example")
        );
    }
}

#[test]
fn host_rule_matches_every_url_form() {
    let t = TestEnv::new("r-host");
    if !version(&t).supports_hasconfig() {
        return;
    }
    base_identity(&t);
    let rule = rules::add_rule(
        t.git(),
        version(&t),
        &NewRule {
            id: "gitea".into(),
            name: Some("annlee".into()),
            email: Some("annlee@gitea.example".into()),
            when: vec![When::RemoteHost {
                host: "gitea.example".into(),
                ssh_port: Some(2222),
            }],
        },
    )
    .unwrap();
    assert_eq!(rule.conditions.len(), 5);

    let matching = [
        "https://gitea.example/ann/a.git",
        "http://gitea.example/ann/a.git",
        "https://annlee:token123@gitea.example/ann/a.git",
        "ssh://git@gitea.example/ann/a.git",
        "ssh://git@gitea.example:2222/ann/a.git",
        "git@gitea.example:ann/a.git",
        "git@gitea.example:org/sub/a.git",
    ];
    let not_matching = [
        "https://github.com/ann/a.git",
        "https://gitea.example.evil.com/ann/a.git",
        "git@github.com:ann/a.git",
    ];
    for (i, url) in matching.iter().chain(not_matching.iter()).enumerate() {
        let repo = t.init_repo(&format!("code/r{i}"));
        t.sh(Some(&repo), &["remote", "add", "origin", url]);
        let expect = matching.contains(url);
        let got = effective_identity(t.git(), &repo).unwrap().author.display();
        let want = if expect {
            "annlee <annlee@gitea.example>"
        } else {
            "Ann Lee <ann@home.example>"
        };
        assert_eq!(got.as_deref(), Some(want), "{url}");
        assert_eq!(
            rules::applies_in(t.git(), &rule, &repo).unwrap(),
            expect,
            "{url}"
        );
    }
}

#[test]
fn adding_twice_updates_identity_without_duplicating_includes() {
    let t = TestEnv::new("r-twice");
    base_identity(&t);
    let work = t.mkdir("home/work");
    let new = |email: &str| NewRule {
        id: "work".into(),
        name: Some("Ann Lee".into()),
        email: Some(email.into()),
        when: vec![When::Folder(work.clone())],
    };
    rules::add_rule(t.git(), version(&t), &new("old@corp.example")).unwrap();
    let rule = rules::add_rule(t.git(), version(&t), &new("new@corp.example")).unwrap();

    assert_eq!(rule.email.as_deref(), Some("new@corp.example"));
    assert_eq!(rule.conditions.len(), 1);
    let global = std::fs::read_to_string(&t.global).unwrap();
    assert_eq!(global.matches("work.gitconfig").count(), 1, "{global}");
}

#[test]
fn remove_rule_cleans_up_and_keeps_other_rules() {
    let t = TestEnv::new("r-remove");
    base_identity(&t);
    let work = t.mkdir("home/work");
    let oss = t.mkdir("home/oss");
    let repo = t.init_repo("home/work/api");
    for (id, dir) in [("work", &work), ("oss", &oss)] {
        rules::add_rule(
            t.git(),
            version(&t),
            &NewRule {
                id: id.into(),
                name: None,
                email: Some(format!("ann@{id}.example")),
                when: vec![When::Folder(dir.clone())],
            },
        )
        .unwrap();
    }
    // A hand-written rule (not in the identities folder) is listed too.
    let custom = t.write_file("home/custom.inc", "[user]\n\temail = ann@custom.example\n");
    t.global_set("includeIf.onbranch:release.path", &custom.to_string_lossy());

    let all = rules::list_rules(t.git()).unwrap();
    assert_eq!(
        all.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["work", "oss", "custom"]
    );
    assert!(!all[2].managed);
    assert!(matches!(all[2].conditions[0], Condition::Other { .. }));

    let work_rule = rules::find(&all, "work").unwrap();
    rules::remove_rule(t.git(), work_rule, true).unwrap();
    assert!(!work_rule.file.exists());
    assert_eq!(email_in(&t, &repo).as_deref(), Some("ann@home.example"));

    // Removing an unmanaged rule never deletes its file.
    rules::remove_rule(t.git(), &all[2], true).unwrap();
    assert!(custom.exists());

    let left = rules::list_rules(t.git()).unwrap();
    assert_eq!(
        left.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["oss"]
    );
    let global = std::fs::read_to_string(&t.global).unwrap();
    assert!(
        !global.contains("work.gitconfig") && !global.contains("onbranch"),
        "{global}"
    );
}

#[test]
fn bad_input_writes_nothing() {
    let t = TestEnv::new("r-bad");
    let before = std::fs::read_to_string(&t.global).unwrap();
    let cases = [
        NewRule {
            id: "../evil".into(),
            name: Some("A".into()),
            email: None,
            when: vec![When::Folder(t.root.clone())],
        },
        NewRule {
            id: "x".into(),
            name: None,
            email: None,
            when: vec![When::Folder(t.root.clone())],
        },
        NewRule {
            id: "x".into(),
            name: Some("A".into()),
            email: None,
            when: vec![],
        },
        NewRule {
            id: "x".into(),
            name: Some("A".into()),
            email: None,
            // A valid folder plus an invalid one: nothing at all must be written.
            when: vec![
                When::Folder(t.root.clone()),
                When::Folder(t.root.join("missing")),
            ],
        },
    ];
    for new in &cases {
        assert!(
            rules::add_rule(t.git(), version(&t), new).is_err(),
            "{new:?}"
        );
    }
    assert_eq!(std::fs::read_to_string(&t.global).unwrap(), before);
    assert!(!t.home.join(".config/git/identities").exists());
}

#[test]
fn old_git_refuses_remote_rules() {
    let t = TestEnv::new("r-oldgit");
    let err = rules::add_rule(
        t.git(),
        GitVersion::new(2, 30, 0),
        &NewRule {
            id: "g".into(),
            name: Some("A".into()),
            email: None,
            when: vec![When::RemoteHost {
                host: "h.example".into(),
                ssh_port: None,
            }],
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::Unsupported { .. }), "{err:?}");
}

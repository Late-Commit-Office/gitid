mod common;

use common::TestEnv;
use gitid_core::config::{self, read_target, Target};
use gitid_core::Error;

fn values(t: &TestEnv, target: &Target, key: &str) -> Vec<Option<String>> {
    read_target(t.git(), target)
        .unwrap()
        .into_iter()
        .filter(|e| e.key_matches(key))
        .map(|e| e.value)
        .collect()
}

#[test]
fn set_add_and_unset_on_global() {
    let t = TestEnv::new("w-global");
    let g = Target::Global;

    config::set(t.git(), &g, "user.name", "Ann Lee").unwrap();
    config::set(t.git(), &g, "user.name", "Ann B. Lee").unwrap(); // replaces
    assert_eq!(values(&t, &g, "user.name"), [Some("Ann B. Lee".into())]);

    config::add(t.git(), &g, "x.multi", "a").unwrap();
    config::add(t.git(), &g, "x.multi", "b").unwrap();
    assert_eq!(values(&t, &g, "x.multi").len(), 2);
    // A plain set must refuse to pick one of several values.
    let err = config::set(t.git(), &g, "x.multi", "c").unwrap_err();
    assert!(matches!(err, Error::MultipleValues { .. }), "{err:?}");

    assert!(config::unset(t.git(), &g, "user.name").unwrap());
    assert!(
        !config::unset(t.git(), &g, "user.name").unwrap(),
        "second unset reports 'was not set'"
    );
    assert!(config::unset_all(t.git(), &g, "x.multi").unwrap());
    assert!(values(&t, &g, "x.multi").is_empty());
}

#[test]
fn unset_value_removes_only_the_exact_value() {
    let t = TestEnv::new("w-value");
    let g = Target::Global;
    // Values full of regex characters must be matched literally.
    config::add(t.git(), &g, "x.paths", "~/a.b/(x)*").unwrap();
    config::add(t.git(), &g, "x.paths", "~/aXb/(x)*").unwrap();

    assert!(config::unset_value(t.git(), &g, "x.paths", "~/a.b/(x)*").unwrap());
    assert_eq!(values(&t, &g, "x.paths"), [Some("~/aXb/(x)*".into())]);
    assert!(!config::unset_value(t.git(), &g, "x.paths", "nope").unwrap());
}

#[test]
fn awkward_values_round_trip() {
    let t = TestEnv::new("w-awkward");
    let g = Target::Global;
    for v in [
        "-starts-with-dash",
        "has \"quotes\" and ; semicolon # hash",
        "two\nlines",
        " padded ",
    ] {
        config::set(t.git(), &g, "x.v", v).unwrap();
        assert_eq!(values(&t, &g, "x.v"), [Some(v.to_string())], "{v:?}");
    }
}

#[test]
fn local_and_file_targets() {
    let t = TestEnv::new("w-local");
    let repo = t.init_repo("proj");
    let deep = t.mkdir("proj/src");

    config::set(
        t.git(),
        &Target::Local(deep.clone()),
        "user.email",
        "ann@repo.example",
    )
    .unwrap();
    assert_eq!(
        values(&t, &Target::Local(repo.clone()), "user.email"),
        [Some("ann@repo.example".into())]
    );
    assert!(
        values(&t, &Target::Global, "user.email").is_empty(),
        "must not leak into global"
    );

    let plain = t.mkdir("not-a-repo");
    let err = config::set(t.git(), &Target::Local(plain), "a.b", "c").unwrap_err();
    assert!(matches!(err, Error::NotARepository(_)), "{err:?}");

    let file = t.root.join("ids/one.gitconfig");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    config::set(t.git(), &Target::File(file.clone()), "user.name", "Ann").unwrap();
    assert!(file.is_file());
    assert_eq!(
        values(&t, &Target::File(file), "user.name"),
        [Some("Ann".into())]
    );
}

#[test]
fn bad_keys_are_input_errors() {
    let t = TestEnv::new("w-badkey");
    for key in ["nosection", "x.has space", ""] {
        let err = config::set(t.git(), &Target::Global, key, "v").unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{key:?}: {err:?}");
    }
}

#[test]
fn missing_file_reads_as_empty() {
    let t = TestEnv::new("w-missing");
    let target = Target::File(t.root.join("nope/x.gitconfig"));
    assert!(read_target(t.git(), &target).unwrap().is_empty());
}

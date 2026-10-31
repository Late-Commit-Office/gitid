mod common;

use common::TestEnv;
use gitid_core::config::read_all;
use gitid_core::Scope;

#[test]
fn reads_every_scope_in_order() {
    let t = TestEnv::new("readall");
    t.global_set("user.name", "Ann Lee");
    t.global_set("init.defaultBranch", "main");
    let repo = t.init_repo("proj");
    t.sh(Some(&repo), &["config", "user.name", "Ann (repo)"]);

    let snap = read_all(t.git(), Some(&repo)).unwrap();

    let names = snap.get_all("user.name");
    assert_eq!(names.len(), 2);
    assert_eq!(names[0].scope, Scope::Global);
    assert_eq!(names[1].scope, Scope::Local);
    assert_eq!(
        snap.get("USER.NAME").unwrap().value.as_deref(),
        Some("Ann (repo)")
    );
    assert_eq!(
        snap.get("init.defaultbranch").unwrap().value.as_deref(),
        Some("main")
    );
}

#[test]
fn empty_config_outside_a_repo_is_fine() {
    let t = TestEnv::new("emptycfg");
    let dir = t.mkdir("nothing");
    let snap = read_all(t.git(), Some(&dir)).unwrap();
    assert!(snap.entries.is_empty(), "{:?}", snap.entries);
}

#[test]
fn awkward_values_survive_the_round_trip() {
    let t = TestEnv::new("awkward");
    t.write_file(
        "home/.gitconfig",
        "[user]\n\tname = \"Ann \\\"The\\\" Lee\" ; comment\n\
         [x]\n\tflag\n\
         [remote \"My Remote\"]\n\turl = https://h/x = y\n\
         [alias]\n\tlg = \"log --oneline\\n--graph\"\n",
    );
    let dir = t.mkdir("d");

    let snap = read_all(t.git(), Some(&dir)).unwrap();

    assert_eq!(
        snap.get("user.name").unwrap().value.as_deref(),
        Some("Ann \"The\" Lee")
    );
    assert_eq!(snap.get("x.flag").unwrap().value, None);
    assert_eq!(snap.get_bool("x.flag"), Some(true));
    assert_eq!(
        snap.get("remote.My Remote.url").unwrap().value.as_deref(),
        Some("https://h/x = y")
    );
    assert_eq!(
        snap.get("alias.lg").unwrap().value.as_deref(),
        Some("log --oneline\n--graph")
    );
}

mod common;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use common::TestEnv;
use gitid_core::rules::{self, NewRule, When};
use gitid_core::scan::{self, Reason, ScanOptions, Verdict};
use gitid_core::GitVersion;

fn no_progress(_: scan::Progress) {}

#[test]
fn sorts_repos_by_identity() {
    let t = TestEnv::new("scan-sort");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@home.example");
    let root = t.mkdir("code");
    t.init_repo("code/same");
    let local = t.init_repo("code/override");
    t.sh(Some(&local), &["config", "user.email", "ann@repo.example"]);
    let work = t.mkdir("code/work");
    t.init_repo("code/work/api");
    rules::add_rule(
        t.git(),
        GitVersion::detect(t.git()).unwrap(),
        &NewRule {
            id: "work".into(),
            name: None,
            email: Some("ann@corp.example".into()),
            when: vec![When::Folder(work)],
        },
    )
    .unwrap();
    // Not found: skipped folders, hidden folders, too deep, inside another repo.
    t.init_repo("code/node_modules/dep");
    t.init_repo("code/.cache/thing");
    t.init_repo("code/d1/d2/d3/d4/d5/d6/d7/deep");
    t.init_repo("code/same/vendored");

    let cancel = AtomicBool::new(false);
    let report = scan::scan(
        t.git(),
        &root,
        &ScanOptions::default(),
        &cancel,
        &no_progress,
    )
    .unwrap();

    let names: Vec<String> = report
        .findings
        .iter()
        .map(|f| {
            f.path
                .strip_prefix(&report.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    // Different first (by path), then same.
    assert_eq!(names, ["override", "work/api", "same"], "{report:#?}");
    assert_eq!(
        report.findings[0].verdict,
        Verdict::Different {
            because: vec![Reason::RepoConfig]
        }
    );
    assert_eq!(
        report.findings[1].verdict,
        Verdict::Different {
            because: vec![Reason::Rule { id: "work".into() }]
        }
    );
    assert_eq!(
        report.findings[1].identity.as_deref(),
        Some("Ann Lee <ann@corp.example>")
    );
    assert_eq!(report.findings[2].verdict, Verdict::Same);
    assert!(!report.cancelled);
}

#[test]
fn problems_come_first() {
    let t = TestEnv::new("scan-problems");
    t.global_set("user.useConfigOnly", "true");
    t.global_set("user.name", "Ann Lee"); // no email anywhere
    let root = t.mkdir("code");
    t.init_repo("code/b-broken");
    let ok = t.init_repo("code/a-fine");
    t.sh(Some(&ok), &["config", "user.email", "ann@repo.example"]);

    let cancel = AtomicBool::new(false);
    let report = scan::scan(
        t.git(),
        &root,
        &ScanOptions::default(),
        &cancel,
        &no_progress,
    )
    .unwrap();
    assert_eq!(report.findings.len(), 2);
    assert!(report.findings[0].is_problem());
    assert!(matches!(
        report.findings[0].verdict,
        Verdict::Missing { .. }
    ));
    assert!(report.findings[0].path.ends_with("b-broken"));
    assert!(!report.findings[1].is_problem());
}

#[test]
fn root_that_is_a_repo_and_symlinks() {
    let t = TestEnv::new("scan-root");
    t.global_set("user.name", "Ann Lee");
    t.global_set("user.email", "ann@home.example");
    let repo = t.init_repo("single");
    let cancel = AtomicBool::new(false);
    let report = scan::scan(
        t.git(),
        &repo,
        &ScanOptions::default(),
        &cancel,
        &no_progress,
    )
    .unwrap();
    assert_eq!(report.findings.len(), 1);

    #[cfg(unix)]
    {
        let root = t.mkdir("withlink");
        t.init_repo("withlink/real");
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap(); // must not hang
        let report = scan::scan(
            t.git(),
            &root,
            &ScanOptions::default(),
            &cancel,
            &no_progress,
        )
        .unwrap();
        assert_eq!(report.findings.len(), 1, "symlinks are not followed");
    }
}

#[test]
fn cancel_and_progress() {
    let t = TestEnv::new("scan-cancel");
    let root = t.mkdir("code");
    for i in 0..5 {
        t.init_repo(&format!("code/r{i}"));
    }
    let seen = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let report = scan::scan(t.git(), &root, &ScanOptions::default(), &cancel, &|p| {
        seen.lock().unwrap().push(p)
    })
    .unwrap();
    assert_eq!(report.findings.len(), 5);
    let progress = seen.into_inner().unwrap();
    assert_eq!(progress.iter().map(|p| p.checked).max(), Some(5));
    assert!(progress.iter().all(|p| p.found <= 5));

    let cancel = AtomicBool::new(true);
    let report = scan::scan(
        t.git(),
        &root,
        &ScanOptions::default(),
        &cancel,
        &no_progress,
    )
    .unwrap();
    assert!(report.cancelled);
    assert!(report.findings.is_empty());
}

//! `gitid scan`.

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use gitid_core::scan::{self, Finding, Progress, Reason, ScanOptions, Verdict};
use gitid_core::{paths, Error, Result, SystemGit};

use crate::style;
use crate::{EXIT_OK, EXIT_PROBLEMS};

fn reasons(because: &[Reason]) -> String {
    because
        .iter()
        .map(|r| match r {
            Reason::Rule { id } => format!("rule {id}"),
            Reason::RepoConfig => "the repo's own config".into(),
            Reason::Env { var } => format!("${var}"),
            Reason::OtherConfig { file: Some(f) } => style::tilde(f),
            Reason::OtherConfig { file: None } => "other config".into(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn line(f: &Finding, root: &Path) -> String {
    let rel = f
        .path
        .strip_prefix(root)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| style::tilde(&f.path), |p| p.display().to_string());
    let who = f.identity.as_deref().unwrap_or("(no identity)");
    match &f.verdict {
        Verdict::Missing { message } => {
            format!("  ! {rel:<28} git will refuse to commit: {message}")
        }
        Verdict::Guessed => format!("  ! {rel:<28} {who}  (guessed from your OS login)"),
        Verdict::Different { because } => {
            format!("    {rel:<28} {who}  ({})", reasons(because))
        }
        Verdict::Same => format!("    {rel:<28} {who}"),
    }
}

pub fn run(
    git: &SystemGit,
    path: &Path,
    all: bool,
    depth: usize,
    hidden: bool,
    json: bool,
) -> Result<u8> {
    let root = paths::canonicalize(path).map_err(|_| Error::NotADirectory(path.to_path_buf()))?;
    let opts = ScanOptions {
        max_depth: depth,
        include_hidden: hidden,
        ..ScanOptions::default()
    };
    let live = !json && std::io::stderr().is_terminal();
    let progress = |p: Progress| {
        if live {
            let mut err = std::io::stderr().lock();
            let _ = write!(
                err,
                "\r\x1b[2Kfound {} repos in {} folders, checked {}",
                p.found, p.folders_seen, p.checked
            );
            let _ = err.flush();
        }
    };
    let report = scan::scan(git, &root, &opts, &AtomicBool::new(false), &progress)?;
    if live {
        eprint!("\r\x1b[2K");
    }

    let problems = report.findings.iter().filter(|f| f.is_problem()).count();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("report serializes")
        );
        return Ok(if problems > 0 { EXIT_PROBLEMS } else { EXIT_OK });
    }

    let different: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| matches!(f.verdict, Verdict::Different { .. }))
        .collect();
    let same: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.verdict == Verdict::Same)
        .collect();

    println!(
        "{} repos under {}",
        report.findings.len(),
        style::tilde(&report.root)
    );
    if problems > 0 {
        println!("\nProblems ({problems})");
        for f in report.findings.iter().filter(|f| f.is_problem()) {
            println!("{}", line(f, &report.root));
        }
    }
    if !different.is_empty() {
        println!("\nNot your default identity ({})", different.len());
        for f in &different {
            println!("{}", line(f, &report.root));
        }
    }
    if !same.is_empty() {
        if all {
            println!("\nYour default identity ({})", same.len());
            for f in &same {
                println!("{}", line(f, &report.root));
            }
        } else {
            println!(
                "\n{} more use your default identity (--all lists them).",
                same.len()
            );
        }
    }
    if report.unreadable > 0 {
        println!(
            "\nSkipped {} folders that couldn't be read.",
            report.unreadable
        );
    }
    Ok(if problems > 0 { EXIT_PROBLEMS } else { EXIT_OK })
}

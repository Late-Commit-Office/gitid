//! `gitid rule list | add | remove | check`.

use std::path::{Path, PathBuf};

use gitid_core::rules::{self, Condition, NewRule, Rule, When};
use gitid_core::{paths, Error, GitVersion, Result, SystemGit};

use crate::style;
use crate::{EXIT_OK, EXIT_PROBLEMS};

fn real(p: &Path) -> PathBuf {
    paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn identity_line(r: &Rule) -> String {
    match (&r.name, &r.email) {
        (Some(n), Some(e)) => format!("{n} <{e}>"),
        (Some(n), None) => format!("{n} (sets the name only)"),
        (None, Some(e)) => format!("<{e}> (sets the email only)"),
        (None, None) if !r.file_exists => "(identity file is missing)".into(),
        (None, None) => "(sets no name or email)".into(),
    }
}

fn print_rule(n: Option<usize>, r: &Rule) {
    let n = n.map(|n| n.to_string()).unwrap_or_default();
    println!("{n:<2} {:<12} {}", r.id, identity_line(r));
    for line in rules::describe(&r.conditions) {
        println!("   {:<12} {line}", "");
    }
    let mut file = style::tilde(&r.file);
    if !r.managed {
        file.push_str("  (not created by gitid)");
    }
    println!("   {:<12} file {file}", "");
}

pub fn list(git: &SystemGit) -> Result<u8> {
    let all = rules::list_rules(git)?;
    if all.is_empty() {
        println!("No identity rules yet. Add one with:");
        println!("  gitid rule add work --email you@company.example --dir ~/work");
        return Ok(EXIT_OK);
    }
    for (i, r) in all.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_rule(Some(i + 1), r);
    }
    Ok(EXIT_OK)
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    git: &SystemGit,
    version: GitVersion,
    id: &str,
    name: Option<&str>,
    email: Option<&str>,
    dirs: &[PathBuf],
    hosts: &[String],
    ssh_port: Option<u16>,
    remotes: &[String],
    checks: &[PathBuf],
) -> Result<u8> {
    let existed = rules::list_rules(git)?
        .iter()
        .any(|r| r.id == id && r.managed);

    let mut when: Vec<When> = dirs.iter().map(|d| When::Folder(d.clone())).collect();
    when.extend(hosts.iter().map(|h| When::RemoteHost {
        host: h.clone(),
        ssh_port,
    }));
    when.extend(remotes.iter().map(|p| When::RemotePattern(p.clone())));

    let rule = rules::add_rule(
        git,
        version,
        &NewRule {
            id: id.into(),
            name: name.map(Into::into),
            email: email.map(Into::into),
            when,
        },
    )?;

    println!("{} rule:", if existed { "Updated" } else { "Added" });
    print_rule(None, &rule);

    // Verify against real repos: the given --check folders, plus repos found
    // directly inside each --dir (folder rules can be checked without asking).
    let mut targets: Vec<PathBuf> = checks.iter().map(|p| real(p)).collect();
    for d in dirs {
        targets.extend(repos_in(&real(d)));
    }
    if targets.is_empty() {
        if !hosts.is_empty() || !remotes.is_empty() {
            println!(
                "\nCheck it on a repo with: gitid rule check {} PATH",
                rule.id
            );
        }
        return Ok(EXIT_OK);
    }

    println!();
    let mut misses = 0;
    for t in &targets {
        let ok = rules::applies_in(git, &rule, t)?;
        if !ok {
            misses += 1;
        }
        println!(
            "  {} {}",
            if ok {
                "applies       "
            } else {
                "DOES NOT apply"
            },
            style::tilde(t)
        );
    }
    if misses > 0 {
        println!("\nThe rule doesn't apply everywhere you expected; `gitid whoami PATH` shows what's used there.");
        return Ok(EXIT_PROBLEMS);
    }
    Ok(EXIT_OK)
}

/// The folder itself (if it is a repo) and repos one level below it.
fn repos_in(dir: &Path) -> Vec<PathBuf> {
    let is_repo = |p: &Path| p.join(".git").exists();
    let mut out = Vec::new();
    if is_repo(dir) {
        out.push(dir.to_path_buf());
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && is_repo(p))
            .collect();
        found.sort();
        // A sample is enough to prove the rule matches.
        out.extend(found.into_iter().take(10));
    }
    out
}

fn lookup(git: &SystemGit, id: &str) -> Result<Rule> {
    let all = rules::list_rules(git)?;
    rules::find(&all, id).cloned().ok_or_else(|| {
        let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
        Error::InvalidInput(if ids.is_empty() {
            format!("no rule {id:?}: there are no rules yet")
        } else {
            format!("no rule {id:?} (rules: {})", ids.join(", "))
        })
    })
}

pub fn remove(git: &SystemGit, id: &str, keep_file: bool) -> Result<u8> {
    let rule = lookup(git, id)?;
    rules::remove_rule(git, &rule, !keep_file)?;
    println!("Removed rule {} ({}).", rule.id, identity_line(&rule));
    if rule.managed && !keep_file {
        println!("Deleted {}", style::tilde(&rule.file));
    } else if rule.file_exists {
        println!("Kept {}", style::tilde(&rule.file));
    }
    Ok(EXIT_OK)
}

pub fn check(git: &SystemGit, id: &str, path: &Path) -> Result<u8> {
    let rule = lookup(git, id)?;
    let folder = real(path);
    if !folder.is_dir() {
        return Err(Error::NotADirectory(folder));
    }
    if rules::applies_in(git, &rule, &folder)? {
        println!("Rule {} applies in {}", rule.id, style::tilde(&folder));
        return Ok(EXIT_OK);
    }
    println!(
        "Rule {} does NOT apply in {}",
        rule.id,
        style::tilde(&folder)
    );
    for c in &rule.conditions {
        if let Condition::Folder {
            pattern,
            case_insensitive: false,
        } = c
        {
            if cfg!(any(windows, target_os = "macos")) {
                println!("  note: {pattern} is matched case-sensitively; folder names must match exactly");
            }
        }
    }
    if rule
        .conditions
        .iter()
        .any(|c| matches!(c, Condition::RemoteUrl { .. }))
    {
        println!(
            "  this folder's remotes: see `git -C {} remote -v`",
            style::tilde(&folder)
        );
    }
    Ok(EXIT_PROBLEMS)
}

//! `gitid rule list | add | use | unuse | check | rm`.

use std::path::{Path, PathBuf};

use gitid_core::rules::{self, Condition, NewRule, Rule, RuleFolder, Unused, Used, When};
use gitid_core::{effective_identity, paths, Error, Git, GitVersion, Invalid, Result, SystemGit};

use crate::args::AddArgs;
use crate::style;
use crate::{EXIT_ERROR, EXIT_OK, EXIT_PROBLEMS};

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

/// A rule plus its human-readable description, for `--json`.
#[derive(serde::Serialize)]
struct RuleJson<'a> {
    #[serde(flatten)]
    rule: &'a Rule,
    description: Vec<String>,
}

pub fn list(git: &SystemGit, json: bool) -> Result<u8> {
    let all = rules::list_rules(git)?;
    if json {
        let out: Vec<RuleJson> = all
            .iter()
            .map(|rule| RuleJson {
                rule,
                description: rules::describe(&rule.conditions),
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&out).expect("rules serialize")
        );
        return Ok(EXIT_OK);
    }
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

pub fn add(git: &SystemGit, version: GitVersion, a: &AddArgs) -> Result<u8> {
    let (id, dirs, hosts, remotes, checks) = (a.id.as_str(), &a.dir, &a.host, &a.remote, &a.check);
    let existed = rules::list_rules(git)?
        .iter()
        .any(|r| r.id == id && r.managed);

    let mut when: Vec<When> = dirs.iter().map(|d| When::Folder(d.clone())).collect();
    when.extend(hosts.iter().map(|h| When::RemoteHost {
        host: h.clone(),
        ssh_port: a.ssh_port,
    }));
    when.extend(remotes.iter().map(|p| When::RemotePattern(p.clone())));

    type Save = fn(&dyn Git, GitVersion, &NewRule) -> Result<Rule>;
    let save: Save = if a.replace {
        rules::replace_rule
    } else {
        rules::add_rule
    };
    let rule = save(
        git,
        version,
        &NewRule {
            id: id.into(),
            name: a.name.clone(),
            email: a.email.clone(),
            when,
        },
    )?;

    println!("{} rule:", if existed { "Updated" } else { "Added" });
    print_rule(None, &rule);

    // A --dir inside a repo (`--dir .` from repo/src) names a folder git never
    // matches, because it compares rules against where the .git folder is.
    let mut misses = 0;
    for d in dirs {
        let target = rules::rule_folder(git, d)?;
        if target.in_repo && target.folder != real(d) {
            misses += 1;
            println!(
                "\nnote: {} is inside a repository, so this folder rule never applies there.",
                style::tilde(&real(d))
            );
            println!(
                "      To use the rule in that repo, run there: gitid rule use {}",
                rule.id
            );
        }
    }

    // Verify against real repos: the given --check folders, plus repos found
    // directly inside each --dir (folder rules can be checked without asking).
    let mut targets: Vec<PathBuf> = checks.iter().map(|p| real(p)).collect();
    for d in dirs {
        // A sample is enough to prove the rule matches.
        targets.extend(rules::sample_repos(&real(d), 10));
    }
    if targets.is_empty() {
        if misses > 0 {
            return Ok(EXIT_PROBLEMS);
        }
        if !hosts.is_empty() || !remotes.is_empty() {
            println!(
                "\nCheck it on a repo with: gitid rule check {} PATH",
                rule.id
            );
        }
        return Ok(EXIT_OK);
    }

    println!();
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

fn lookup(git: &SystemGit, id: &str) -> Result<Rule> {
    let all = rules::list_rules(git)?;
    rules::find(&all, id).cloned().ok_or_else(|| {
        Error::Invalid(Invalid::NoSuchRule {
            id: id.to_string(),
            known: all.iter().map(|r| r.id.clone()).collect(),
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

/// "~/code/app", plus why the rule names another folder when it does.
fn place(target: &RuleFolder) -> String {
    let folder = style::tilde(&target.folder);
    match &target.work_tree {
        Some(top) if *top != target.folder => format!(
            "{folder} (the folder that holds the git folder of {}; git matches rules by where that is)",
            style::tilde(top)
        ),
        _ => folder,
    }
}

/// After a change in a repo: who commits there now. `Some(rule)`: the rule
/// is meant to apply, so say if something else wins. Returns the exit code.
fn commits_now(git: &SystemGit, path: &Path, expect: Option<&Rule>) -> Result<u8> {
    let id = effective_identity(git, &real(path))?;
    let now = id.author.display();
    println!(
        "Commits here: {}",
        now.as_deref()
            .unwrap_or("no usable identity; `gitid whoami` shows why")
    );
    let Some(rule) = expect else {
        return Ok(EXIT_OK);
    };
    let differs =
        |want: &Option<String>, got: Option<&str>| want.as_deref().is_some_and(|w| Some(w) != got);
    let name = id.author.name.as_ref().map(|f| f.value.as_str());
    let email = id.author.email.as_ref().map(|f| f.value.as_str());
    if differs(&rule.name, name) || differs(&rule.email, email) {
        println!(
            "Rule {} isn't what applies here: something later in your config, the repo's own config or the environment wins. `gitid whoami` shows which.",
            rule.id
        );
        return Ok(EXIT_PROBLEMS);
    }
    Ok(EXIT_OK)
}

pub fn use_rule(git: &SystemGit, id: &str, path: &Path) -> Result<u8> {
    let rule = lookup(git, id)?;
    let (target, used) = rules::use_rule(git, &rule, path)?;
    match used {
        Used::Added => println!("Rule {} now applies in {}", rule.id, place(&target)),
        Used::AlreadyNamed => println!(
            "Rule {} already applies in {}; nothing changed.",
            rule.id,
            place(&target)
        ),
        Used::AlreadyApplies => println!(
            "Rule {} already applies in {} through another of its folders or servers; nothing changed.",
            rule.id,
            style::tilde(target.work_tree.as_ref().unwrap_or(&target.folder))
        ),
    }
    if target.in_repo {
        return commits_now(git, path, Some(&rule));
    }
    // A folder of repos: check the repos directly inside, as `rule add` does.
    let repos = rules::sample_repos(&target.folder, 10);
    if repos.is_empty() {
        return Ok(EXIT_OK);
    }
    println!();
    let mut misses = 0;
    for r in &repos {
        let ok = rules::applies_in(git, &rule, r)?;
        misses += usize::from(!ok);
        println!(
            "  {} {}",
            if ok {
                "applies       "
            } else {
                "DOES NOT apply"
            },
            style::tilde(r)
        );
    }
    Ok(if misses > 0 { EXIT_PROBLEMS } else { EXIT_OK })
}

pub fn unuse(git: &SystemGit, id: &str, path: &Path, host: Option<&str>) -> Result<u8> {
    let rule = lookup(git, id)?;
    let (what, unused, target) = match host {
        Some(h) => (
            format!("to repos on {h}"),
            rules::unuse_host(git, &rule, h)?,
            None,
        ),
        None => {
            let (target, unused) = rules::unuse_folder(git, &rule, path)?;
            (format!("in {}", place(&target)), unused, Some(target))
        }
    };
    match unused {
        Unused::Removed(_) => {
            println!("Rule {} no longer applies {what}", rule.id);
            match target {
                Some(t) if t.in_repo => {
                    if rules::applies_in(git, &lookup(git, &rule.id)?, path)? {
                        println!(
                            "It still applies here through another of its folders or servers."
                        );
                    }
                    commits_now(git, path, None)
                }
                _ => Ok(EXIT_OK),
            }
        }
        Unused::OnlyCondition => {
            eprintln!(
                "gitid: that's the only place rule {} applies ({what}), so it would apply nowhere afterwards. Nothing changed.",
                rule.id
            );
            eprintln!(
                "If you meant to delete the rule, use: gitid rule rm {}",
                rule.id
            );
            Ok(EXIT_ERROR)
        }
        Unused::AppliesThrough(through) => {
            eprintln!("gitid: rule {} applies {what} through:", rule.id);
            for d in &through {
                eprintln!("  {d}");
            }
            eprintln!("git can't leave one folder out of a rule. Nothing changed. To commit as someone else here:");
            eprintln!("  gitid rule use OTHER-RULE");
            eprintln!("  gitid set user.email you@example.com --repo .");
            Ok(EXIT_ERROR)
        }
        Unused::NotHere => {
            println!("Rule {} doesn't apply {what}; nothing changed.", rule.id);
            Ok(EXIT_OK)
        }
    }
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

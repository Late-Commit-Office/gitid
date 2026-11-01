use std::path::Path;

use gitid_core::rules::{self, Rule};
use gitid_core::{
    effective_identity, paths, GitVersion, Part, Result, RoleIdentity, Severity, Source, SystemGit,
    Warning,
};

use crate::style;
use crate::{EXIT_OK, EXIT_PROBLEMS};

pub fn run(git: &SystemGit, version: GitVersion, path: &Path) -> Result<u8> {
    let folder = gitid_core::paths::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let id = effective_identity(git, &folder)?;
    // Only used to label values that come from a rule; never fatal.
    let rules = rules::list_rules(git).unwrap_or_default();

    let repo = match &id.repo {
        Some(r) => style::tilde(r.work_tree.as_deref().unwrap_or(&r.git_dir)),
        None => "(not a git repository)".into(),
    };
    println!("Folder      {}", style::tilde(&id.folder));
    println!("Repository  {repo}");
    println!("git         {version}  {}", style::tilde(git.path()));
    println!();

    print_role("Author", &id.author, &rules);
    if id.author_matches_committer() && same_sources(&id.author, &id.committer) {
        println!("Committer   same as author");
    } else {
        print_role("Committer", &id.committer, &rules);
    }

    if !id.env.is_empty() {
        println!();
        println!("Environment (this terminal only)");
        for v in &id.env {
            let value = if v.sensitive {
                "(set, hidden)"
            } else {
                v.value.as_str()
            };
            println!("  {:<22} {value}", v.name);
        }
    }

    let warnings = id.warnings();
    for (severity, title, mark) in [
        (Severity::Problem, "Problems", "!"),
        (Severity::Note, "Notes", "-"),
    ] {
        let ws: Vec<_> = warnings
            .iter()
            .filter(|w| w.severity() == severity)
            .collect();
        if ws.is_empty() {
            continue;
        }
        println!();
        println!("{title}");
        for w in ws {
            println!("  {mark} {w}");
            if let Some(fix) = hint(w) {
                println!("      fix: {fix}");
            }
        }
    }

    Ok(if id.has_problems() {
        EXIT_PROBLEMS
    } else {
        EXIT_OK
    })
}

fn print_role(title: &str, r: &RoleIdentity, rules: &[Rule]) {
    match r.display() {
        Some(line) => println!("{title:<11} {line}"),
        None => println!("{title:<11} (none: git will refuse to commit)"),
    }
    for part in [Part::Name, Part::Email] {
        let expected = format!("user.{}", part.label());
        let detail = match r.field(part) {
            // When git refuses, the header has no values, so show them per line.
            Some(f) if r.error.is_some() => {
                format!("{:<20} {}", f.value, style::source(&f.source, &expected))
            }
            Some(f) => style::source(&f.source, &expected),
            None => format!("{:<9} not set", "missing"),
        };
        let detail = match r.field(part).and_then(|f| rule_for(&f.source, rules)) {
            Some(rule) => format!("{detail}  (rule: {})", rule.id),
            None => detail,
        };
        println!("  {:<9} {detail}", part.label());
    }
}

/// Collapse the committer block only when both values come from the very same
/// setting. `committer.name` or `GIT_COMMITTER_*` set to the same text still
/// get their own block, so the extra setting is visible.
fn same_sources(a: &RoleIdentity, b: &RoleIdentity) -> bool {
    [Part::Name, Part::Email]
        .iter()
        .all(|p| a.field(*p).map(|f| &f.source) == b.field(*p).map(|f| &f.source))
}

/// The rule whose identity file supplied this value, if any.
fn rule_for<'a>(source: &Source, rules: &'a [Rule]) -> Option<&'a Rule> {
    let Source::Config(entry) = source else {
        return None;
    };
    let file = entry.origin.file()?;
    let real = |p: &Path| paths::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let file = real(file);
    rules.iter().find(|r| real(&r.file) == file)
}

/// A command the user can paste to fix a problem.
fn hint(w: &Warning) -> Option<&'static str> {
    match w {
        Warning::Guessed {
            part: Part::Name, ..
        } => Some(r#"gitid set user.name "Your Name""#),
        Warning::Guessed {
            part: Part::Email, ..
        } => Some("gitid set user.email you@example.com"),
        Warning::Missing { .. } => {
            Some(r#"gitid set user.name "Your Name" and gitid set user.email you@example.com"#)
        }
        Warning::GuessingAllowed => Some("gitid set user.useConfigOnly true"),
        _ => None,
    }
}

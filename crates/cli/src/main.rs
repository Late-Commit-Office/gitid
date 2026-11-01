//! `gitid`: see which git identity applies to a folder, and why, and fix it.

mod args;
mod edit;
mod rule;
mod scan;
mod settings;
mod show;
mod style;
mod whoami;

use std::process::ExitCode;

use args::{Cli, Cmd, RuleCmd};
use clap::Parser;
use gitid_core::{GitVersion, SystemGit};

/// Exit codes, so scripts can check `gitid whoami` before committing.
/// (clap itself exits with 2 for bad arguments.)
pub const EXIT_OK: u8 = 0;
pub const EXIT_ERROR: u8 = 1;
pub const EXIT_PROBLEMS: u8 = 3;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let git = match SystemGit::locate(cli.git.as_deref()) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gitid: {e}");
            if cli.git.is_none() {
                eprintln!("Install git, or point gitid at it with --git PATH.");
            }
            return ExitCode::from(EXIT_ERROR);
        }
    };
    let version = match GitVersion::detect(&git) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("gitid: {e}");
            return ExitCode::from(EXIT_ERROR);
        }
    };

    let result = match &cli.command {
        Cmd::Whoami { path, json } => whoami::run(&git, version, path, *json),
        Cmd::Show { path, key } => show::run(&git, path, key.as_deref()),
        Cmd::Set { key, value, scope } => edit::set(&git, key, value, &scope.scope()),
        Cmd::Unset { key, all, scope } => edit::unset(&git, key, &scope.scope(), *all),
        Cmd::Settings { json } => settings::list(&git, *json),
        Cmd::Scan {
            path,
            all,
            depth,
            hidden,
            json,
        } => scan::run(&git, path, *all, *depth, *hidden, *json),
        Cmd::Rule { command } => match command {
            None => rule::list(&git, false),
            Some(RuleCmd::List { json }) => rule::list(&git, *json),
            Some(RuleCmd::Add(a)) => rule::add(&git, version, a),
            Some(RuleCmd::Check { id, path }) => rule::check(&git, id, path),
            Some(RuleCmd::Remove { id, keep_file }) => rule::remove(&git, id, *keep_file),
        },
    };

    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("gitid: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

//! `gitid`: see which git identity applies to a folder, and why.

mod args;
mod show;
mod style;
mod whoami;

use std::process::ExitCode;

use args::{Command, ParseError};
use gitid_core::{GitVersion, SystemGit};

/// Exit codes, so scripts can check `gitid whoami` before committing.
pub const EXIT_OK: u8 = 0;
pub const EXIT_ERROR: u8 = 1;
pub const EXIT_USAGE: u8 = 2;
pub const EXIT_PROBLEMS: u8 = 3;

const HELP: &str = "\
gitid - see which git identity applies to a folder, and why

USAGE:
    gitid whoami [PATH]          Who commits in PATH will be from, and where that comes from
    gitid show [PATH]            All config visible from PATH, grouped by file
    gitid show [PATH] --key KEY  Every value of one key, and which one wins

OPTIONS:
    --git PATH     Use this git executable instead of searching for one
    -h, --help     Show this help
    -V, --version  Show version

PATH defaults to the current folder.

EXIT CODES:
    0  identity is configured        3  identity is missing or guessed
    1  error (git not found, ...)    2  bad arguments
";

fn main() -> ExitCode {
    let parsed = match args::parse(std::env::args().skip(1)) {
        Ok(p) => p,
        Err(ParseError(msg)) => {
            eprintln!("gitid: {msg}\n\nRun `gitid --help` for usage.");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    match parsed.command {
        Command::Help => {
            print!("{HELP}");
            return ExitCode::from(EXIT_OK);
        }
        Command::Version => {
            println!("gitid {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::from(EXIT_OK);
        }
        _ => {}
    }

    let git = match SystemGit::locate(parsed.git.as_deref()) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gitid: {e}");
            if parsed.git.is_none() {
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

    let result = match parsed.command {
        Command::Whoami { path } => whoami::run(&git, version, &path),
        Command::Show { path, key } => show::run(&git, &path, key.as_deref()),
        Command::Help | Command::Version => unreachable!(),
    };

    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("gitid: {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

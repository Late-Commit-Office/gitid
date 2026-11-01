//! `gitid`: see which git identity applies to a folder, and why, and fix it.

mod args;
mod edit;
mod rule;
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

LOOK:
    gitid whoami [PATH]              Who commits in PATH will be from, and where that comes from
    gitid show [PATH]                All config visible from PATH, grouped by file
    gitid show [PATH] --key KEY      Every value of one key, and which one wins

CHANGE:
    gitid set KEY VALUE [WHERE]      Set a value (default: your global config)
    gitid unset KEY [--all] [WHERE]  Remove a value (--all: every value of the key)
        WHERE is one of --global, --system, --local (this repo), --repo PATH, --file FILE

RULES (a different identity per folder or per server):
    gitid rule list
    gitid rule add ID [--name NAME] [--email EMAIL] WHEN... [--check PATH]...
        WHEN is any mix of:
          --dir FOLDER        repos stored under FOLDER
          --host SERVER       repos with a remote on SERVER (https, http or ssh; git 2.36+)
          --ssh-port PORT     also match ssh on a custom port (with --host)
          --remote PATTERN    a raw remote-URL pattern, e.g. 'https://example.com/team/**'
        Running add again with the same ID updates that rule.
    gitid rule check ID [PATH]       Does the rule apply in PATH?
    gitid rule remove ID [--keep-file]

EXAMPLES:
    gitid rule add work  --email ann@company.example --dir ~/work
    gitid rule add gitea --name annlee --email ann@gitea.example --host gitea.example.com --ssh-port 2222
    gitid set user.useConfigOnly true

OPTIONS:
    --git PATH     Use this git executable instead of searching for one
    -h, --help     Show this help
    -V, --version  Show version

PATH defaults to the current folder. Values starting with '-' go after `--`:
    gitid set -- core.pager -R

EXIT CODES:
    0  ok                             3  identity missing/guessed, or a rule doesn't apply
    1  error (git not found, ...)     2  bad arguments
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

    let result = match &parsed.command {
        Command::Whoami { path } => whoami::run(&git, version, path),
        Command::Show { path, key } => show::run(&git, path, key.as_deref()),
        Command::Set { key, value, scope } => edit::set(&git, key, value, scope),
        Command::Unset { key, scope, all } => edit::unset(&git, key, scope, *all),
        Command::RuleList => rule::list(&git),
        Command::RuleAdd {
            id,
            name,
            email,
            dirs,
            hosts,
            ssh_port,
            remotes,
            checks,
        } => rule::add(
            &git,
            version,
            id,
            name.as_deref(),
            email.as_deref(),
            dirs,
            hosts,
            *ssh_port,
            remotes,
            checks,
        ),
        Command::RuleRemove { id, keep_file } => rule::remove(&git, id, *keep_file),
        Command::RuleCheck { id, path } => rule::check(&git, id, path),
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

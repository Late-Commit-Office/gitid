//! Command-line definition (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

const AFTER_HELP: &str = "\
EXAMPLES:
    gitid whoami ~/code/app
    gitid set user.useConfigOnly true
    gitid rule add work  --email ann@company.example --dir ~/work
    gitid rule add gitea --name annlee --email ann@gitea.example --host gitea.example.com --ssh-port 2222

EXIT CODES:
    0  ok                             3  identity missing/guessed, or a rule doesn't apply
    1  error (git not found, ...)     2  bad arguments";

#[derive(Parser, Debug)]
#[command(
    name = "gitid",
    version,
    about = "See which git identity applies to a folder, and why",
    after_help = AFTER_HELP,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Use this git executable instead of searching for one
    #[arg(long, global = true, value_name = "PATH")]
    pub git: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Cmd {
    /// Who commits in PATH will be from, and where each value comes from
    Whoami {
        /// Folder to check
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// All config visible from PATH, grouped by file
    Show {
        /// Folder to check
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Show every value of one key, and which one wins
        #[arg(long, value_name = "KEY")]
        key: Option<String>,
    },
    /// Set a config value (default: your global config)
    Set {
        /// e.g. user.name
        key: String,
        /// The value; may start with '-'
        #[arg(allow_hyphen_values = true)]
        value: String,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Remove a config value (default: from your global config)
    Unset {
        key: String,
        /// Remove every value of the key, not just a single one
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// The global settings gitid manages, with their current values
    Settings {
        /// Print JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// A different identity per folder or per server (default: list rules)
    #[command(alias = "rules")]
    Rule {
        #[command(subcommand)]
        command: Option<RuleCmd>,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum RuleCmd {
    /// List identity rules
    #[command(alias = "ls")]
    List {
        /// Print JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Add a rule, or update the rule with this ID
    Add(AddArgs),
    /// Does a rule apply in PATH?
    Check {
        /// Rule ID or number from `gitid rule list`
        id: String,
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Remove a rule (and the identity file gitid created for it)
    #[command(alias = "rm")]
    Remove {
        /// Rule ID or number from `gitid rule list`
        id: String,
        /// Keep the identity file
        #[arg(long)]
        keep_file: bool,
    },
}

#[derive(Args, Debug, PartialEq, Eq)]
pub struct AddArgs {
    /// Short name for the rule, e.g. work or gitea
    pub id: String,
    /// user.name to use where the rule applies
    #[arg(long)]
    pub name: Option<String>,
    /// user.email to use where the rule applies
    #[arg(long)]
    pub email: Option<String>,
    /// Repos stored under this folder (repeatable)
    #[arg(long, value_name = "FOLDER", required_unless_present_any = ["host", "remote"])]
    pub dir: Vec<PathBuf>,
    /// Repos with a remote on this server: https, http or ssh (git 2.36+, repeatable)
    #[arg(long, value_name = "SERVER")]
    pub host: Vec<String>,
    /// Also match ssh on this port (with --host)
    #[arg(long, value_name = "PORT", requires = "host")]
    pub ssh_port: Option<u16>,
    /// A raw remote-URL pattern, e.g. 'https://example.com/team/**' (repeatable)
    #[arg(long, value_name = "PATTERN")]
    pub remote: Vec<String>,
    /// Also check the rule against this repo afterwards (repeatable)
    #[arg(long, value_name = "PATH")]
    pub check: Vec<PathBuf>,
    /// Replace the rule's folders, servers, name and email with exactly these,
    /// instead of adding to them
    #[arg(long)]
    pub replace: bool,
}

/// Which config file `set`/`unset` write to. At most one may be given.
#[derive(Args, Debug, Default, PartialEq, Eq)]
#[group(id = "where", multiple = false)]
pub struct ScopeArgs {
    /// Your global config (default)
    #[arg(long)]
    global: bool,
    /// The system config (usually needs admin rights)
    #[arg(long)]
    system: bool,
    /// The repo containing the current folder
    #[arg(long)]
    local: bool,
    /// The repo containing PATH
    #[arg(long, value_name = "PATH")]
    repo: Option<PathBuf>,
    /// A specific config file
    #[arg(long, value_name = "FILE")]
    file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Global,
    System,
    Repo(PathBuf),
    File(PathBuf),
}

impl ScopeArgs {
    pub fn scope(&self) -> Scope {
        if self.system {
            Scope::System
        } else if self.local {
            Scope::Repo(".".into())
        } else if let Some(p) = &self.repo {
            Scope::Repo(p.clone())
        } else if let Some(f) = &self.file {
            Scope::File(f.clone())
        } else {
            Scope::Global
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("gitid").chain(args.iter().copied()))
    }
    fn cmd(args: &[&str]) -> Cmd {
        parse(args).unwrap().command
    }

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn basic_commands() {
        assert_eq!(
            cmd(&["whoami"]),
            Cmd::Whoami {
                path: ".".into(),
                json: false
            }
        );
        let r = parse(&["show", "~/x", "--key", "user.name", "--git=/opt/git"]).unwrap();
        assert_eq!(
            r.command,
            Cmd::Show {
                path: "~/x".into(),
                key: Some("user.name".into())
            }
        );
        assert_eq!(r.git, Some("/opt/git".into()));
        assert_eq!(
            cmd(&["whoami", "--json", "x"]),
            Cmd::Whoami {
                path: "x".into(),
                json: true
            }
        );
    }

    #[test]
    fn set_and_unset_scopes() {
        let Cmd::Set { key, value, scope } = cmd(&["set", "user.name", "Ann Lee"]) else {
            panic!()
        };
        assert_eq!(
            (key.as_str(), value.as_str(), scope.scope()),
            ("user.name", "Ann Lee", Scope::Global)
        );

        let Cmd::Set { scope, .. } = cmd(&["set", "user.email", "a@b", "--repo", "~/x"]) else {
            panic!()
        };
        assert_eq!(scope.scope(), Scope::Repo("~/x".into()));

        // A value starting with a dash works with or without `--`.
        for args in [
            &["set", "core.pager", "-R"][..],
            &["set", "--", "core.pager", "-R"],
        ] {
            let Cmd::Set { value, .. } = cmd(args) else {
                panic!()
            };
            assert_eq!(value, "-R");
        }

        let Cmd::Unset { all, scope, .. } = cmd(&["unset", "x.y", "--local", "--all"]) else {
            panic!()
        };
        assert!(all);
        assert_eq!(scope.scope(), Scope::Repo(".".into()));

        assert!(parse(&["set", "x.y", "v", "--global", "--system"]).is_err());
        assert!(parse(&["set", "x.y"]).is_err());
    }

    #[test]
    fn settings_command() {
        assert_eq!(cmd(&["settings"]), Cmd::Settings { json: false });
        assert_eq!(cmd(&["settings", "--json"]), Cmd::Settings { json: true });
    }

    #[test]
    fn rule_commands() {
        assert_eq!(cmd(&["rule"]), Cmd::Rule { command: None });
        assert_eq!(
            cmd(&["rules", "ls"]),
            Cmd::Rule {
                command: Some(RuleCmd::List { json: false })
            }
        );

        let Cmd::Rule {
            command: Some(RuleCmd::Add(a)),
        } = cmd(&[
            "rule",
            "add",
            "gitea",
            "--email",
            "a@g",
            "--host",
            "g.example",
            "--host=h.example",
            "--ssh-port",
            "2222",
            "--check",
            "~/code/app",
        ])
        else {
            panic!()
        };
        assert_eq!(
            a.hosts_and_port(),
            (
                vec!["g.example".to_string(), "h.example".to_string()],
                Some(2222)
            )
        );
        assert_eq!(a.check, vec![PathBuf::from("~/code/app")]);

        assert_eq!(
            cmd(&["rule", "rm", "work"]),
            Cmd::Rule {
                command: Some(RuleCmd::Remove {
                    id: "work".into(),
                    keep_file: false
                })
            }
        );
        assert_eq!(
            cmd(&["rule", "check", "work"]),
            Cmd::Rule {
                command: Some(RuleCmd::Check {
                    id: "work".into(),
                    path: ".".into()
                })
            }
        );
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            &["frobnicate"][..],
            &["whoami", "a", "b"],
            &["whoami", "--nope"],
            &["show", "--key"],
            &["whoami", "--key", "x"],
            &["rule", "add", "w", "--email", "a@b"], // no --dir/--host/--remote
            &["rule", "add", "w", "--dir", "x", "--ssh-port", "22"], // --ssh-port needs --host
            &["rule", "add", "w", "--host", "h", "--ssh-port", "x"],
            &["rule", "add"],
            &["set", "a.b", "c", "--all"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    impl AddArgs {
        fn hosts_and_port(&self) -> (Vec<String>, Option<u16>) {
            (self.host.clone(), self.ssh_port)
        }
    }
}

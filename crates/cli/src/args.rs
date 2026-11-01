//! A small argument parser: positionals plus `--flag value` / `--flag=value`
//! / boolean flags, then a typed [`Command`]. Replace with clap's derive API
//! once dependencies are available.

use std::path::PathBuf;

/// Flags that take a value. Repeatable ones keep every value.
const VALUE_FLAGS: &[&str] = &[
    "--git",
    "--key",
    "--name",
    "--email",
    "--dir",
    "--host",
    "--ssh-port",
    "--remote",
    "--repo",
    "--file",
    "--check",
];
const BOOL_FLAGS: &[&str] = &[
    "--global",
    "--system",
    "--local",
    "--all",
    "--keep-file",
    "--help",
    "-h",
    "--version",
    "-V",
];

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

fn err<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(msg.into()))
}

#[derive(Debug, Default)]
struct Raw {
    positionals: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Raw {
    fn has(&self, f: &str) -> bool {
        self.flags.iter().any(|(k, _)| k == f)
    }
    fn values(&self, f: &str) -> Vec<String> {
        self.flags
            .iter()
            .filter(|(k, _)| k == f)
            .filter_map(|(_, v)| v.clone())
            .collect()
    }
    fn value(&self, f: &str) -> Option<String> {
        self.values(f).pop()
    }
    /// Reject flags the command doesn't understand (and `--git`, which all accept).
    fn only(&self, cmd: &str, allowed: &[&str]) -> Result<(), ParseError> {
        for (k, _) in &self.flags {
            if k != "--git" && !allowed.contains(&k.as_str()) {
                return err(format!("{k} can't be used with `{cmd}`"));
            }
        }
        Ok(())
    }
}

fn lex(args: impl IntoIterator<Item = String>) -> Result<Raw, ParseError> {
    let mut raw = Raw::default();
    let mut it = args.into_iter();
    let mut only_positionals = false;
    while let Some(arg) = it.next() {
        if only_positionals || arg == "-" || !arg.starts_with('-') {
            raw.positionals.push(arg);
            continue;
        }
        if arg == "--" {
            only_positionals = true;
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        if VALUE_FLAGS.contains(&name.as_str()) {
            let v = match inline {
                Some(v) => v,
                None => it
                    .next()
                    .ok_or_else(|| ParseError(format!("{name} needs a value")))?,
            };
            raw.flags.push((name, Some(v)));
        } else if BOOL_FLAGS.contains(&name.as_str()) && inline.is_none() {
            raw.flags.push((name, None));
        } else {
            return err(format!("unknown option {arg}"));
        }
    }
    Ok(raw)
}

/// Which config file `set`/`unset` write to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Global,
    System,
    Repo(PathBuf),
    File(PathBuf),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Whoami {
        path: PathBuf,
    },
    Show {
        path: PathBuf,
        key: Option<String>,
    },
    Set {
        key: String,
        value: String,
        scope: Scope,
    },
    Unset {
        key: String,
        scope: Scope,
        all: bool,
    },
    RuleList,
    RuleAdd {
        id: String,
        name: Option<String>,
        email: Option<String>,
        dirs: Vec<PathBuf>,
        hosts: Vec<String>,
        ssh_port: Option<u16>,
        remotes: Vec<String>,
        checks: Vec<PathBuf>,
    },
    RuleRemove {
        id: String,
        keep_file: bool,
    },
    RuleCheck {
        id: String,
        path: PathBuf,
    },
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub command: Command,
    pub git: Option<PathBuf>,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, ParseError> {
    let raw = lex(args)?;
    let git = raw.value("--git").map(PathBuf::from);
    let done = |command| {
        Ok(Parsed {
            command,
            git: git.clone(),
        })
    };

    if raw.has("--help") || raw.has("-h") {
        return done(Command::Help);
    }
    if raw.has("--version") || raw.has("-V") {
        return done(Command::Version);
    }

    let pos: Vec<&str> = raw.positionals.iter().map(String::as_str).collect();
    let path_at = |i: usize, max: usize, cmd: &str| -> Result<PathBuf, ParseError> {
        if pos.len() > max {
            return err(format!("too many arguments for `{cmd}`"));
        }
        Ok(PathBuf::from(pos.get(i).copied().unwrap_or(".")))
    };

    let command = match pos.first().copied() {
        None | Some("help") => Command::Help,
        Some("whoami") => {
            raw.only("whoami", &[])?;
            Command::Whoami {
                path: path_at(1, 2, "whoami")?,
            }
        }
        Some("show") => {
            raw.only("show", &["--key"])?;
            Command::Show {
                path: path_at(1, 2, "show")?,
                key: raw.value("--key"),
            }
        }
        Some("set") => {
            raw.only(
                "set",
                &["--global", "--system", "--local", "--repo", "--file"],
            )?;
            let [_, key, value] = pos[..] else {
                return err("usage: gitid set KEY VALUE [--global | --system | --local | --repo PATH | --file FILE]");
            };
            Command::Set {
                key: key.into(),
                value: value.into(),
                scope: scope(&raw)?,
            }
        }
        Some("unset") => {
            raw.only(
                "unset",
                &[
                    "--global", "--system", "--local", "--repo", "--file", "--all",
                ],
            )?;
            let [_, key] = pos[..] else {
                return err("usage: gitid unset KEY [--all] [--global | --system | --local | --repo PATH | --file FILE]");
            };
            Command::Unset {
                key: key.into(),
                scope: scope(&raw)?,
                all: raw.has("--all"),
            }
        }
        Some("rule") | Some("rules") => rule(&raw, &pos[1..])?,
        Some(other) => return err(format!("unknown command {other:?}")),
    };
    done(command)
}

fn scope(raw: &Raw) -> Result<Scope, ParseError> {
    let mut picked = Vec::new();
    if raw.has("--global") {
        picked.push(Scope::Global);
    }
    if raw.has("--system") {
        picked.push(Scope::System);
    }
    if raw.has("--local") {
        picked.push(Scope::Repo(".".into()));
    }
    if let Some(p) = raw.value("--repo") {
        picked.push(Scope::Repo(p.into()));
    }
    if let Some(f) = raw.value("--file") {
        picked.push(Scope::File(f.into()));
    }
    match picked.len() {
        0 => Ok(Scope::Global),
        1 => Ok(picked.remove(0)),
        _ => err("choose only one of --global, --system, --local, --repo, --file"),
    }
}

fn rule(raw: &Raw, pos: &[&str]) -> Result<Command, ParseError> {
    match pos {
        [] | ["list"] => {
            raw.only("rule list", &[])?;
            Ok(Command::RuleList)
        }
        ["add", id] => {
            raw.only(
                "rule add",
                &[
                    "--name",
                    "--email",
                    "--dir",
                    "--host",
                    "--ssh-port",
                    "--remote",
                    "--check",
                ],
            )?;
            let ssh_port = match raw.value("--ssh-port") {
                None => None,
                Some(p) => Some(
                    p.parse::<u16>()
                        .map_err(|_| ParseError(format!("bad --ssh-port {p:?}")))?,
                ),
            };
            let dirs: Vec<PathBuf> = raw.values("--dir").into_iter().map(PathBuf::from).collect();
            let hosts = raw.values("--host");
            let remotes = raw.values("--remote");
            if dirs.is_empty() && hosts.is_empty() && remotes.is_empty() {
                return err(
                    "say where the rule applies: --dir FOLDER, --host SERVER or --remote PATTERN",
                );
            }
            if ssh_port.is_some() && hosts.is_empty() {
                return err("--ssh-port only makes sense with --host");
            }
            Ok(Command::RuleAdd {
                id: id.to_string(),
                name: raw.value("--name"),
                email: raw.value("--email"),
                dirs,
                hosts,
                ssh_port,
                remotes,
                checks: raw
                    .values("--check")
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
            })
        }
        ["remove" | "rm", id] => {
            raw.only("rule remove", &["--keep-file"])?;
            Ok(Command::RuleRemove {
                id: id.to_string(),
                keep_file: raw.has("--keep-file"),
            })
        }
        ["check", id] | ["check", id, _] => {
            raw.only("rule check", &[])?;
            Ok(Command::RuleCheck {
                id: id.to_string(),
                path: PathBuf::from(pos.get(2).copied().unwrap_or(".")),
            })
        }
        _ => err("usage: gitid rule [list | add ID ... | remove ID | check ID [PATH]]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Parsed, ParseError> {
        parse(args.iter().map(|s| s.to_string()))
    }
    fn cmd(args: &[&str]) -> Command {
        p(args).unwrap().command
    }

    #[test]
    fn basic_commands() {
        assert_eq!(cmd(&["whoami"]), Command::Whoami { path: ".".into() });
        let r = p(&["show", "~/x", "--key", "user.name", "--git=/opt/git"]).unwrap();
        assert_eq!(
            r.command,
            Command::Show {
                path: "~/x".into(),
                key: Some("user.name".into())
            }
        );
        assert_eq!(r.git, Some("/opt/git".into()));
        assert_eq!(cmd(&[]), Command::Help);
        assert_eq!(cmd(&["whoami", "--help"]), Command::Help);
        assert_eq!(cmd(&["-V"]), Command::Version);
    }

    #[test]
    fn set_and_unset_scopes() {
        assert_eq!(
            cmd(&["set", "user.name", "Ann Lee"]),
            Command::Set {
                key: "user.name".into(),
                value: "Ann Lee".into(),
                scope: Scope::Global
            }
        );
        assert_eq!(
            cmd(&["set", "user.email", "a@b", "--repo", "~/x"]),
            Command::Set {
                key: "user.email".into(),
                value: "a@b".into(),
                scope: Scope::Repo("~/x".into())
            }
        );
        // A value starting with a dash, after `--`.
        assert_eq!(
            cmd(&["set", "--", "core.pager", "-R"]),
            Command::Set {
                key: "core.pager".into(),
                value: "-R".into(),
                scope: Scope::Global
            }
        );
        assert_eq!(
            cmd(&["unset", "x.y", "--local", "--all"]),
            Command::Unset {
                key: "x.y".into(),
                scope: Scope::Repo(".".into()),
                all: true
            }
        );
        assert!(p(&["set", "x.y", "v", "--global", "--system"]).is_err());
        assert!(p(&["set", "x.y"]).is_err());
    }

    #[test]
    fn rule_commands() {
        assert_eq!(cmd(&["rule"]), Command::RuleList);
        assert_eq!(cmd(&["rule", "list"]), Command::RuleList);
        let c = cmd(&[
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
        ]);
        assert_eq!(
            c,
            Command::RuleAdd {
                id: "gitea".into(),
                name: None,
                email: Some("a@g".into()),
                dirs: vec![],
                hosts: vec!["g.example".into(), "h.example".into()],
                ssh_port: Some(2222),
                remotes: vec![],
                checks: vec!["~/code/app".into()],
            }
        );
        assert_eq!(
            cmd(&["rule", "rm", "work"]),
            Command::RuleRemove {
                id: "work".into(),
                keep_file: false
            }
        );
        assert_eq!(
            cmd(&["rule", "check", "work"]),
            Command::RuleCheck {
                id: "work".into(),
                path: ".".into()
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
            &["rule", "add", "w", "--email", "a@b"],
            &["rule", "add", "w", "--dir", "x", "--ssh-port", "22"],
            &["rule", "add", "w", "--host", "h", "--ssh-port", "x"],
            &["rule", "add"],
            &["set", "a.b", "c", "--all"],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }
}

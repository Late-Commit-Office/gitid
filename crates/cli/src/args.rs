//! A tiny argument parser. Small enough not to need clap yet; swap it for
//! clap's derive API when more subcommands arrive.

use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Whoami { path: PathBuf },
    Show { path: PathBuf, key: Option<String> },
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub command: Command,
    pub git: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, ParseError> {
    let mut sub: Option<String> = None;
    let mut path: Option<PathBuf> = None;
    let mut key: Option<String> = None;
    let mut git: Option<PathBuf> = None;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value_for = |flag: &str| {
            it.next()
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| ParseError(format!("{flag} needs a value")))
        };
        match arg.as_str() {
            "-h" | "--help" | "help" if sub.is_none() || arg != "help" => {
                return Ok(Parsed {
                    command: Command::Help,
                    git,
                })
            }
            "-V" | "--version" => {
                return Ok(Parsed {
                    command: Command::Version,
                    git,
                })
            }
            "--git" => git = Some(PathBuf::from(value_for("--git")?)),
            "--key" => key = Some(value_for("--key")?),
            s if s.starts_with("--git=") => git = Some(PathBuf::from(&s[6..])),
            s if s.starts_with("--key=") => key = Some(s[6..].to_string()),
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(ParseError(format!("unknown option {s}")))
            }
            s if sub.is_none() => sub = Some(s.to_string()),
            s if path.is_none() => path = Some(PathBuf::from(s)),
            s => return Err(ParseError(format!("unexpected argument {s:?}"))),
        }
    }

    let path = path.unwrap_or_else(|| PathBuf::from("."));
    let command = match sub.as_deref() {
        None => Command::Help,
        Some("whoami") => {
            if key.is_some() {
                return Err(ParseError("--key only works with `show`".into()));
            }
            Command::Whoami { path }
        }
        Some("show") => Command::Show { path, key },
        Some(other) => return Err(ParseError(format!("unknown command {other:?}"))),
    };
    Ok(Parsed { command, git })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Parsed, ParseError> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_commands() {
        assert_eq!(
            p(&["whoami"]).unwrap().command,
            Command::Whoami { path: ".".into() }
        );
        let r = p(&["show", "~/x", "--key", "user.name", "--git=/opt/git"]).unwrap();
        assert_eq!(
            r.command,
            Command::Show {
                path: "~/x".into(),
                key: Some("user.name".into())
            }
        );
        assert_eq!(r.git, Some("/opt/git".into()));
        assert_eq!(p(&[]).unwrap().command, Command::Help);
        assert_eq!(p(&["whoami", "--help"]).unwrap().command, Command::Help);
        assert_eq!(p(&["-V"]).unwrap().command, Command::Version);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(p(&["frobnicate"]).is_err());
        assert!(p(&["whoami", "a", "b"]).is_err());
        assert!(p(&["whoami", "--nope"]).is_err());
        assert!(p(&["show", "--key"]).is_err());
        assert!(p(&["whoami", "--key", "x"]).is_err());
    }
}

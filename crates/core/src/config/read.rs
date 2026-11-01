use std::path::Path;

use super::model::{ConfigSnapshot, Entry, Origin, Scope};
use super::write::Target;
use crate::error::{Error, Result};
use crate::git::Git;
use crate::repo::{self, RepoLocation};

/// Read every config entry visible from `dir` (includes and `includeIf`
/// evaluated by git itself), with scope and origin.
///
/// `dir = None` reads what applies outside any repository.
pub fn read_all(git: &dyn Git, dir: Option<&Path>) -> Result<ConfigSnapshot> {
    let repo = match dir {
        Some(d) => repo::discover(git, d)?,
        None => None,
    };
    read_all_in(git, dir, repo.as_ref())
}

/// Same as [`read_all`], when the caller already discovered the repository.
pub fn read_all_in(
    git: &dyn Git,
    dir: Option<&Path>,
    repo: Option<&RepoLocation>,
) -> Result<ConfigSnapshot> {
    let base = repo.map(RepoLocation::git_cwd);

    let out = git.run(
        dir,
        &["config", "--list", "--show-origin", "--show-scope", "-z"],
    )?;
    let (out, with_scope) = if out.success() {
        (out, true)
    } else if out.stderr.contains("show-scope") {
        // git < 2.26: no --show-scope. Scopes will be Unknown.
        let out = git.run_ok(dir, &["config", "--list", "--show-origin", "-z"])?;
        (out, false)
    } else if out.stdout.is_empty() && out.stderr.trim().is_empty() {
        // `git config --list` exits 1 with no output when nothing is configured at all.
        return Ok(ConfigSnapshot::default());
    } else {
        return Err(Error::GitFailed {
            args: vec!["config".into(), "--list".into()],
            code: out.code,
            stderr: out.stderr,
        });
    };

    let entries = parse_z(&out.stdout, with_scope, base)?;
    Ok(ConfigSnapshot { entries })
}

/// Read only what one file sets, without following its includes.
/// A file that doesn't exist yet reads as empty.
pub fn read_target(git: &dyn Git, target: &Target) -> Result<Vec<Entry>> {
    let repo = match target {
        Target::Local(dir) => match repo::discover(git, dir)? {
            Some(r) => Some(r),
            None => return Err(Error::NotARepository(dir.clone())),
        },
        _ => None,
    };
    let (dir, flags) = target.invocation()?;
    let mut args = vec!["config"];
    args.extend(flags);
    args.extend(["--no-includes", "--list", "--show-origin", "-z"]);

    let out = git.run(dir, &args)?;
    if !out.success() {
        let missing = out.stderr.contains("unable to read config file")
            || out.stderr.contains("No such file");
        if missing || (out.stdout.is_empty() && out.stderr.trim().is_empty()) {
            return Ok(Vec::new());
        }
        return Err(Error::GitFailed {
            args: args.iter().map(|s| s.to_string()).collect(),
            code: out.code,
            stderr: out.stderr,
        });
    }

    let scope = match target {
        Target::System => Scope::System,
        Target::Global => Scope::Global,
        Target::Local(_) => Scope::Local,
        Target::File(_) => Scope::Unknown,
    };
    let base = repo.as_ref().map(RepoLocation::git_cwd);
    let mut entries = parse_z(&out.stdout, false, base)?;
    for e in &mut entries {
        e.scope = scope;
    }
    Ok(entries)
}

/// Parse `git config --list --show-origin [--show-scope] -z`.
///
/// Each entry is NUL-separated fields: `[scope\0]origin\0key[\nvalue]\0`.
/// A key with no `\n` had no `=` in the file (a bare boolean).
pub fn parse_z(bytes: &[u8], with_scope: bool, base: Option<&Path>) -> Result<Vec<Entry>> {
    let mut fields: Vec<&[u8]> = bytes.split(|b| *b == 0).collect();
    // Output ends with a NUL, which leaves one empty trailing field.
    if fields.last().is_some_and(|f| f.is_empty()) {
        fields.pop();
    }

    let per_entry = if with_scope { 3 } else { 2 };
    if fields.len() % per_entry != 0 {
        return Err(Error::Parse(format!(
            "config list had {} fields, not a multiple of {per_entry}",
            fields.len()
        )));
    }

    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();

    let entries = fields
        .chunks(per_entry)
        .map(|chunk| {
            let (scope, origin, kv) = if with_scope {
                (Scope::parse(&text(chunk[0])), chunk[1], chunk[2])
            } else {
                (Scope::Unknown, chunk[0], chunk[1])
            };
            let kv = text(kv);
            let (key, value) = match kv.split_once('\n') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (kv, None),
            };
            Entry {
                key,
                value,
                scope,
                origin: Origin::parse(&text(origin), base),
            }
        })
        .collect();

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_scope_origin_and_values() {
        let raw = b"global\0file:/h/.gitconfig\0user.name\nAnn Lee\0\
                    global\0file:/h/.gitconfig\0x.flag\0\
                    global\0file:/h/.gitconfig\0sec.Sub.Name.key\nmulti\nline\0\
                    local\0file:.git/config\0user.email\na@b.c\0\
                    command\0command line:\0credential.interactive\nfalse\0";
        let e = parse_z(raw, true, Some(Path::new("/repo"))).unwrap();
        assert_eq!(e.len(), 5);

        assert_eq!(e[0].key, "user.name");
        assert_eq!(e[0].value.as_deref(), Some("Ann Lee"));
        assert_eq!(e[0].scope, Scope::Global);

        assert_eq!(e[1].key, "x.flag");
        assert_eq!(e[1].value, None);
        assert_eq!(e[1].as_bool(), Some(true));

        assert_eq!(e[2].key, "sec.Sub.Name.key");
        assert_eq!(e[2].value.as_deref(), Some("multi\nline"));

        assert_eq!(
            e[3].origin,
            Origin::File(PathBuf::from("/repo/.git/config"))
        );
        assert_eq!(e[4].scope, Scope::Command);
        assert_eq!(e[4].origin, Origin::CommandLine);
    }

    #[test]
    fn parses_without_scope() {
        let raw = b"file:/h/.gitconfig\0user.name\nAnn\0";
        let e = parse_z(raw, false, None).unwrap();
        assert_eq!(e[0].scope, Scope::Unknown);
        assert_eq!(e[0].value.as_deref(), Some("Ann"));
    }

    #[test]
    fn empty_and_malformed() {
        assert!(parse_z(b"", true, None).unwrap().is_empty());
        assert!(parse_z(b"global\0file:/x\0", true, None).is_err());
    }
}

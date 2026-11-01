//! Identity rules: "repos in this folder / on this server commit as X".
//!
//! A rule is a small identity file (`~/.config/git/identities/<id>.gitconfig`
//! holding `user.name` / `user.email`) plus one or more `includeIf ... .path`
//! entries in the global config that pull it in. git evaluates the conditions;
//! this module only writes them and reads them back.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::{self, Entry, Target};
use crate::error::{Error, Result};
use crate::git::{Git, GitVersion};
use crate::{paths, platform};

/// Where rule identity files live, relative to home.
pub const IDENTITIES_DIR: &str = ".config/git/identities";
const INCLUDE_PREFIX: &str = "includeif.";
const INCLUDE_SUFFIX: &str = ".path";
const HASCONFIG_REMOTE: &str = "hasconfig:remote.*.url:";

/// One `includeIf` condition, as git understands it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Condition {
    /// `gitdir:<pattern>` or `gitdir/i:<pattern>`: repos stored under a folder.
    Folder {
        pattern: String,
        case_insensitive: bool,
    },
    /// `hasconfig:remote.*.url:<glob>`: repos with a matching remote (git 2.36+).
    RemoteUrl { pattern: String },
    /// Anything else (`onbranch:`, ...). Shown, never edited.
    Other { raw: String },
}

impl Condition {
    pub fn parse(raw: &str) -> Condition {
        if let Some(p) = raw.strip_prefix("gitdir/i:") {
            Condition::Folder {
                pattern: p.into(),
                case_insensitive: true,
            }
        } else if let Some(p) = raw.strip_prefix("gitdir:") {
            Condition::Folder {
                pattern: p.into(),
                case_insensitive: false,
            }
        } else if let Some(p) = raw.strip_prefix(HASCONFIG_REMOTE) {
            Condition::RemoteUrl { pattern: p.into() }
        } else {
            Condition::Other { raw: raw.into() }
        }
    }

    /// The text inside `includeIf "<here>"`.
    pub fn raw(&self) -> String {
        match self {
            Condition::Folder {
                pattern,
                case_insensitive: true,
            } => format!("gitdir/i:{pattern}"),
            Condition::Folder { pattern, .. } => format!("gitdir:{pattern}"),
            Condition::RemoteUrl { pattern } => format!("{HASCONFIG_REMOTE}{pattern}"),
            Condition::Other { raw } => raw.clone(),
        }
    }

    fn key(&self) -> String {
        format!("includeIf.{}{INCLUDE_SUFFIX}", self.raw())
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Condition::Folder { pattern, .. } => write!(f, "repos under {pattern}"),
            Condition::RemoteUrl { pattern } => write!(f, "remote URL matches {pattern}"),
            Condition::Other { raw } => write!(f, "{raw}"),
        }
    }
}

/// An identity rule found in the global config.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct Rule {
    /// Short handle: the identity file's name without extension.
    pub id: String,
    /// The include path exactly as written in config (e.g. `~/.config/...`).
    pub include_path: String,
    /// That path resolved to a real file location.
    pub file: PathBuf,
    pub file_exists: bool,
    pub conditions: Vec<Condition>,
    pub name: Option<String>,
    pub email: Option<String>,
    /// Created by gitid (lives in the identities folder). Others are shown
    /// and can be removed, but their files are never deleted.
    pub managed: bool,
}

/// What a new rule should match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    /// Repos stored under this folder (it must exist).
    Folder(PathBuf),
    /// Repos whose remote is on this server, over https, http or ssh,
    /// with or without credentials in the URL. `host` may carry a web port
    /// (`gitea.example:3000`); `ssh_port` covers non-standard SSH ports.
    RemoteHost { host: String, ssh_port: Option<u16> },
    /// A raw `hasconfig:remote.*.url:` glob, for anything the host form misses.
    RemotePattern(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRule {
    /// Becomes the file name: letters, digits, `-`, `_`.
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub when: Vec<When>,
}

// ---------------------------------------------------------------- reading

/// Every `includeIf` in the global config, grouped by the file it includes.
pub fn list_rules(git: &dyn Git) -> Result<Vec<Rule>> {
    let home = platform::home_dir(git);
    let identities = home.as_ref().map(|h| h.join(IDENTITIES_DIR));
    let entries = config::read_target(git, &Target::Global)?;

    let mut rules: Vec<Rule> = Vec::new();
    for e in &entries {
        let Some((cond, value)) = include_entry(e) else {
            continue;
        };
        let file = platform::resolve_config_path(value, e.origin.file(), home.as_deref());
        let file = paths::simplify(file);
        if let Some(rule) = rules.iter_mut().find(|r| r.file == file) {
            if !rule.conditions.contains(&cond) {
                rule.conditions.push(cond);
            }
            continue;
        }
        rules.push(Rule {
            id: String::new(),
            include_path: value.to_string(),
            managed: identities
                .as_ref()
                .is_some_and(|d| file.parent() == Some(d.as_path())),
            file_exists: file.is_file(),
            file,
            conditions: vec![cond],
            name: None,
            email: None,
        });
    }

    for rule in &mut rules {
        if rule.file_exists {
            let target = Target::File(rule.file.clone());
            let file_entries = config::read_target(git, &target)?;
            let get = |k: &str| {
                file_entries
                    .iter()
                    .rev()
                    .find(|e| e.key_matches(k))
                    .and_then(|e| e.value.clone())
            };
            rule.name = get("user.name");
            rule.email = get("user.email");
        }
    }
    assign_ids(&mut rules);
    Ok(rules)
}

fn include_entry(e: &Entry) -> Option<(Condition, &str)> {
    let cond = e
        .key
        .strip_prefix(INCLUDE_PREFIX)?
        .strip_suffix(INCLUDE_SUFFIX)?;
    Some((Condition::parse(cond), e.value.as_deref()?))
}

fn assign_ids(rules: &mut [Rule]) {
    let mut used: Vec<String> = Vec::new();
    for rule in rules.iter_mut() {
        let stem = rule
            .file
            .file_stem()
            .map(|s| s.to_string_lossy().trim_start_matches('.').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "rule".into());
        let mut id = stem.clone();
        let mut n = 2;
        while used.contains(&id) {
            id = format!("{stem}-{n}");
            n += 1;
        }
        used.push(id.clone());
        rule.id = id;
    }
}

/// Does `rule` currently apply in `folder`? Asks git which files it read there,
/// so it reflects every quirk of matching (case, symlinks, credentials in URLs).
pub fn applies_in(git: &dyn Git, rule: &Rule, folder: &Path) -> Result<bool> {
    let snapshot = config::read_all(git, Some(folder))?;
    let target = paths::canonicalize(&rule.file).unwrap_or_else(|_| rule.file.clone());
    Ok(snapshot.entries.iter().any(|e| {
        e.origin
            .file()
            .map(|f| paths::canonicalize(f).unwrap_or_else(|_| f.to_path_buf()))
            .is_some_and(|f| f == target)
    }))
}

// ---------------------------------------------------------------- writing

/// The `includeIf` conditions a [`When`] turns into.
pub fn conditions_for(when: &When, home: Option<&Path>) -> Result<Vec<Condition>> {
    match when {
        When::Folder(dir) => {
            if !dir.is_dir() {
                return Err(Error::NotADirectory(dir.clone()));
            }
            let real = paths::canonicalize(dir).map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
            Ok(vec![Condition::Folder {
                pattern: platform::gitdir_pattern(&real, home),
                case_insensitive: platform::case_insensitive_fs(),
            }])
        }
        When::RemoteHost { host, ssh_port } => {
            let (web, bare) = clean_host(host)?;
            let mut patterns = vec![
                format!("http*://{web}/**"),   // https:// and http://
                format!("http*://*@{web}/**"), // with user or token in the URL
                format!("ssh://*@{bare}/**"),
                format!("*@{bare}:*/**"), // scp-style: git@host:owner/repo.git
            ];
            if let Some(port) = ssh_port {
                patterns.push(format!("ssh://*@{bare}:{port}/**"));
            }
            Ok(patterns
                .into_iter()
                .map(|pattern| Condition::RemoteUrl { pattern })
                .collect())
        }
        When::RemotePattern(p) => {
            let p = p.trim();
            if p.is_empty() {
                return Err(Error::InvalidInput("remote pattern is empty".into()));
            }
            Ok(vec![Condition::RemoteUrl {
                pattern: p.to_string(),
            }])
        }
    }
}

/// Accepts `gitea.example`, `gitea.example:3000`, `https://gitea.example/`,
/// `git@gitea.example`. Returns `(host[:port], host)`.
fn clean_host(input: &str) -> Result<(String, String)> {
    let mut s = input.trim();
    if let Some(i) = s.find("://") {
        s = &s[i + 3..];
    }
    s = s.trim_end_matches('/');
    if let Some(i) = s.rfind('@') {
        s = &s[i + 1..];
    }
    let bad = s.is_empty()
        || s.contains(|c: char| c.is_whitespace() || "/*?[]\\".contains(c))
        || s.starts_with(':');
    if bad {
        return Err(Error::InvalidInput(format!(
            "{input:?} is not a server name (expected something like gitea.example.com or gitea.example.com:3000)"
        )));
    }
    let bare = match s.rsplit_once(':') {
        Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => h,
        Some(_) => return Err(Error::InvalidInput(format!("bad port in {input:?}"))),
        None => s,
    };
    Ok((s.to_string(), bare.to_string()))
}

fn check_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !id.starts_with('-');
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "rule id {id:?} must be letters, digits, '-' or '_' (e.g. work, gitea)"
        )))
    }
}

fn check_identity(name: Option<&str>, email: Option<&str>) -> Result<()> {
    if name.is_none() && email.is_none() {
        return Err(Error::InvalidInput(
            "a rule needs a name, an email, or both".into(),
        ));
    }
    if name.is_some_and(|n| n.trim().is_empty() || n.contains(['<', '>', '\n'])) {
        return Err(Error::InvalidInput(
            "name can't be empty or contain <, > or newlines".into(),
        ));
    }
    if let Some(e) = email {
        let ok =
            e.contains('@') && !e.contains(|c: char| c.is_whitespace() || c == '<' || c == '>');
        if !ok {
            return Err(Error::InvalidInput(format!(
                "{e:?} doesn't look like an email address"
            )));
        }
    }
    Ok(())
}

/// Create a rule, or extend an existing one with the same id: writes (or
/// updates) its identity file, then adds any `includeIf` entries not already
/// present. Returns the rule as git now sees it.
pub fn add_rule(git: &dyn Git, version: GitVersion, new: &NewRule) -> Result<Rule> {
    check_id(&new.id)?;
    check_identity(new.name.as_deref(), new.email.as_deref())?;
    if new.when.is_empty() {
        return Err(Error::InvalidInput(
            "a rule needs at least one folder or server".into(),
        ));
    }
    let remote = new.when.iter().any(|w| !matches!(w, When::Folder(_)));
    if remote && !version.supports_hasconfig() {
        return Err(Error::Unsupported {
            feature: "matching repos by remote server",
            needs: GitVersion::new(2, 36, 0),
            have: version,
        });
    }

    let home = platform::home_dir(git).ok_or_else(|| {
        Error::InvalidInput("can't find your home folder (HOME is not set)".into())
    })?;

    // Work out every condition first, so bad input writes nothing.
    let mut conditions = Vec::new();
    for w in &new.when {
        for c in conditions_for(w, Some(&home))? {
            if !conditions.contains(&c) {
                conditions.push(c);
            }
        }
    }

    // 1. The identity file. Written before the includes, so an include never
    //    points at a missing file.
    let dir = home.join(IDENTITIES_DIR);
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
        path: dir.clone(),
        source,
    })?;
    let file = dir.join(format!("{}.gitconfig", new.id));
    let target = Target::File(file.clone());
    if let Some(name) = &new.name {
        config::set(git, &target, "user.name", name)?;
    }
    if let Some(email) = &new.email {
        config::set(git, &target, "user.email", email)?;
    }

    // 2. The includes, skipping exact duplicates.
    let include_path = format!("~/{IDENTITIES_DIR}/{}.gitconfig", new.id);
    let existing = config::read_target(git, &Target::Global)?;
    for c in &conditions {
        let key = config::normalize_key(&c.key());
        let present = existing
            .iter()
            .any(|e| e.key == key && e.value.as_deref() == Some(include_path.as_str()));
        if !present {
            config::add(git, &Target::Global, &c.key(), &include_path)?;
        }
    }

    let file = paths::simplify(file);
    list_rules(git)?
        .into_iter()
        .find(|r| r.file == file)
        .ok_or_else(|| Error::Parse("the new rule did not show up in the global config".into()))
}

/// Remove a rule's `includeIf` entries from the global config. With
/// `delete_file`, also delete its identity file, but only for rules gitid
/// created; other people's files are never touched.
pub fn remove_rule(git: &dyn Git, rule: &Rule, delete_file: bool) -> Result<()> {
    for c in &rule.conditions {
        config::unset_value(git, &Target::Global, &c.key(), &rule.include_path)?;
    }
    if delete_file && rule.managed && rule.file_exists {
        std::fs::remove_file(&rule.file).map_err(|source| Error::Io {
            path: rule.file.clone(),
            source,
        })?;
    }
    Ok(())
}

/// Human-readable lines for a rule's conditions. The set of patterns that
/// [`When::RemoteHost`] writes collapses back into one "server ..." line;
/// anything else is listed as-is.
pub fn describe(conditions: &[Condition]) -> Vec<String> {
    let remote: Vec<&str> = conditions
        .iter()
        .filter_map(|c| match c {
            Condition::RemoteUrl { pattern } => Some(pattern.as_str()),
            _ => None,
        })
        .collect();
    let mut used: Vec<&str> = Vec::new();
    let mut out = Vec::new();

    for c in conditions {
        match c {
            Condition::RemoteUrl { pattern } => {
                if used.contains(&pattern.as_str()) {
                    continue;
                }
                let web = pattern
                    .strip_prefix("http*://")
                    .and_then(|p| p.strip_suffix("/**"));
                let Some(web) = web.filter(|w| !w.starts_with("*@")) else {
                    continue; // printed below unless claimed by a host group
                };
                let bare = web.split(':').next().unwrap_or(web);
                let set = [
                    format!("http*://{web}/**"),
                    format!("http*://*@{web}/**"),
                    format!("ssh://*@{bare}/**"),
                    format!("*@{bare}:*/**"),
                ];
                if set.iter().all(|p| remote.contains(&p.as_str())) {
                    let mut line = format!("repos with a remote on {web} (https, http or ssh)");
                    let ssh_prefix = format!("ssh://*@{bare}:");
                    for p in &remote {
                        if let Some(port) = p
                            .strip_prefix(ssh_prefix.as_str())
                            .and_then(|r| r.strip_suffix("/**"))
                        {
                            line.push_str(&format!(", ssh port {port}"));
                            used.push(p);
                        }
                    }
                    used.extend(remote.iter().filter(|p| set.iter().any(|s| s == *p)));
                    out.push(line);
                }
            }
            other => out.push(other.to_string()),
        }
    }
    for c in conditions {
        if let Condition::RemoteUrl { pattern } = c {
            if !used.contains(&pattern.as_str()) {
                out.push(c.to_string());
            }
        }
    }
    out
}

/// Find a rule by id (or by 1-based position, as `gitid rule list` numbers them).
pub fn find<'a>(rules: &'a [Rule], handle: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| r.id == handle).or_else(|| {
        handle
            .parse::<usize>()
            .ok()
            .filter(|n| *n >= 1)
            .and_then(|n| rules.get(n - 1))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_conditions() {
        for raw in [
            "gitdir:~/work/",
            "gitdir/i:C:/code/",
            "hasconfig:remote.*.url:https://h/**",
            "onbranch:main",
        ] {
            assert_eq!(Condition::parse(raw).raw(), raw);
        }
        assert!(matches!(
            Condition::parse("gitdir/i:x/"),
            Condition::Folder {
                case_insensitive: true,
                ..
            }
        ));
    }

    #[test]
    fn host_patterns_cover_https_credentials_ssh_and_scp() {
        let c = conditions_for(
            &When::RemoteHost {
                host: "https://git@gitea.example:3000/".into(),
                ssh_port: Some(2222),
            },
            None,
        )
        .unwrap();
        let p: Vec<String> = c.iter().map(Condition::raw).collect();
        assert_eq!(
            p,
            [
                "hasconfig:remote.*.url:http*://gitea.example:3000/**",
                "hasconfig:remote.*.url:http*://*@gitea.example:3000/**",
                "hasconfig:remote.*.url:ssh://*@gitea.example/**",
                "hasconfig:remote.*.url:*@gitea.example:*/**",
                "hasconfig:remote.*.url:ssh://*@gitea.example:2222/**",
            ]
        );
    }

    #[test]
    fn describe_collapses_host_sets() {
        let mut c = conditions_for(
            &When::RemoteHost {
                host: "gitea.example:3000".into(),
                ssh_port: Some(2222),
            },
            None,
        )
        .unwrap();
        c.insert(0, Condition::parse("gitdir:~/work/"));
        c.push(Condition::parse("hasconfig:remote.*.url:https://other/**"));
        assert_eq!(
            describe(&c),
            [
                "repos under ~/work/",
                "repos with a remote on gitea.example:3000 (https, http or ssh), ssh port 2222",
                "remote URL matches https://other/**",
            ]
        );
        // A partial set is shown pattern by pattern.
        let partial = vec![Condition::parse("hasconfig:remote.*.url:http*://h/**")];
        assert_eq!(describe(&partial), ["remote URL matches http*://h/**"]);
    }

    #[test]
    fn rejects_bad_hosts_ids_and_emails() {
        for h in ["", "  ", "a b", "h/x", "*.example", "h:abc", ":22"] {
            assert!(clean_host(h).is_err(), "{h:?}");
        }
        assert!(check_id("work").is_ok() && check_id("my_gitea-2").is_ok());
        for id in ["", "../x", "a b", "-x", "a.b"] {
            assert!(check_id(id).is_err(), "{id:?}");
        }
        assert!(check_identity(None, None).is_err());
        assert!(check_identity(Some("Ann"), Some("not-an-email")).is_err());
        assert!(check_identity(Some("Ann <x>"), None).is_err());
        assert!(check_identity(None, Some("ann@example.com")).is_ok());
    }

    #[test]
    fn find_by_id_or_number() {
        let r = |id: &str| Rule {
            id: id.into(),
            include_path: String::new(),
            file: PathBuf::new(),
            file_exists: true,
            conditions: vec![],
            name: None,
            email: None,
            managed: true,
        };
        let rules = vec![r("work"), r("gitea")];
        assert_eq!(find(&rules, "gitea").unwrap().id, "gitea");
        assert_eq!(find(&rules, "1").unwrap().id, "work");
        assert!(find(&rules, "0").is_none() && find(&rules, "3").is_none());
    }
}

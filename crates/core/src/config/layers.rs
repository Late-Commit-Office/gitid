//! Which value wins: every key visible from a folder, with each value it has
//! across system, global, local (and included files), in git's reading order.

use super::model::{ConfigSnapshot, Entry, Origin};

/// What git does with one value of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub enum Status {
    /// The value git uses (the last one, for a single-valued key).
    Used,
    /// A later value of the same key replaces this one.
    Overridden,
    /// The key takes several values and git uses all of them
    /// (e.g. `remote.origin.fetch`, `include.path`).
    AlsoUsed,
}

/// Points at one value precisely: the n-th value of `key` in `origin`.
/// Lets a UI edit or remove a value it was only shown masked.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EntryRef {
    pub origin: Origin,
    pub key: String,
    pub nth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Layer {
    pub entry: Entry,
    pub status: Status,
    pub at: EntryRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct KeyLayers {
    pub key: String,
    pub multi_valued: bool,
    /// In git's reading order: system, global, local, ... (later wins).
    pub layers: Vec<Layer>,
}

/// Keys git treats as lists: every value applies, none overrides another.
pub fn is_multi_valued(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    let last = k.rsplit('.').next().unwrap_or("");
    k == "include.path"
        || (k.starts_with("includeif.") && last == "path")
        || (k.starts_with("remote.") && matches!(last, "fetch" | "push" | "pushurl"))
        || (k.starts_with("url.") && matches!(last, "insteadof" | "pushinsteadof"))
        || (k.starts_with("credential") && last == "helper")
        || (k.starts_with("branch.") && last == "merge")
}

/// Group a snapshot by key, sorted by key, marking which values git uses.
/// A key is treated as multi-valued if it is known to be one, or if any
/// single file sets it more than once.
pub fn layers(snapshot: &ConfigSnapshot) -> Vec<KeyLayers> {
    let mut keys: Vec<&str> = snapshot.entries.iter().map(|e| e.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();

    keys.into_iter()
        .map(|key| {
            let entries: Vec<&Entry> = snapshot.entries.iter().filter(|e| e.key == key).collect();
            let repeated_in_one_file = entries.iter().enumerate().any(|(i, a)| {
                entries[i + 1..]
                    .iter()
                    .any(|b| b.origin == a.origin && b.scope == a.scope)
            });
            let multi = is_multi_valued(key) || repeated_in_one_file;

            let mut seen: Vec<(&Origin, usize)> = Vec::new();
            let last = entries.len() - 1;
            let layers = entries
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let nth = match seen.iter_mut().find(|(o, _)| *o == &e.origin) {
                        Some((_, n)) => {
                            *n += 1;
                            *n
                        }
                        None => {
                            seen.push((&e.origin, 0));
                            0
                        }
                    };
                    Layer {
                        entry: (*e).clone(),
                        status: if multi {
                            Status::AlsoUsed
                        } else if i == last {
                            Status::Used
                        } else {
                            Status::Overridden
                        },
                        at: EntryRef {
                            origin: e.origin.clone(),
                            key: key.to_string(),
                            nth,
                        },
                    }
                })
                .collect();
            KeyLayers {
                key: key.to_string(),
                multi_valued: multi,
                layers,
            }
        })
        .collect()
}

/// Find the entry an [`EntryRef`] points at in a (fresh) snapshot.
pub fn resolve<'a>(snapshot: &'a ConfigSnapshot, at: &EntryRef) -> Option<&'a Entry> {
    snapshot
        .entries
        .iter()
        .filter(|e| e.key == at.key && e.origin == at.origin)
        .nth(at.nth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Scope;
    use std::path::PathBuf;

    fn e(key: &str, value: &str, scope: Scope, file: &str) -> Entry {
        Entry {
            key: key.into(),
            value: Some(value.into()),
            scope,
            origin: Origin::File(PathBuf::from(file)),
        }
    }

    fn snap(entries: Vec<Entry>) -> ConfigSnapshot {
        ConfigSnapshot { entries }
    }

    #[test]
    fn last_value_wins_for_single_valued_keys() {
        let s = snap(vec![
            e("user.email", "sys@x", Scope::System, "/etc/gitconfig"),
            e("user.email", "home@x", Scope::Global, "/h/.gitconfig"),
            e("core.editor", "vim", Scope::Global, "/h/.gitconfig"),
            e("user.email", "work@x", Scope::Local, "/r/.git/config"),
        ]);
        let l = layers(&s);
        assert_eq!(
            l.iter().map(|k| k.key.as_str()).collect::<Vec<_>>(),
            ["core.editor", "user.email"]
        );
        let email = &l[1];
        assert!(!email.multi_valued);
        let st: Vec<Status> = email.layers.iter().map(|x| x.status).collect();
        assert_eq!(st, [Status::Overridden, Status::Overridden, Status::Used]);
        assert_eq!(l[0].layers[0].status, Status::Used);
    }

    #[test]
    fn list_keys_use_every_value() {
        let s = snap(vec![
            e(
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
                Scope::Local,
                "/r/.git/config",
            ),
            e(
                "includeif.gitdir:~/w/.path",
                "a",
                Scope::Global,
                "/h/.gitconfig",
            ),
            e("x.custom", "1", Scope::Global, "/h/.gitconfig"),
            e("x.custom", "2", Scope::Global, "/h/.gitconfig"), // repeated in one file
        ]);
        for k in layers(&s) {
            assert!(k.multi_valued, "{}", k.key);
            assert!(k.layers.iter().all(|x| x.status == Status::AlsoUsed));
        }
        assert!(!is_multi_valued("user.name"));
        assert!(is_multi_valued("Remote.Origin.Fetch"));
    }

    #[test]
    fn refs_count_per_file_and_resolve() {
        let s = snap(vec![
            e("x.k", "a", Scope::Global, "/h/.gitconfig"),
            e("x.k", "b", Scope::Local, "/r/.git/config"),
            e("x.k", "c", Scope::Global, "/h/.gitconfig"),
        ]);
        let l = layers(&s);
        let refs: Vec<usize> = l[0].layers.iter().map(|x| x.at.nth).collect();
        assert_eq!(refs, [0, 0, 1]);
        for layer in &l[0].layers {
            assert_eq!(resolve(&s, &layer.at), Some(&layer.entry));
        }
        let missing = EntryRef {
            origin: Origin::File("/h/.gitconfig".into()),
            key: "x.k".into(),
            nth: 2,
        };
        assert_eq!(resolve(&s, &missing), None);
    }
}

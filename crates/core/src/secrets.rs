//! Hide credentials before a value is shown in the UI or CLI.

const MASK: &str = "••••";

/// Mask a config value for display, based on its key and contents.
///
/// - keys that name a secret (`*password*`, `*token*`, `*secret*`, `*.extraheader`)
///   are masked entirely;
/// - otherwise any `scheme://user:password@host` credentials inside the value
///   are masked (this covers `url.*.insteadOf`, remotes and helper arguments).
pub fn mask_value(key: &str, value: &str) -> String {
    let k = key.to_ascii_lowercase();
    let secret_key = ["password", "token", "secret", "passphrase"]
        .iter()
        .any(|w| k.contains(w))
        || k.ends_with(".extraheader");
    if secret_key && !value.is_empty() {
        return MASK.to_string();
    }
    mask_url_credentials(value)
}

/// Mask userinfo in every URL in `s`:
/// `https://u:tok@host` -> `https://u:••••@host`, `https://tok@host` -> `https://••••@host`.
/// `ssh://git@host` and scp-like `git@host:x` are left alone (a username, not a secret).
pub fn mask_url_credentials(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find("://") {
        let scheme_start = rest[..pos]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'))
            .map_or(0, |i| i + 1);
        let scheme = rest[scheme_start..pos].to_ascii_lowercase();
        let after = pos + 3;
        out.push_str(&rest[..after]);
        rest = &rest[after..];

        let authority_end = rest
            .find(|c: char| c == '/' || c == '?' || c == '#' || c.is_whitespace())
            .unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        if let Some(at) = authority.rfind('@') {
            let userinfo = &authority[..at];
            let masked = match userinfo.split_once(':') {
                Some((user, _)) => format!("{user}:{MASK}"),
                None if scheme == "ssh" || scheme == "git+ssh" => userinfo.to_string(),
                None => MASK.to_string(),
            };
            out.push_str(&masked);
            out.push_str(&authority[at..]);
            rest = &rest[authority_end..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_by_key() {
        assert_eq!(mask_value("sendemail.smtpPassword", "hunter2"), MASK);
        assert_eq!(
            mask_value(
                "http.https://x.com/.extraHeader",
                "Authorization: Bearer abc"
            ),
            MASK
        );
        assert_eq!(mask_value("user.name", "Ann"), "Ann");
    }

    #[test]
    fn masks_url_userinfo() {
        assert_eq!(
            mask_url_credentials("https://ann:ghp_abc@github.com/a/b.git"),
            "https://ann:••••@github.com/a/b.git"
        );
        assert_eq!(
            mask_url_credentials("https://ghp_abc@github.com/"),
            "https://••••@github.com/"
        );
        assert_eq!(
            mask_url_credentials("ssh://git@gitea.example.com/a.git"),
            "ssh://git@gitea.example.com/a.git"
        );
        assert_eq!(
            mask_url_credentials("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
        assert_eq!(
            mask_url_credentials("a https://u:p@h/x and http://q@r"),
            "a https://u:••••@h/x and http://••••@r"
        );
        assert_eq!(
            mask_url_credentials("https://github.com/"),
            "https://github.com/"
        );
    }
}

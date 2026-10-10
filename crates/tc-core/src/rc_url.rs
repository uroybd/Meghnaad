//! Which links the Worker will fetch a taskrc from, and the page-to-file rewrite for the common hosts.
//!
//! The Worker makes the request on the user's behalf, so the rules are strict: HTTPS to a public-looking host name
//! only (no addresses, no `localhost`, no credentials or ports in the link). The same rules apply to every redirect.

/// A link the Worker may fetch, rewritten to the raw file when it is a GitHub or GitLab "blob" page.
pub fn source_url(link: &str) -> Result<String, String> {
    let link = link.trim();
    if link.len() > 2048 {
        return Err("the link is too long".into());
    }
    let rest = link
        .strip_prefix("https://")
        .ok_or("the link must start with https://")?;
    if rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("the link has spaces or control characters in it".into());
    }
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    if authority.contains('@') {
        return Err("the link must not contain a user name or password; use a link that needs none".into());
    }
    if authority.contains(':') {
        return Err("the link must use the standard HTTPS port".into());
    }
    let host = authority.to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').collect();
    // A real top-level domain has a letter in it; this is what turns `10.0.0.5` away.
    let has_tld = labels
        .last()
        .is_some_and(|l| l.bytes().any(|b| b.is_ascii_alphabetic()));
    let valid = labels
        .iter()
        .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    if labels.len() < 2 || !valid || !has_tld || host.ends_with(".local") || host.ends_with(".internal") {
        return Err("the link must name a public web site".into());
    }
    let path = path.split('#').next().unwrap_or("");
    // A page that shows the file is not the file: use the host's raw address for it.
    if host == "github.com" {
        let parts: Vec<&str> = path.trim_start_matches('/').splitn(5, '/').collect();
        if let [user, repo, "blob", branch, file] = parts[..] {
            return Ok(format!(
                "https://raw.githubusercontent.com/{user}/{repo}/{branch}/{file}"
            ));
        }
    }
    if let Some((project, file)) = path.split_once("/-/blob/") {
        if host == "gitlab.com" {
            return Ok(format!("https://{host}{project}/-/raw/{file}"));
        }
    }
    Ok(format!("https://{host}{path}"))
}

/// Where a redirect from `base` leads, held to the same rules as the first link (so it can't leave for a
/// private address or plain HTTP). `location` is absolute or starts with `/`.
pub fn follow(base: &str, location: &str) -> Result<String, String> {
    let target = if location.starts_with('/') && !location.starts_with("//") {
        let rest = base.strip_prefix("https://").unwrap_or(base);
        let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        format!("https://{authority}{location}")
    } else {
        location.to_owned()
    };
    source_url(&target)
}

/// Whether a fetched body is a web page rather than the file (a wrong link is the likeliest mistake).
pub fn looks_like_html(body: &str) -> bool {
    let head: String = body
        .trim_start()
        .chars()
        .take(15)
        .collect::<String>()
        .to_ascii_lowercase();
    head.starts_with("<!doctype") || head.starts_with("<html")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_https_links_pass() {
        assert_eq!(
            source_url(" https://raw.githubusercontent.com/me/dots/main/.taskrc ").unwrap(),
            "https://raw.githubusercontent.com/me/dots/main/.taskrc"
        );
        assert_eq!(
            source_url("https://Example.COM/rc?token=abc").unwrap(),
            "https://example.com/rc?token=abc"
        );
        assert_eq!(source_url("https://example.com").unwrap(), "https://example.com");
    }

    #[test]
    fn github_and_gitlab_pages_become_the_raw_file() {
        assert_eq!(
            source_url("https://github.com/me/dots/blob/main/task/.taskrc").unwrap(),
            "https://raw.githubusercontent.com/me/dots/main/task/.taskrc"
        );
        assert_eq!(
            source_url("https://github.com/me/dots/blob/main/.taskrc#L10").unwrap(),
            "https://raw.githubusercontent.com/me/dots/main/.taskrc"
        );
        assert_eq!(
            source_url("https://gitlab.com/me/dots/-/blob/main/.taskrc").unwrap(),
            "https://gitlab.com/me/dots/-/raw/main/.taskrc"
        );
        // Not a blob page: left as it is.
        assert_eq!(
            source_url("https://github.com/me/dots/raw/main/.taskrc").unwrap(),
            "https://github.com/me/dots/raw/main/.taskrc"
        );
    }

    #[test]
    fn only_public_https_hosts_are_allowed() {
        for bad in [
            "http://example.com/rc",
            "ftp://example.com/rc",
            "example.com/rc",
            "https://localhost/rc",
            "https://intranet/rc",
            "https://127.0.0.1/rc",
            "https://10.0.0.5/rc",
            "https://[::1]/rc",
            "https://example.com:8443/rc",
            "https://me:pw@example.com/rc",
            "https://example.com@evil.test/rc",
            "https://printer.local/rc",
            "https://db.internal/rc",
            "https://exa mple.com/rc",
            "https://-.-/rc",
            "https:///rc",
            "",
            &format!("https://example.com/{}", "a".repeat(2100)),
        ] {
            assert!(source_url(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn redirects_are_held_to_the_same_rules() {
        let base = "https://github.com/me/dots/raw/main/.taskrc";
        assert_eq!(
            follow(base, "https://raw.githubusercontent.com/me/dots/main/.taskrc").unwrap(),
            "https://raw.githubusercontent.com/me/dots/main/.taskrc"
        );
        assert_eq!(follow(base, "/me/other").unwrap(), "https://github.com/me/other");
        for bad in [
            "http://example.com/x",
            "https://127.0.0.1/x",
            "//evil.test/x",
            "x/y",
            "",
            "https://u@a.test/x",
        ] {
            assert!(follow(base, bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn spots_a_web_page() {
        assert!(looks_like_html("  <!DOCTYPE html><html>"));
        assert!(looks_like_html("<html lang=en>"));
        assert!(!looks_like_html("data.location=~/.task\n"));
        assert!(!looks_like_html(""));
    }
}

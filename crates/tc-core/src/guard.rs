//! Small request-safety checks, kept pure so they can be tested natively.

/// True for loopback hosts: the only place the local-development auth bypass may apply.
///
/// Deliberately strict: `0.0.0.0` (reachable from other machines) and look-alikes such as
/// `localhost.evil.com` or `127.0.0.1.evil.com` are not local.
pub fn is_local_host(host: &str) -> bool {
    let h = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h == "::1" {
        return true;
    }
    // 127.0.0.0/8, written as exactly four decimal octets.
    let parts: Vec<&str> = h.split('.').collect();
    parts.len() == 4
        && parts[0] == "127"
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u16>().is_ok_and(|n| n <= 255))
}

/// Cross-site request check for state-changing requests.
///
/// Browsers attach an `Origin` header to cross-origin writes, so a page on another site that
/// manages to send a request using your logged-in session is recognisable by its origin. Clients
/// that aren't browsers (curl, scripts) send none, and there's no ambient session for them to
/// abuse, so a missing header is allowed. `"null"` (sandboxed frames, some redirects) is not.
pub fn origin_allowed(origin_header: Option<&str>, request_origin: &str) -> bool {
    match origin_header.map(str::trim) {
        None => true,
        Some("") | Some("null") => false,
        Some(o) => o.trim_end_matches('/').eq_ignore_ascii_case(request_origin.trim_end_matches('/')),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_are_local() {
        for h in ["localhost", "LOCALHOST", "127.0.0.1", "127.12.0.9", "[::1]", "::1", "app.localhost"] {
            assert!(is_local_host(h), "{h}");
        }
    }

    #[test]
    fn everything_else_is_not_local() {
        for h in [
            "example.com", "taskwarrior-web.me.workers.dev", "localhost.evil.com", "127.0.0.1.evil.com",
            "evil127.0.0.1", "0.0.0.0", "10.0.0.5", "192.168.1.2", "128.0.0.1", "127.0.0", "127.0.0.1.5",
            "127.0.0.256", "", "::2", "notlocalhost",
        ] {
            assert!(!is_local_host(h), "{h:?} must not count as local");
        }
    }

    #[test]
    fn same_origin_and_non_browser_clients_are_allowed() {
        let me = "https://tasks.example.com";
        assert!(origin_allowed(None, me));
        assert!(origin_allowed(Some("https://tasks.example.com"), me));
        assert!(origin_allowed(Some("HTTPS://Tasks.Example.com/"), me));
    }

    #[test]
    fn cross_site_and_opaque_origins_are_refused() {
        let me = "https://tasks.example.com";
        for o in ["https://evil.example", "http://tasks.example.com", "https://tasks.example.com.evil.io",
                  "https://tasks.example.com:8443", "null", "", "  "] {
            assert!(!origin_allowed(Some(o), me), "{o:?}");
        }
    }
}

//! The secrets a hooks script may use, and the hosts each may be sent to: what a host keeps (the Worker and the phone
//! seal it, each with its own key), how it is read and written, and the rules for a name, a value and a host. The hook
//! runner ([`crate::lua_hooks`]) only ever puts a secret's value into a request to one of its hosts; a script holds a
//! stand-in, never the value.
//!
//! The kept form is a JSON object, `{"NAME": {"value": "…", "hosts": ["acme.example.com"]}}`. The form before hosts,
//! `{"NAME": "value"}`, still reads, as secrets with no hosts, which can be sent nowhere until hosts are given.

use serde::Serialize;
use serde_json::Value;

/// The most secrets kept, and the largest value.
pub const MAX_SECRETS: usize = 64;
pub const MAX_VALUE: usize = 16 * 1024;
/// The most hosts one secret may have.
pub const MAX_HOSTS: usize = 16;

/// A secret: its value and the hosts it may be sent to (`acme.example.com`, or `*.example.com` for any name under it).
#[derive(Clone, PartialEq, Eq, Default, Serialize)]
pub struct Secret {
    pub value: String,
    pub hosts: Vec<String>,
}

impl Secret {
    /// `{"value": "…", "hosts": ["…"]}` (hosts may be left out). Read by hand from a `Value`, one reader for the kept file
    /// and a request alike, which keeps the Worker smaller than a derived one per source would.
    fn from_value(v: &Value) -> Option<Secret> {
        let o = v.as_object()?;
        let value = o.get("value")?.as_str()?.to_owned();
        let hosts = match o.get("hosts") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(a)) => a.iter().map(|h| h.as_str().map(str::to_owned)).collect::<Option<_>>()?,
            Some(_) => return None,
        };
        Some(Secret { value, hosts })
    }
}

impl std::fmt::Debug for Secret {
    /// Never the value: a secret that ends up in a log or an error shows only where it may go.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret")
            .field("hosts", &self.hosts)
            .finish_non_exhaustive()
    }
}

/// The secrets by name, kept sorted. A list rather than a map: there are at most [`MAX_SECRETS`], and a map of its own
/// type would add its own copy of the B-tree code to the Worker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Secrets(Vec<(String, Secret)>);

impl Serialize for Secrets {
    /// As a JSON object by name, the form [`Secrets::parse`] reads.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(k, v)| (k, v)))
    }
}

/// A name and its hosts, for the list a screen shows (never a value).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listed {
    pub name: String,
    pub hosts: Vec<String>,
}

impl Secrets {
    /// The kept bytes (after unsealing), in either form.
    pub fn parse(bytes: &[u8]) -> Result<Secrets, String> {
        let unreadable = |e: &dyn std::fmt::Display| format!("the saved secrets could not be read: {e}");
        let Value::Object(kept) = serde_json::from_slice::<Value>(bytes).map_err(|e| unreadable(&e))? else {
            return Err(unreadable(&"not an object"));
        };
        let mut out = Vec::with_capacity(kept.len());
        for (name, v) in kept {
            let secret = match v {
                Value::String(value) => Secret {
                    value,
                    hosts: Vec::new(),
                },
                other => Secret::from_value(&other).ok_or_else(|| unreadable(&format!("{name} is not a secret")))?,
            };
            out.push((name, secret));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Secrets(out))
    }

    /// The bytes to seal and keep.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    fn at(&self, name: &str) -> Result<usize, usize> {
        self.0.binary_search_by(|(n, _)| n.as_str().cmp(name))
    }

    pub fn get(&self, name: &str) -> Option<&Secret> {
        self.at(name).ok().and_then(|i| self.0.get(i)).map(|(_, s)| s)
    }

    /// Names and hosts, by name.
    pub fn listed(&self) -> Vec<Listed> {
        self.0
            .iter()
            .map(|(name, s)| Listed {
                name: name.clone(),
                hosts: s.hosts.clone(),
            })
            .collect()
    }

    /// Keep `secret` as `name`, after checking both. The hosts are only ever set together with the value, so nothing that
    /// can't read a value can send it somewhere new.
    pub fn set(&mut self, name: &str, secret: Secret) -> Result<(), String> {
        if !valid_name(name) {
            return Err("a secret's name is letters, digits and underscores, starting with a letter".into());
        }
        if secret.value.is_empty() || secret.value.len() > MAX_VALUE {
            return Err("a secret is between 1 character and 16 KB".into());
        }
        let secret = Secret {
            hosts: check_hosts(&secret.hosts)?,
            value: secret.value,
        };
        match self.at(name) {
            Ok(i) => {
                if let Some(slot) = self.0.get_mut(i) {
                    slot.1 = secret;
                }
            }
            Err(_) if self.0.len() >= MAX_SECRETS => return Err(format!("there can be at most {MAX_SECRETS} secrets")),
            Err(i) => self.0.insert(i, (name.to_owned(), secret)),
        }
        Ok(())
    }

    /// Whether there was one to remove.
    pub fn remove(&mut self, name: &str) -> bool {
        match self.at(name) {
            Ok(i) => {
                self.0.remove(i);
                true
            }
            Err(_) => false,
        }
    }
}

/// A secret's name: letters, digits and underscores, starting with a letter.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// One host as a script or a secret names it, lowercased: `acme.example.com` or `*.example.com`. `None` when it is not
/// a host name (a scheme, a path, a port or `user@` are not part of one).
pub fn host_pattern(text: &str) -> Option<String> {
    let h = text.trim().to_ascii_lowercase();
    let name = h.strip_prefix("*.").unwrap_or(&h);
    let ok = !name.is_empty()
        && name.len() <= 253
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.contains("..");
    ok.then_some(h)
}

/// A secret's hosts, checked, lowercased and without repeats.
pub fn check_hosts(hosts: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for h in hosts.iter().filter(|h| !h.trim().is_empty()) {
        let Some(p) = host_pattern(h) else {
            return Err(format!(
                "{:?} is not a host name (write just the name: acme.example.com, or *.example.com)",
                h.trim()
            ));
        };
        if !out.contains(&p) {
            out.push(p);
        }
    }
    if out.len() > MAX_HOSTS {
        return Err(format!("a secret may have at most {MAX_HOSTS} hosts"));
    }
    Ok(out)
}

/// Hosts written in one line, separated by commas or spaces (a Worker setting, an environment variable).
pub fn hosts_from_line(line: &str) -> Result<Vec<String>, String> {
    check_hosts(
        &line
            .split([',', ' ', '\t', '\n'])
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )
}

/// Whether `host` (from a request's address, lowercase) is one of `patterns`.
pub fn matches(patterns: &[String], host: &str) -> bool {
    patterns.iter().any(|p| match p.strip_prefix("*.") {
        Some(suffix) => {
            host.len() > suffix.len() + 1 && host.ends_with(suffix) && host[..host.len() - suffix.len()].ends_with('.')
        }
        None => p == host,
    })
}

/// What a request to save a secret holds: `{"value": "…", "hosts": […]}`, or (the form before hosts) the value as plain
/// text, which saves it with no hosts. A JSON object naming `value` or `hosts` has to be the whole first form, so hosts
/// sent without a value are refused rather than kept as a value; any other text (a JSON credential included) is a value.
pub fn from_request(body: &[u8]) -> Result<Secret, String> {
    let text = String::from_utf8(body.to_vec()).map_err(|_| "a secret has to be text".to_owned())?;
    if let Ok(v @ Value::Object(_)) = serde_json::from_str::<Value>(&text) {
        if v.get("value").is_some() || v.get("hosts").is_some() {
            return match v.get("value") {
                Some(Value::String(_)) => Secret::from_value(&v)
                    .ok_or_else(|| "a secret is {\"value\": \"…\", \"hosts\": [\"…\"]}".to_owned()),
                _ => Err("a secret's hosts are only ever set together with its value".into()),
            };
        }
    }
    Ok(Secret {
        value: text,
        hosts: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_before_hosts_still_reads_with_no_hosts() {
        let s = Secrets::parse(br#"{"JIRA":"tok","SLACK":{"value":"x","hosts":["hooks.slack.com"]}}"#).unwrap();
        assert_eq!(s.get("JIRA").unwrap().value, "tok");
        assert!(s.get("JIRA").unwrap().hosts.is_empty());
        assert_eq!(s.get("SLACK").unwrap().hosts, ["hooks.slack.com"]);
        assert_eq!(Secrets::parse(&s.to_bytes()).unwrap(), s);
    }

    #[test]
    fn names_values_and_hosts_are_checked() {
        let mut s = Secrets::default();
        let secret = |hosts: &[&str]| Secret {
            value: "v".into(),
            hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        };
        assert!(s.set("1x", secret(&[])).is_err());
        assert!(s.set("X", Secret::default()).is_err());
        for bad in [
            "https://a.example",
            "a.example/x",
            "a.example:443",
            "me@a.example",
            "*.",
            "a..b",
            ".a",
        ] {
            assert!(s.set("X", secret(&[bad])).is_err(), "{bad}");
        }
        s.set(
            "X",
            secret(&[" Acme.Example.com ", "*.example.org", "acme.example.com", ""]),
        )
        .unwrap();
        assert_eq!(s.get("X").unwrap().hosts, ["acme.example.com", "*.example.org"]);
        assert_eq!(
            s.listed(),
            [Listed {
                name: "X".into(),
                hosts: vec!["acme.example.com".into(), "*.example.org".into()]
            }]
        );
    }

    #[test]
    fn hosts_match_exactly_or_under_a_wildcard() {
        let p = vec!["acme.example.com".to_owned(), "*.example.org".to_owned()];
        assert!(matches(&p, "acme.example.com"));
        assert!(!matches(&p, "evil-acme.example.com"));
        assert!(!matches(&p, "acme.example.com.evil.net"));
        assert!(matches(&p, "api.example.org"));
        assert!(!matches(&p, "example.org"));
        assert!(!matches(&p, "notexample.org"));
        assert!(!matches(&[], "acme.example.com"));
    }

    #[test]
    fn a_request_is_json_with_hosts_or_plain_text_without() {
        let s = from_request(br#"{"value":"tok","hosts":["a.example"]}"#).unwrap();
        assert_eq!((s.value.as_str(), s.hosts.len()), ("tok", 1));
        assert!(from_request(b"plain-token").unwrap().hosts.is_empty());
        assert!(from_request(br#"{"hosts":["evil.example"]}"#).is_err());
        assert!(from_request(br#"{"value":1,"hosts":[]}"#).is_err());
        let credential = br#"{"type":"service_account","private_key":"k"}"#;
        assert_eq!(from_request(credential).unwrap().value.as_bytes(), credential);
        assert_eq!(hosts_from_line("a.example, *.b.example c.example").unwrap().len(), 3);
    }

    #[test]
    fn a_secret_never_shows_its_value_in_debug() {
        let s = Secret {
            value: "hunter2".into(),
            hosts: vec![],
        };
        assert!(!format!("{s:?}").contains("hunter2"));
    }
}

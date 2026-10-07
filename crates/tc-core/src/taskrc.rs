//! `taskrc` ingestion: UDA definitions, custom reports, contexts, urgency coefficients.
//!
//! Safety model. A taskrc usually also holds `sync.*` (encryption secret, bucket credentials)
//! and other secrets, and this module must never let those reach storage, logs or responses:
//!
//! 1. **Deny-list first**: `sync.*`, `taskd.*` and any key whose name looks credential-like is
//!    blocked before anything else looks at it. Only the key *name* is recorded.
//! 2. **Allowlist second**: only keys matching a known, harmless pattern are kept; everything
//!    else is dropped (name recorded as ignored).
//! 3. [`Parsed`] has no field that can hold a dropped value, so a leak is impossible by
//!    construction, not just by discipline.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Refuse absurdly large inputs; real taskrc files are a few KB.
pub const MAX_TASKRC_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UdaType {
    String,
    Numeric,
    Date,
    Duration,
    Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdaDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: UdaType,
    #[serde(default)]
    pub label: Option<String>,
    /// Allowed values, highest sort priority first. May contain `""` (blank is permitted).
    #[serde(default)]
    pub values: Vec<String>,
    pub default: Option<String>,
    pub indicator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Column specs, e.g. `id`, `description.truncated`, `due.relative`, `estimate`.
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    /// Raw sort spec, e.g. `due+,priority-,project+/`.
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
    /// Whether the active context applies (default true).
    #[serde(default = "yes")]
    pub context: bool,
    #[serde(default)]
    pub dateformat: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextDef {
    pub name: String,
    pub read: Option<String>,
    pub write: Option<String>,
}

/// Every field is optional on read, so settings saved by an older (or newer) version always load:
/// a missing field takes its default and an unknown one is ignored, instead of the whole config
/// being rejected and silently replaced by defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub udas: BTreeMap<String, UdaDef>,
    pub reports: BTreeMap<String, ReportDef>,
    pub contexts: BTreeMap<String, ContextDef>,
    pub active_context: Option<String>,
    /// `urgency.*` coefficients, keyed by the full setting name.
    pub urgency: BTreeMap<String, f64>,
    /// Allowlisted scalar settings (`default.command`, `dateformat*`, `weekstart`, ...).
    pub settings: BTreeMap<String, String>,
}

impl Config {
    /// `(start annotation, stop annotation)` when `journal.time` is on.
    pub fn journal(&self) -> Option<(String, String)> {
        let on = self.settings.get("journal.time").is_some_and(|v| truthy(v));
        let text = |key: &str, default: &str| {
            self.settings.get(key).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| default.to_owned())
        };
        on.then(|| {
            (
                text("journal.time.start.annotation", "Started task"),
                text("journal.time.stop.annotation", "Stopped task"),
            )
        })
    }
}

/// Result of parsing. Holds names of dropped keys, never their values.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Parsed {
    pub config: Config,
    /// Keys refused for being sensitive (`sync.*`, `taskd.*`, credential-like names).
    pub blocked: Vec<String>,
    /// Keys that are harmless but not used by the web UI.
    pub ignored: Vec<String>,
    pub warnings: Vec<String>,
}

const SENSITIVE_PREFIXES: &[&str] = &["sync.", "taskd."];
const SENSITIVE_WORDS: &[&str] = &[
    "secret",
    "password",
    "passwd",
    "passphrase",
    "token",
    "credential",
    "apikey",
    "api_key",
    "access_key",
    "private_key",
    "encryption",
];

/// True for any setting name that might carry a secret. Deliberately over-inclusive: a
/// false positive only hides a setting, a false negative leaks a credential.
pub fn is_sensitive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    SENSITIVE_PREFIXES.iter().any(|p| n.starts_with(p))
        || n == "sync"
        || SENSITIVE_WORDS.iter().any(|w| n.contains(w))
}

const SCALAR_SETTINGS: &[&str] = &[
    "default.command",
    "due",
    "journal.time",
    "journal.time.start.annotation",
    "journal.time.stop.annotation",
    "journal.info",
    "dateformat",
    "dateformat.report",
    "dateformat.info",
    "dateformat.annotation",
    "weekstart",
    "search.case.sensitive",
    "uda.priority.values",
];

fn valid_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Taskwarrior accepts JSON-style `\uNNNN` escapes in values.
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars().peekable();
    let mut pending_high: Option<u32> = None;
    while let Some(c) = it.next() {
        if c == '\\' && it.peek() == Some(&'u') {
            let mut look = it.clone();
            look.next();
            let hex: String = look.by_ref().take(4).collect();
            if hex.len() == 4 {
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    it = look;
                    match (pending_high.take(), cp) {
                        (Some(hi), lo) if (0xDC00..0xE000).contains(&lo) => {
                            let full = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                            out.extend(char::from_u32(full));
                        }
                        (_, hi) if (0xD800..0xDC00).contains(&hi) => pending_high = Some(hi),
                        (_, cp) => out.extend(char::from_u32(cp)),
                    }
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn split_list(v: &str) -> Vec<String> {
    // Whitespace within lists isn't permitted by Taskwarrior; trim anyway for forgiveness.
    v.split(',').map(|s| s.trim().to_owned()).collect()
}

fn truthy(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true")
}

/// Apply a command-line override (`rc.<key>:<value>`) to a copy of the config.
///
/// Held to the same rules as a taskrc: sensitive keys are refused, and only allowlisted settings
/// take effect (harmless unknown ones such as `rc.verbose` are accepted and ignored, as the
/// real `task` does). `report.<name>.<attr>` changes one attribute of a report, so
/// `rc.report.next.sort:due-` re-sorts `next` without redefining it.
pub fn apply_override(cfg: &mut Config, key: &str, value: &str) -> Result<(), String> {
    if is_sensitive(key) {
        return Err(format!("'rc.{key}' can't be set from a command line"));
    }
    let parts: Vec<&str> = key.split('.').collect();
    if let ["report", name, attr] = parts.as_slice() {
        let mut def = crate::report::resolve(cfg, name)
            .ok_or_else(|| format!("'{name}' is not a report (rc.{key})"))?;
        let v = unescape(value);
        match *attr {
            "columns" => def.columns = split_list(&v).into_iter().filter(|c| !c.is_empty()).collect(),
            "labels" => def.labels = split_list(&v),
            "sort" => def.sort = Some(v),
            "filter" => def.filter = Some(v).filter(|f| !f.is_empty()),
            "description" => def.description = Some(v),
            "context" => def.context = truthy(&v),
            "dateformat" => def.dateformat = Some(v),
            other => return Err(format!("'{other}' is not a report setting (rc.{key})")),
        }
        cfg.reports.insert((*name).to_owned(), def);
        return Ok(());
    }

    let p = parse(&format!("{key}={value}"));
    if !p.blocked.is_empty() {
        return Err(format!("'rc.{key}' can't be set from a command line"));
    }
    let c = p.config;
    cfg.udas.extend(c.udas);
    for (n, ctx) in c.contexts {
        let e = cfg.contexts.entry(n.clone()).or_insert_with(|| ContextDef { name: n, ..Default::default() });
        if ctx.read.is_some() {
            e.read = ctx.read;
        }
        if ctx.write.is_some() {
            e.write = ctx.write;
        }
    }
    if c.active_context.is_some() || parts == ["context"] {
        cfg.active_context = c.active_context;
    }
    cfg.urgency.extend(c.urgency);
    cfg.settings.extend(c.settings);
    Ok(())
}

/// The settings as a taskrc: what the web UI actually holds, in a form you can read, edit and
/// import again. Parsing the result gives back the same config (see the round-trip test).
/// Never contains anything that was blocked, since that was never stored.
pub fn render(cfg: &Config) -> String {
    let one_line = |s: &str| s.replace(['\n', '\r'], " ");
    let mut out = String::new();
    let mut line = |k: String, v: &str| {
        out.push_str(&k);
        out.push('=');
        out.push_str(&one_line(v));
        out.push('\n');
    };

    for u in cfg.udas.values() {
        let n = &u.name;
        line(format!("uda.{n}.type"), &format!("{:?}", u.ty).to_lowercase());
        if let Some(v) = &u.label {
            line(format!("uda.{n}.label"), v);
        }
        if !u.values.is_empty() {
            line(format!("uda.{n}.values"), &u.values.join(","));
        }
        if let Some(v) = &u.default {
            line(format!("uda.{n}.default"), v);
        }
        if let Some(v) = &u.indicator {
            line(format!("uda.{n}.indicator"), v);
        }
    }
    for r in cfg.reports.values() {
        let n = &r.name;
        if let Some(v) = &r.description {
            line(format!("report.{n}.description"), v);
        }
        if !r.columns.is_empty() {
            line(format!("report.{n}.columns"), &r.columns.join(","));
        }
        if !r.labels.is_empty() {
            line(format!("report.{n}.labels"), &r.labels.join(","));
        }
        if let Some(v) = &r.sort {
            line(format!("report.{n}.sort"), v);
        }
        if let Some(v) = &r.filter {
            line(format!("report.{n}.filter"), v);
        }
        if !r.context {
            line(format!("report.{n}.context"), "0");
        }
        if let Some(v) = &r.dateformat {
            line(format!("report.{n}.dateformat"), v);
        }
    }
    for c in cfg.contexts.values() {
        if let Some(v) = &c.read {
            line(format!("context.{}.read", c.name), v);
        }
        if let Some(v) = &c.write {
            line(format!("context.{}.write", c.name), v);
        }
    }
    if let Some(c) = &cfg.active_context {
        line("context".to_owned(), c);
    }
    for (k, v) in &cfg.urgency {
        line(k.clone(), &v.to_string());
    }
    for (k, v) in &cfg.settings {
        line(k.clone(), v);
    }
    out
}

pub fn parse(text: &str) -> Parsed {
    let mut p = Parsed::default();
    let mut includes = 0usize;
    let mut raw_uda: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut raw_report: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("include") {
            if rest.starts_with(char::is_whitespace) {
                includes += 1;
                continue;
            }
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        // Value is only used after the key passes both gates below.
        let value = value.trim();

        if is_sensitive(name) {
            p.blocked.push(name.to_owned());
            continue;
        }

        let parts: Vec<&str> = name.split('.').collect();
        match parts.as_slice() {
            ["uda", n, attr]
                if valid_ident(n)
                    && matches!(*attr, "type" | "label" | "values" | "default" | "indicator") =>
            {
                raw_uda
                    .entry((*n).to_owned())
                    .or_default()
                    .insert((*attr).to_owned(), unescape(value));
            }
            ["report", n, attr]
                if valid_ident(n)
                    && matches!(
                        *attr,
                        "description" | "columns" | "labels" | "sort" | "filter" | "context"
                            | "dateformat"
                    ) =>
            {
                raw_report
                    .entry((*n).to_owned())
                    .or_default()
                    .insert((*attr).to_owned(), unescape(value));
            }
            ["context", n, attr] if valid_ident(n) && matches!(*attr, "read" | "write") => {
                let c = p.config.contexts.entry((*n).to_owned()).or_insert_with(|| ContextDef {
                    name: (*n).to_owned(),
                    ..Default::default()
                });
                let v = Some(unescape(value));
                if *attr == "read" {
                    c.read = v;
                } else {
                    c.write = v;
                }
            }
            ["context"] => {
                p.config.active_context = (!value.is_empty()).then(|| value.to_owned());
            }
            ["urgency", ..] => match value.parse::<f64>() {
                Ok(f) if f.is_finite() => {
                    p.config.urgency.insert(name.to_owned(), f);
                }
                _ => p.warnings.push(format!("{name}: not a number, ignored")),
            },
            _ if SCALAR_SETTINGS.contains(&name) => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            _ => p.ignored.push(name.to_owned()),
        }
    }

    if includes > 0 {
        p.warnings.push(format!(
            "{includes} include directive(s) skipped: includes can't be resolved here; \
             paste the included files' contents too"
        ));
    }

    for (name, attrs) in raw_uda {
        let ty = match attrs.get("type").map(String::as_str) {
            Some("string") => UdaType::String,
            Some("numeric") => UdaType::Numeric,
            Some("date") => UdaType::Date,
            Some("duration") => UdaType::Duration,
            Some("uuid") => UdaType::Uuid,
            Some(other) => {
                p.warnings.push(format!("uda.{name}: unknown type {other:?}, UDA skipped"));
                continue;
            }
            None => {
                p.warnings.push(format!("uda.{name}: no type, UDA skipped"));
                continue;
            }
        };
        let def = UdaDef {
            name: name.clone(),
            ty,
            label: attrs.get("label").cloned().filter(|s| !s.is_empty()),
            values: attrs.get("values").map(|v| split_list(v)).unwrap_or_default(),
            default: attrs.get("default").cloned().filter(|s| !s.is_empty()),
            indicator: attrs.get("indicator").cloned().filter(|s| !s.is_empty()),
        };
        p.config.udas.insert(name, def);
    }

    for (name, attrs) in raw_report {
        let columns = attrs.get("columns").map(|v| split_list(v)).unwrap_or_default();
        let columns: Vec<String> = columns.into_iter().filter(|c| !c.is_empty()).collect();
        let overrides_builtin_only = columns.is_empty();
        if overrides_builtin_only && !crate::report::BUILTIN_NAMES.contains(&name.as_str()) {
            p.warnings.push(format!("report.{name}: no columns, report skipped"));
            continue;
        }
        let labels = attrs.get("labels").map(|v| split_list(v)).unwrap_or_default();
        if !labels.is_empty() && !columns.is_empty() && labels.len() != columns.len() {
            p.warnings.push(format!(
                "report.{name}: {} labels for {} columns",
                labels.len(),
                columns.len()
            ));
        }
        p.config.reports.insert(
            name.clone(),
            ReportDef {
                name,
                description: attrs.get("description").cloned(),
                columns,
                labels,
                sort: attrs.get("sort").cloned(),
                filter: attrs.get("filter").cloned(),
                context: attrs.get("context").map(|v| truthy(v)).unwrap_or(true),
                dateformat: attrs.get("dateformat").cloned(),
            },
        );
    }

    p.blocked.sort();
    p.blocked.dedup();
    p.ignored.sort();
    p.ignored.dedup();
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# comment
data.location=~/.task
include dark-256.theme
uda.estimate.type=string
uda.estimate.label=Size Estimate
uda.estimate.values=huge,large,medium,small,trivial,
uda.points.type=numeric
uda.points.default=1
uda.bad.type=wat
uda.notype.label=x
report.mine.description=My tasks
report.mine.columns=id,project,description.truncated,estimate,due.relative
report.mine.labels=ID,Proj,Desc,Est,Due
report.mine.sort=due+,priority-,project+/
report.mine.filter=status:pending +work
report.mine.context=0
report.next.filter=status:pending limit:10
context.work.read=+work or project:Work
context=work
urgency.uda.estimate.huge.coefficient=-3.0
urgency.user.tag.next.coefficient=15.0
urgency.age.max=oops
default.command=mine
verbose=nothing
"#;

    #[test]
    fn parses_udas() {
        let p = parse(SAMPLE);
        let e = &p.config.udas["estimate"];
        assert_eq!(e.ty, UdaType::String);
        assert_eq!(e.label.as_deref(), Some("Size Estimate"));
        assert_eq!(e.values, ["huge", "large", "medium", "small", "trivial", ""]);
        assert_eq!(p.config.udas["points"].default.as_deref(), Some("1"));
        assert!(!p.config.udas.contains_key("bad"));
        assert!(!p.config.udas.contains_key("notype"));
        assert!(p.warnings.iter().any(|w| w.contains("uda.bad")));
    }

    #[test]
    fn parses_reports_contexts_urgency() {
        let p = parse(SAMPLE);
        let r = &p.config.reports["mine"];
        assert_eq!(r.columns.len(), 5);
        assert_eq!(r.labels.len(), 5);
        assert_eq!(r.sort.as_deref(), Some("due+,priority-,project+/"));
        assert_eq!(r.filter.as_deref(), Some("status:pending +work"));
        assert!(!r.context);
        // Overriding a built-in report's filter alone is valid.
        assert_eq!(p.config.reports["next"].filter.as_deref(), Some("status:pending limit:10"));
        assert_eq!(p.config.contexts["work"].read.as_deref(), Some("+work or project:Work"));
        assert_eq!(p.config.active_context.as_deref(), Some("work"));
        assert_eq!(p.config.urgency["urgency.user.tag.next.coefficient"], 15.0);
        assert!(!p.config.urgency.contains_key("urgency.age.max"));
        assert_eq!(p.config.settings["default.command"], "mine");
    }

    #[test]
    fn include_is_reported_not_followed() {
        let p = parse(SAMPLE);
        assert!(p.warnings.iter().any(|w| w.contains("include")));
    }

    #[test]
    fn unknown_harmless_keys_are_ignored_by_name() {
        let p = parse(SAMPLE);
        assert!(p.ignored.contains(&"data.location".to_string()));
        assert!(p.ignored.contains(&"verbose".to_string()));
    }

    const SECRETS: &[(&str, &str)] = &[
        ("sync.encryption_secret", "hunter2-ENCRYPTION"),
        ("sync.aws.access_key_id", "AKIAEXAMPLEKEYID"),
        ("sync.aws.secret_access_key", "wJalrXUtnFEMI-SECRETKEY"),
        ("sync.aws.bucket", "my-private-bucket"),
        ("sync.aws.endpoint_url", "https://acct123.r2.cloudflarestorage.com"),
        ("sync.server.client_id", "11111111-2222-3333-4444-555555555555"),
        ("sync.server.url", "https://tasks.example.com"),
        ("sync.gcp.credential_path", "/home/me/gcp.json"),
        ("sync.local.server_dir", "/home/me/tasksync"),
        ("taskd.server", "taskd.example.com:53589"),
        ("taskd.certificate", "/home/me/.task/me.cert"),
        ("taskd.credentials", "org/me/aaaa-bbbb"),
        ("SYNC.AWS.REGION", "us-east-1"),
        ("  sync.encryption_secret  ", "spaced-SECRET"),
        ("my.api_token", "tok-123456"),
        ("webhook.password", "pw-abcdef"),
    ];

    #[test]
    fn sync_and_credential_keys_are_blocked_and_never_stored() {
        let mut rc = String::from(SAMPLE);
        for (k, v) in SECRETS {
            rc.push_str(&format!("{k}={v}\n"));
        }
        // Even if a secret is smuggled into an otherwise-allowed key's *name* position.
        rc.push_str("uda.estimate.sync.token=smuggled-TOKEN\n");

        let p = parse(&rc);
        let stored = serde_json::to_string(&p.config).unwrap();
        let everything = serde_json::to_string(&p).unwrap();

        for (k, v) in SECRETS {
            assert!(!everything.contains(v), "value of {k:?} leaked: {everything}");
            assert!(!stored.contains(k.trim()), "key {k:?} stored");
        }
        assert!(!everything.contains("smuggled-TOKEN"));
        assert!(!everything.contains("hunter2"));
        // The user is told which names were refused (names only).
        for name in ["sync.encryption_secret", "sync.aws.secret_access_key", "taskd.server",
                     "SYNC.AWS.REGION", "my.api_token", "webhook.password"] {
            assert!(p.blocked.iter().any(|b| b == name), "{name} not reported blocked");
        }
        // Legitimate config survives alongside.
        assert!(p.config.udas.contains_key("estimate"));
        assert!(p.config.reports.contains_key("mine"));
    }

    #[test]
    fn legit_keys_that_merely_contain_key_are_not_blocked() {
        // urgency.user.keyword.* is a real Taskwarrior setting; "key" alone isn't sensitive.
        let p = parse("urgency.user.keyword.foo.coefficient=2.0\n");
        assert!(p.blocked.is_empty());
        assert_eq!(p.config.urgency["urgency.user.keyword.foo.coefficient"], 2.0);
    }

    #[test]
    fn unicode_escapes_decode() {
        let p = parse("uda.x.type=string\nuda.x.label=Caf\\u00e9 \\ud83d\\ude00\n");
        assert_eq!(p.config.udas["x"].label.as_deref(), Some("Café 😀"));
    }

    #[test]
    fn tolerates_whitespace_and_garbage() {
        let p = parse("  uda.a.type  =  numeric  \nnot a setting\n=\n\n#x\n");
        assert_eq!(p.config.udas["a"].ty, UdaType::Numeric);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    const FULL: &str = r#"
uda.estimate.type=string
uda.estimate.label=Size Estimate
uda.estimate.values=huge,large,small,
uda.estimate.default=small
uda.estimate.indicator=E
uda.points.type=numeric
uda.shipby.type=date
report.mine.description=My tasks
report.mine.columns=id,project,description.truncated,estimate,due.relative
report.mine.labels=ID,Proj,Desc,Est,Due
report.mine.sort=due+,priority-,project+/
report.mine.filter=status:pending +work
report.mine.context=0
report.mine.dateformat=Y-M-D
report.next.filter=status:pending limit:10
context.work.read=+work or project:Work
context.work.write=+work
context.home.read=project:Home
context=work
urgency.uda.estimate.huge.coefficient=-3.0
urgency.user.tag.next.coefficient=15
journal.time=on
journal.time.start.annotation=Clock in
default.command=mine
due=14
dateformat.report=Y-M-D H:N
"#;

    #[test]
    fn render_then_parse_gives_back_the_same_config() {
        let cfg = parse(FULL).config;
        assert!(!cfg.udas.is_empty() && !cfg.reports.is_empty() && !cfg.contexts.is_empty());
        let text = render(&cfg);
        let again = parse(&text);
        assert_eq!(again.config, cfg, "rendered:\n{text}");
        assert!(again.warnings.is_empty() && again.blocked.is_empty() && again.ignored.is_empty(), "{again:?}");
        // And it is a fixed point: rendering again changes nothing.
        assert_eq!(render(&again.config), text);
    }

    #[test]
    fn an_empty_config_renders_empty_and_survives() {
        assert_eq!(render(&Config::default()), "");
        assert_eq!(parse("").config, Config::default());
    }

    #[test]
    fn the_rendered_text_never_holds_a_blocked_setting() {
        let cfg = parse(&format!("{FULL}\nsync.encryption_secret=TOPSECRET\nsync.aws.bucket=b\nmy.api_token=tok\n")).config;
        let text = render(&cfg);
        assert!(!text.contains("TOPSECRET") && !text.contains("sync.") && !text.contains("api_token"), "{text}");
    }

    #[test]
    fn settings_saved_by_another_version_still_load() {
        // Missing fields take defaults; unknown fields are ignored. Neither rejects the config.
        let older = r#"{ "udas": { "x": { "name": "x", "type": "numeric" } },
                         "reports": { "r": { "name": "r", "columns": ["id"] } } }"#;
        let c: Config = serde_json::from_str(older).unwrap();
        assert_eq!(c.udas["x"].ty, UdaType::Numeric);
        assert!(c.udas["x"].values.is_empty());
        assert!(c.reports["r"].context, "context defaults to on");
        assert!(c.contexts.is_empty() && c.settings.is_empty());

        let newer = r#"{ "udas": {}, "future_field": [1,2,3], "settings": { "due": "7" } }"#;
        let c: Config = serde_json::from_str(newer).unwrap();
        assert_eq!(c.settings["due"], "7");

        // A config round-trips through JSON (this is how it is stored).
        let cfg = parse(FULL).config;
        let back: Config = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn multi_line_values_cannot_inject_extra_settings() {
        let mut cfg = Config::default();
        cfg.settings.insert("default.command".into(), "next\nsync.encryption_secret=x".into());
        let text = render(&cfg);
        let p = parse(&text);
        assert!(p.blocked.is_empty(), "an embedded newline must not create a second setting: {text:?}");
    }
}

//! `import`: tasks from the JSON `task export` writes. A port of Taskwarrior's `CmdImport` and the parts of
//! `Task::parseJSON` and `Task::validate` it relies on.
//!
//! A task is matched by its uuid. A new one is added; one that is already there is compared with the
//! incoming one (a generated `modified`, `entry` or `end` doesn't count) and, if they differ, takes the
//! incoming one's attributes whole, so what the file leaves out is removed; if not, it is skipped. That
//! makes importing the same file twice harmless.
//!
//! Unlike `task`, which imports a task at a time, this reads and checks the whole file before anything is
//! written and then writes it in one step, so a file with a bad task in it changes nothing and a good one
//! is one `undo`.

use crate::cli::{apply_changes, load_facts, UndoStack};
use crate::dates::{parse_date, Clock};
use crate::filter::EvalCtx;
use crate::hooks::{Hooks, Runner};
use crate::model::Facts;
use crate::modify::{self, Change, Mode};
use crate::taskrc::Config;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;
use taskchampion::storage::Storage;
use taskchampion::{Operations, Replica, Status, Tag};
use uuid::Uuid;

/// The most tasks one import takes, and the most it lists in its report.
pub const MAX_TASKS: usize = 5000;
const MAX_LINES: usize = 300;

/// One task of the report.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// `add`, `mod` or `skip`.
    pub action: &'static str,
    pub uuid: Uuid,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ImportOut {
    /// Whether it was written (a check only reads).
    pub applied: bool,
    pub added: usize,
    pub modified: usize,
    pub skipped: usize,
    /// The first of the tasks, and how many more there were.
    pub lines: Vec<Entry>,
    pub more: usize,
    pub warnings: Vec<String>,
    /// What the hooks printed.
    pub feedback: Vec<crate::hooks::Line>,
}

/// The tasks of a file: one object, an array of them, or one object to a line (the oldest form).
fn read(text: &str) -> Result<Vec<Map<String, Value>>, String> {
    let text = text.trim_start_matches('\u{feff}').trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let object = |v: Value, at: &str| match v {
        Value::Object(o) => Ok(o),
        _ => Err(format!("Invalid JSON: {at} is not a task.")),
    };
    // From bytes, which the settings reader uses too: the parser is only compiled once.
    match serde_json::from_slice::<Value>(text.as_bytes()) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .enumerate()
            .map(|(i, v)| object(v, &format!("item {}", i + 1)))
            .collect(),
        Ok(v) => Ok(vec![object(v, "the file")?]),
        Err(e) if !text.starts_with('{') => Err(format!("Invalid JSON: {e}")),
        Err(_) => text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| match serde_json::from_slice::<Value>(l.as_bytes()) {
                Ok(v) => object(v, &format!("line {}", i + 1)),
                Err(e) => Err(format!("Invalid JSON on line {}: {e}", i + 1)),
            })
            .collect(),
    }
}

/// A date as the file writes it (`20261225T083000Z`, `2026-12-25T08:30:00Z`, a date, or an epoch).
fn epoch(text: &str, clock: &Clock) -> Option<i64> {
    if text.len() >= 9 && text.bytes().all(|c| c.is_ascii_digit()) {
        return text.parse().ok();
    }
    parse_date(text, clock).map(|d| d.ts)
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A task as it will be stored.
struct Incoming {
    uuid: Uuid,
    props: BTreeMap<String, String>,
    /// `entry` and `end` were made up here, not in the file, so they don't count as a difference.
    generated_entry: bool,
    generated_end: bool,
    /// How many times the file had this uuid.
    seen: usize,
}

/// What `Task::parseJSON` and `Task::validate` make of one object. `defaults` are what a task gets for
/// the attributes it lacks (`default.project`, a UDA's default).
fn convert(
    obj: &Map<String, Value>,
    cfg: &Config,
    clock: &Clock,
    defaults: &[(String, String)],
    warnings: &mut Vec<String>,
) -> Result<Incoming, String> {
    if obj.is_empty() {
        return Err("Cannot import an empty task.".into());
    }
    let uuid = match obj.get("uuid").map(text_of) {
        Some(u) => Uuid::parse_str(&u).map_err(|_| format!("Not a valid UUID '{u}'."))?,
        None => crate::crypto::new_uuid().map_err(|e| e.to_string())?,
    };
    let now = clock.now;
    let date =
        |key: &str, text: &str| epoch(text, clock).ok_or_else(|| format!("{key}: '{text}' is not a valid date."));
    let mut props: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in obj {
        match key.as_str() {
            "id" | "urgency" | "uuid" => {}
            "tags" => {
                for tag in value.as_array().into_iter().flatten().filter_map(Value::as_str) {
                    if Tag::from_str(tag).is_ok() {
                        props.insert(format!("tag_{tag}"), String::new());
                    }
                }
            }
            "depends" => {
                // An array of uuids, or the older comma-separated string (sometimes written as `["…","…"]`).
                let list: Vec<String> = match value {
                    Value::Array(items) => items.iter().map(text_of).collect(),
                    Value::String(s) => {
                        let bare = s.starts_with('[') && s.ends_with(']');
                        let s: String = s
                            .chars()
                            .filter(|c| !bare || c.is_ascii_hexdigit() || matches!(c, ',' | '-'))
                            .collect();
                        s.split(',').map(str::to_owned).collect()
                    }
                    _ => Vec::new(),
                };
                for dep in list.iter().filter(|d| !d.is_empty()) {
                    let id = Uuid::parse_str(dep).map_err(|_| format!("Not a valid UUID '{dep}'."))?;
                    props.insert(format!("dep_{id}"), String::new());
                }
            }
            "annotations" => {
                let Value::Array(notes) = value else {
                    return Err(format!("Annotations is malformed: {value}"));
                };
                for note in notes {
                    let Some(what) = note.get("description").and_then(Value::as_str) else {
                        return Err(format!("Annotation is missing a description: {note}"));
                    };
                    let mut at = match note.get("entry").and_then(Value::as_str) {
                        Some(t) => date("annotation entry", t)?,
                        None => now,
                    };
                    // Two in the same second would overwrite each other: move the later one on.
                    while props.contains_key(&format!("annotation_{at}")) {
                        at += 1;
                    }
                    props.insert(format!("annotation_{at}"), what.to_owned());
                }
            }
            k => {
                let (k, text) = (if k == "modification" { "modified" } else { k }, text_of(value));
                if crate::run::kind_of(k, cfg) == "date" {
                    props.insert(k.to_owned(), date(k, &text)?.to_string());
                } else if !text.is_empty() {
                    props.insert(k.to_owned(), text);
                }
            }
        }
    }

    // `Task::validate`: the status, then the dates that depend on it.
    let given = props.get("status").cloned().unwrap_or_default();
    let mut status = match given.as_str() {
        "" | "pending" | "waiting" => "pending",
        "completed" => "completed",
        "deleted" => "deleted",
        "recurring" => "recurring",
        other => return Err(format!("The status '{other}' is not valid.")),
    };
    let has = |p: &BTreeMap<String, String>, k: &str| p.contains_key(k);
    if status == "pending" && has(&props, "due") && has(&props, "recur") && !has(&props, "parent") {
        status = "recurring";
    }
    props.insert("status".into(), status.into());
    if status == "recurring" && !has(&props, "rtype") {
        props.insert("rtype".into(), "periodic".into());
    }
    let short = &uuid.to_string()[..8];
    let generated_entry = !has(&props, "entry");
    if generated_entry {
        props.insert("entry".into(), now.to_string());
    }
    let generated_end = matches!(status, "completed" | "deleted") && !has(&props, "end");
    if generated_end {
        props.insert("end".into(), now.to_string());
    }
    if status == "pending" {
        props.remove("end");
    }
    if !has(&props, "modified") {
        props.insert("modified".into(), now.to_string());
    }
    // What a new task gets for what it lacks (not an instance of a recurring task, which has its parent's).
    if !has(&props, "parent") {
        for (k, v) in defaults {
            props.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    let warn = |warnings: &mut Vec<String>, m: String| warnings.push(format!("{m} (task {short})"));
    let at = |p: &BTreeMap<String, String>, k: &str| p.get(k).and_then(|v| v.parse::<i64>().ok());
    for (left, right) in [
        ("wait", "due"),
        ("entry", "start"),
        ("entry", "end"),
        ("wait", "scheduled"),
        ("scheduled", "start"),
        ("scheduled", "due"),
        ("scheduled", "end"),
    ] {
        if let (Some(l), Some(r)) = (at(&props, left), at(&props, right)) {
            if l > r && r != 0 {
                let m = format!("Warning: You have specified that the '{left}' date is after the '{right}' date.");
                warn(warnings, m);
            }
        }
    }
    if props.get("description").is_none_or(String::is_empty) {
        warn(warnings, "Warning: task has no description.".into());
    }
    if let Some(recur) = props.get("recur").cloned() {
        if !has(&props, "due") {
            warn(warnings, "Warning: recurring task has no due date.".into());
            props.remove("recur");
        } else if crate::recur::validate_period(&recur).is_err() {
            warn(
                warnings,
                format!("Warning: The recurrence value '{recur}' is not valid."),
            );
            props.remove("recur");
        }
    }
    Ok(Incoming {
        uuid,
        props,
        generated_entry,
        generated_end,
        seen: 1,
    })
}

/// `import`. With `apply` false it only reads: the report says what would happen.
///
/// When it fails, what the hooks printed still reaches the caller, after the reason, and `on_exit` still runs.
pub async fn import<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    text: &str,
    apply: bool,
    undo: &mut UndoStack,
    hooks: Option<Arc<dyn Hooks>>,
) -> Result<ImportOut, String> {
    let hk = Runner::new(hooks, cfg.hooks());
    let said = |m: String, lines: Vec<crate::hooks::Line>| lines.iter().fold(m, |m, l| format!("{m}\n{}", l.text));
    if let Err(m) = hk.launch("import") {
        return Err(said(m, hk.abort()));
    }
    match run(replica, cfg, clock, text, apply, undo, &hk).await {
        Ok(mut out) => {
            out.feedback = hk.finish(false);
            Ok(out)
        }
        Err(m) => Err(said(m, hk.finish(true))),
    }
}

async fn run<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    text: &str,
    apply: bool,
    undo: &mut UndoStack,
    hk: &Runner,
) -> Result<ImportOut, String> {
    let objects = read(text)?;
    if objects.len() > MAX_TASKS {
        return Err(format!(
            "The file has {} tasks; import at most {MAX_TASKS} at a time.",
            objects.len()
        ));
    }
    let e = |e: taskchampion::Error| e.to_string();
    let all = load_facts(replica).await.map_err(e)?;
    let ids = crate::run::working_set_ids(&all);
    let ctx = EvalCtx::new(cfg, clock, &ids);

    // What a new task gets for what it lacks: `plan` for a bare `add` says.
    let bare = modify::Mods {
        words: vec!["x".into()],
        ..Default::default()
    };
    let defaults: Vec<(String, String)> = modify::plan(&bare, Mode::Add, None, &ctx, &all)
        .map_err(|e| e.0)?
        .into_iter()
        .filter_map(|c| match c {
            Change::Prop { name, value: Some(v) } => Some((name, v)),
            Change::Timestamp { name, value: Some(v) } => Some((name.to_owned(), v.to_string())),
            _ => None,
        })
        .collect();

    // Read and check everything first. A uuid that comes twice is one task: the later wins.
    let mut warnings = Vec::new();
    let mut incoming: Vec<Incoming> = Vec::with_capacity(objects.len());
    let mut at: BTreeMap<Uuid, usize> = BTreeMap::new();
    for (i, obj) in objects.iter().enumerate() {
        let task = convert(obj, cfg, &clock, &defaults, &mut warnings).map_err(|m| format!("Task {}: {m}", i + 1))?;
        match at.get(&task.uuid) {
            Some(&j) => {
                incoming[j] = Incoming {
                    seen: incoming[j].seen + 1,
                    ..task
                }
            }
            None => {
                at.insert(task.uuid, incoming.len());
                incoming.push(task);
            }
        }
    }
    for t in incoming.iter().filter(|t| t.seen > 1) {
        warnings.push(format!("Input contains UUID '{}' {} times.", t.uuid, t.seen));
    }
    if incoming.iter().any(|t| t.seen > 1) {
        warnings.push("Tasks with the same UUID have been merged. Please check the results.".into());
    }

    let now = clock.now.to_string();
    let mut ops = Operations::new();
    let mut out = ImportOut {
        applied: false,
        added: 0,
        modified: 0,
        skipped: 0,
        lines: Vec::new(),
        more: 0,
        warnings,
        feedback: Vec::new(),
    };
    for t in incoming {
        let description = t.props.get("description").cloned().unwrap_or_default();
        let mut old: Option<BTreeMap<String, String>> = None;
        if let Some(data) = replica.get_task_data(t.uuid).await.map_err(e)? {
            let map = old.get_or_insert_with(BTreeMap::new);
            for (k, v) in data.iter() {
                map.insert(k.clone(), v.clone());
            }
        }
        // A task that is there keeps the time of its last change and any `entry` or `end` that was made up
        // here (the file had none), as Taskwarrior does: they must neither count as a difference nor be written.
        let mut props = t.props;
        if let Some(old) = &old {
            let mut keep = vec!["modified"];
            keep.extend(t.generated_entry.then_some("entry"));
            keep.extend(t.generated_end.then_some("end"));
            for k in keep {
                match old.get(k) {
                    Some(v) => props.insert(k.to_owned(), v.clone()),
                    None => props.remove(k),
                };
            }
        }
        let action = match &old {
            None => "add",
            Some(old) if &props == old => "skip",
            Some(_) => "mod",
        };
        match action {
            "add" => out.added += 1,
            "mod" => out.modified += 1,
            _ => out.skipped += 1,
        }
        if out.lines.len() < MAX_LINES {
            out.lines.push(Entry {
                action,
                uuid: t.uuid,
                description,
            });
        } else {
            out.more += 1;
        }
        if action == "skip" {
            continue;
        }

        // Every attribute but `status`, which goes last (it settles `end`), as in `TDB2::add`.
        let mut new = props;
        let mut task = match &old {
            None => replica.create_task(t.uuid, &mut ops).await.map_err(e)?,
            Some(_) => replica
                .get_task(t.uuid)
                .await
                .map_err(e)?
                .ok_or_else(|| format!("task {} disappeared", t.uuid))?,
        };
        let before = Facts::from_task(&task);
        if old.is_some() {
            new.insert("modified".into(), now.clone());
        }
        let status = new.remove("status").unwrap_or_default();
        let modified = new.remove("modified");
        task.set_value("modified", modified, &mut ops).map_err(e)?;
        for (k, v) in &new {
            if old.as_ref().is_none_or(|o| o.get(k) != Some(v)) {
                task.set_value(k.clone(), Some(v.clone()), &mut ops).map_err(e)?;
            }
        }
        for k in old.iter().flat_map(BTreeMap::keys) {
            if !new.contains_key(k) && !matches!(k.as_str(), "status" | "modified") {
                task.set_value(k.clone(), None, &mut ops).map_err(e)?;
            }
        }
        if old.as_ref().and_then(|o| o.get("status")) != Some(&status) {
            let status = match status.as_str() {
                "completed" => Status::Completed,
                "deleted" => Status::Deleted,
                "recurring" => Status::Recurring,
                _ => Status::Pending,
            };
            task.set_status(status, &mut ops).map_err(e)?;
        }
        // `on-add` or `on-modify`: the hook sees the task as it will be and may change it or refuse.
        let after = Facts::from_task(&task);
        let hooked = if old.is_some() {
            hk.modify(&before, after)?
        } else {
            hk.add(after)?
        };
        apply_changes(&mut task, &hooked, &mut ops)?;
    }

    if apply && !ops.is_empty() {
        undo.push(&ops);
        replica.commit_operations(ops).await.map_err(e)?;
        out.applied = true;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::clock;

    fn tasks(text: &str) -> Result<Vec<Map<String, Value>>, String> {
        read(text)
    }

    #[test]
    fn a_file_is_one_task_an_array_or_a_task_a_line() {
        assert_eq!(tasks(r#"{"description":"a"}"#).unwrap().len(), 1);
        assert_eq!(tasks(r#"[{"description":"a"},{"description":"b"}]"#).unwrap().len(), 2);
        assert_eq!(
            tasks("{\"description\":\"a\"}\n\n{\"description\":\"b\"}\n")
                .unwrap()
                .len(),
            2
        );
        assert_eq!(tasks("  \n").unwrap().len(), 0);
        assert_eq!(tasks("[]").unwrap().len(), 0);
        assert!(tasks("[1,2]").unwrap_err().contains("item 1 is not a task"));
        assert!(tasks("not json").unwrap_err().starts_with("Invalid JSON"));
        let e = tasks("{\"description\":\"a\"}\n{broken\n").unwrap_err();
        assert!(e.contains("line 2"), "{e}");
    }

    #[test]
    fn dates_come_in_the_forms_taskwarrior_reads() {
        let c = clock();
        let want = Some(1_798_187_400); // 2026-12-25T08:30:00Z
        assert_eq!(epoch("20261225T083000Z", &c), want);
        assert_eq!(epoch("2026-12-25T08:30:00Z", &c), want);
        assert_eq!(epoch("1798187400", &c), want);
        assert_eq!(epoch("garbage", &c), None);
    }

    #[test]
    fn an_object_becomes_the_properties_taskchampion_stores() {
        let c = clock();
        let cfg = Config::default();
        let obj: Map<String, Value> = serde_json::from_str(
            r#"{"id":9,"urgency":3,"uuid":"11111111-1111-4111-8111-111111111111","description":"First",
                "status":"waiting","wait":"20271001T000000Z","entry":"20261001T100000Z","tags":["a","b"],
                "depends":"22222222-2222-4222-8222-222222222222","annotations":[{"entry":"20261002T000000Z","description":"n"},
                {"entry":"20261002T000000Z","description":"m"}],"est":3.5,"mystery":"orphan","project":""}"#,
        )
        .unwrap();
        let mut warnings = Vec::new();
        let t = convert(&obj, &cfg, &c, &[], &mut warnings).unwrap();
        let p = &t.props;
        assert_eq!(p["status"], "pending", "waiting is a pending task with a wait");
        assert_eq!(p["wait"], "1822348800");
        assert_eq!(p["entry"], "1790848800");
        assert!(!t.generated_entry && !t.generated_end);
        assert!(p.contains_key("tag_a") && p.contains_key("tag_b") && !p.contains_key("tags"));
        assert!(p.contains_key("dep_22222222-2222-4222-8222-222222222222"));
        // Two notes in the same second keep both.
        assert_eq!(p["annotation_1790899200"], "n");
        assert_eq!(p["annotation_1790899201"], "m");
        assert_eq!((p["est"].as_str(), p["mystery"].as_str()), ("3.5", "orphan"));
        assert!(!p.contains_key("project") && !p.contains_key("id") && !p.contains_key("urgency"));
        assert!(p.contains_key("modified") && warnings.is_empty());
    }

    #[test]
    fn what_a_file_leaves_out_is_filled_in_as_validate_does() {
        let c = clock();
        let cfg = Config::default();
        let one = |json: &str| {
            let obj: Map<String, Value> = serde_json::from_str(json).unwrap();
            let mut w = Vec::new();
            let t = convert(&obj, &cfg, &c, &[], &mut w).unwrap();
            (t, w)
        };
        let (t, _) = one(r#"{"description":"done","status":"completed"}"#);
        assert!(t.generated_entry && t.generated_end);
        assert_eq!(t.props["end"], c.now.to_string());
        // A pending task has no end.
        let (t, _) = one(r#"{"description":"p","end":"20261001T000000Z"}"#);
        assert!(!t.props.contains_key("end") && t.props["status"] == "pending");
        // Due and recur make a recurring template.
        let (t, _) = one(r#"{"description":"r","due":"20261225T083000Z","recur":"weekly"}"#);
        assert_eq!(
            (t.props["status"].as_str(), t.props["rtype"].as_str()),
            ("recurring", "periodic")
        );
        // Odd states are kept or repaired with a warning.
        let (t, w) = one(r#"{"description":"r","recur":"weekly"}"#);
        assert!(!t.props.contains_key("recur") && w[0].starts_with("Warning: recurring task has no due date."));
        let (_, w) = one(r#"{"project":"P"}"#);
        assert!(w[0].starts_with("Warning: task has no description."));
        let (_, w) = one(r#"{"description":"d","entry":"20261002T000000Z","start":"20261001T000000Z"}"#);
        assert!(w[0].contains("'entry' date is after the 'start' date"), "{w:?}");
    }

    #[test]
    fn a_bad_task_is_named() {
        let c = clock();
        let cfg = Config::default();
        let bad = |json: &str| {
            let obj: Map<String, Value> = serde_json::from_str(json).unwrap();
            convert(&obj, &cfg, &c, &[], &mut Vec::new()).err().expect(json)
        };
        assert_eq!(bad("{}"), "Cannot import an empty task.");
        assert_eq!(bad(r#"{"uuid":"nope"}"#), "Not a valid UUID 'nope'.");
        assert_eq!(
            bad(r#"{"description":"x","status":"bogus"}"#),
            "The status 'bogus' is not valid."
        );
        assert!(bad(r#"{"description":"x","due":"garbage"}"#).contains("due: 'garbage' is not a valid date"));
        assert!(bad(r#"{"description":"x","wait":""}"#).contains("is not a valid date"));
        assert!(bad(r#"{"description":"x","annotations":"oops"}"#).starts_with("Annotations is malformed"));
        assert!(
            bad(r#"{"description":"x","annotations":[{"entry":"20261001T000000Z"}]}"#)
                .starts_with("Annotation is missing a description")
        );
        assert!(bad(r#"{"description":"x","depends":["not-a-uuid"]}"#).starts_with("Not a valid UUID"));
    }
}

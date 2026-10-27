//! A task as `task` hands it to a hook (its JSON: `20261027T120000Z` dates, `tags` as a list, UDAs beside the other
//! attributes) and as the engine's hooks see it (`tc_core::hooks`). The script is given exactly what the web app's
//! hooks are given; what it changed comes back as the engine's own `diff`, applied to the JSON `task` sent, so a field
//! the script cannot touch is left as `task` wrote it.

use serde_json::{Map, Value};
use tc_core::export::iso;
use tc_core::hooks::{diff, task_from_json, task_to_json};
use tc_core::model::{Facts, Note};
use tc_core::modify::Change;
use tc_core::taskrc::{Config, UdaType};
use uuid::Uuid;

/// Keys of a task that are not UDAs (and `id` and `urgency`, which `task` adds to what it sends).
const NOT_UDAS: &[&str] = &[
    "id",
    "urgency",
    "uuid",
    "status",
    "description",
    "entry",
    "modified",
    "start",
    "end",
    "due",
    "wait",
    "scheduled",
    "until",
    "recur",
    "parent",
    "mask",
    "imask",
    "rtype",
    "template",
    "last",
    "project",
    "priority",
    "depends",
    "tags",
    "annotations",
];

/// `20261027T120000Z` as seconds since the epoch.
fn epoch(text: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(text, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|d| d.and_utc().timestamp())
}

fn text<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

fn date(o: &Map<String, Value>, key: &str) -> Result<Option<i64>, String> {
    match text(o, key) {
        None => Ok(None),
        Some(t) => epoch(t)
            .map(Some)
            .ok_or_else(|| format!("task `{key}` is not a date: {t}")),
    }
}

fn uuid_of(s: &str) -> Result<Uuid, String> {
    Uuid::parse_str(s.trim()).map_err(|_| format!("`{s}` is not a task uuid"))
}

fn uda_type(cfg: &Config, name: &str) -> Option<UdaType> {
    cfg.udas.get(name).map(|d| d.ty)
}

/// The task `task` sent, as the engine sees it. `blocked` and `blocking` are not in what `task` sends and a hook
/// cannot ask `task` (it holds the data), so they read as false here.
pub fn facts_of(task: &Value, cfg: &Config) -> Result<Facts, String> {
    let o = task
        .as_object()
        .ok_or("a hook was given something that is not a task")?;
    let mut f = Facts {
        uuid: uuid_of(text(o, "uuid").ok_or("a hook was given a task with no uuid")?)?,
        status: text(o, "status").unwrap_or("pending").to_owned(),
        description: text(o, "description").unwrap_or_default().to_owned(),
        project: text(o, "project").map(str::to_owned),
        priority: text(o, "priority").map(str::to_owned),
        entry: date(o, "entry")?,
        modified: date(o, "modified")?,
        start: date(o, "start")?,
        end: date(o, "end")?,
        due: date(o, "due")?,
        wait: date(o, "wait")?,
        scheduled: date(o, "scheduled")?,
        until: date(o, "until")?,
        recur: text(o, "recur").map(str::to_owned),
        parent: text(o, "parent").map(uuid_of).transpose()?,
        mask: text(o, "mask").map(str::to_owned),
        imask: match o.get("imask") {
            Some(Value::Number(n)) => n.as_u64().map(|n| n as usize),
            Some(Value::String(s)) => s.parse().ok(),
            _ => None,
        },
        rtype: text(o, "rtype").map(str::to_owned),
        ..Facts::default()
    };
    if let Some(Value::Array(tags)) = o.get("tags") {
        f.tags = tags.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect();
    }
    if let Some(Value::Array(notes)) = o.get("annotations") {
        f.annotations = notes
            .iter()
            .map(|n| Note {
                entry: n.get("entry").and_then(Value::as_str).and_then(epoch).unwrap_or(0),
                text: n
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
            .collect();
    }
    f.depends = match o.get("depends") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(uuid_of)
            .collect::<Result<_, _>>()?,
        Some(Value::String(s)) => s
            .split(',')
            .filter(|p| !p.is_empty())
            .map(uuid_of)
            .collect::<Result<_, _>>()?,
        _ => Vec::new(),
    };
    for (k, v) in o.iter().filter(|(k, _)| !NOT_UDAS.contains(&k.as_str())) {
        let value = match (v, uda_type(cfg, k)) {
            (Value::String(s), Some(UdaType::Date)) => epoch(s).map_or_else(|| s.clone(), |e| e.to_string()),
            (Value::String(s), _) => s.clone(),
            (Value::Number(n), _) => n.to_string(),
            _ => continue,
        };
        f.extra.insert(k.clone(), value);
    }
    Ok(f)
}

/// The table the script is given for a task.
pub fn table_of(f: &Facts) -> Value {
    task_to_json(f)
}

/// What the script returned for a task, checked against `before` as the engine checks a hook's (it may not change a
/// task's uuid, status, annotations or recurrence), as the changes to make.
pub fn changes_of(before: &Facts, returned: &Value) -> Result<Vec<Change>, String> {
    diff(before, &task_from_json(returned, before)?)
}

fn set_or_remove(o: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    match value {
        Some(v) => {
            o.insert(key.to_owned(), v);
        }
        None => {
            o.remove(key);
        }
    }
}

/// A list kept as `task` writes it: absent, not empty, when nothing is in it.
fn edit_list(o: &mut Map<String, Value>, key: &str, edit: impl FnOnce(&mut Vec<Value>)) {
    let mut list = match o.remove(key) {
        Some(Value::Array(a)) => a,
        Some(Value::String(s)) => s.split(',').filter(|p| !p.is_empty()).map(Value::from).collect(),
        _ => Vec::new(),
    };
    edit(&mut list);
    if !list.is_empty() {
        o.insert(key.to_owned(), Value::Array(list));
    }
}

/// The task as `task` sent it with `changes` made, ready to be written back.
pub fn apply(mut task: Value, changes: &[Change], cfg: &Config) -> Value {
    let Some(o) = task.as_object_mut() else {
        return task;
    };
    for change in changes {
        match change {
            Change::Description(d) => {
                o.insert("description".into(), d.clone().into());
            }
            Change::Priority(p) => set_or_remove(o, "priority", p.clone().map(Value::from)),
            Change::Timestamp { name, value } => set_or_remove(o, name, value.map(|e| iso(e).into())),
            Change::Prop { name, value } => {
                let value = value.as_ref().map(|v| match uda_type(cfg, name) {
                    Some(UdaType::Date) => v.parse().map_or_else(|_| v.clone().into(), |e: i64| iso(e).into()),
                    Some(UdaType::Numeric) => v
                        .parse::<i64>()
                        .map(Value::from)
                        .or_else(|_| v.parse::<f64>().map(Value::from))
                        .unwrap_or_else(|_| v.clone().into()),
                    _ => v.clone().into(),
                });
                set_or_remove(o, name, value);
            }
            Change::AddTag(t) => edit_list(o, "tags", |l| {
                if !l.iter().any(|x| x.as_str() == Some(t)) {
                    l.push(t.clone().into());
                }
            }),
            Change::RemoveTag(t) => edit_list(o, "tags", |l| l.retain(|x| x.as_str() != Some(t))),
            Change::AddDep(u) => edit_list(o, "depends", |l| {
                let u = u.to_string();
                if !l.iter().any(|x| x.as_str() == Some(&u)) {
                    l.push(u.into());
                }
            }),
            Change::RemoveDep(u) => {
                let u = u.to_string();
                edit_list(o, "depends", |l| l.retain(|x| x.as_str() != Some(&u)));
            }
            // `diff` makes none of these.
            Change::SetTags(_) | Change::Status(_) | Change::ClearDeps | Change::Recurring => {}
        }
    }
    task
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg() -> Config {
        tc_core::taskrc::parse("uda.estimate.type=numeric\nuda.review.type=date\n").config
    }

    fn sent() -> Value {
        json!({
            "id": 3,
            "uuid": "11111111-1111-1111-1111-111111111111",
            "status": "pending",
            "description": "Pay rent",
            "entry": "20261027T100000Z",
            "modified": "20261027T100000Z",
            "due": "20261101T000000Z",
            "project": "Home",
            "tags": ["money"],
            "depends": ["22222222-2222-2222-2222-222222222222"],
            "estimate": 4,
            "review": "20261105T000000Z",
            "urgency": 9.1,
            "annotations": [{ "entry": "20261027T101500Z", "description": "landlord" }]
        })
    }

    #[test]
    fn the_task_task_sends_is_what_a_script_reads() {
        let f = facts_of(&sent(), &cfg()).unwrap();
        assert_eq!(f.description, "Pay rent");
        assert_eq!(f.due, Some(1_793_491_200));
        assert!(f.tags.contains("money"));
        assert_eq!(f.annotations[0].text, "landlord");
        assert_eq!(f.depends.len(), 1);
        assert_eq!(f.extra["estimate"], "4");
        assert_eq!(
            f.extra["review"], "1793836800",
            "a date UDA is seconds, as in the engine"
        );
        assert!(!f.extra.contains_key("urgency") && !f.extra.contains_key("id"));
    }

    #[test]
    fn nothing_changed_means_the_task_goes_back_as_it_came() {
        let f = facts_of(&sent(), &cfg()).unwrap();
        let changes = changes_of(&f, &table_of(&f)).unwrap();
        assert!(changes.is_empty());
        assert_eq!(apply(sent(), &changes, &cfg()), sent());
    }

    #[test]
    fn what_a_script_changed_is_written_in_task_s_form() {
        let f = facts_of(&sent(), &cfg()).unwrap();
        let mut t = table_of(&f);
        t["description"] = json!("Pay rent today");
        t["tags"] = json!(["money", "office"]);
        t["due"] = json!(1_793_577_600);
        t["uda"] = json!({ "estimate": "5", "review": "1793923200", "note": "x" });
        t["priority"] = json!("H");
        t["project"] = Value::Null;
        let out = apply(sent(), &changes_of(&f, &t).unwrap(), &cfg());
        assert_eq!(out["description"], "Pay rent today");
        assert_eq!(out["tags"], json!(["money", "office"]));
        assert_eq!(out["due"], "20261102T000000Z");
        assert_eq!(out["estimate"], 5);
        assert_eq!(out["review"], "20261106T000000Z");
        assert_eq!(out["note"], "x");
        assert_eq!(out["priority"], "H");
        assert!(out.get("project").is_none());
        assert_eq!(out["urgency"], 9.1, "what the script cannot see is left alone");
    }

    #[test]
    fn removing_the_last_tag_or_dependency_leaves_no_empty_list() {
        let f = facts_of(&sent(), &cfg()).unwrap();
        let mut t = table_of(&f);
        t["tags"] = json!([]);
        t["depends"] = json!([]);
        let out = apply(sent(), &changes_of(&f, &t).unwrap(), &cfg());
        assert!(out.get("tags").is_none() && out.get("depends").is_none());
    }

    #[test]
    fn a_script_may_not_change_what_a_hook_may_not() {
        let f = facts_of(&sent(), &cfg()).unwrap();
        let mut t = table_of(&f);
        t["status"] = json!("completed");
        assert!(changes_of(&f, &t).unwrap_err().contains("status"));
    }
}

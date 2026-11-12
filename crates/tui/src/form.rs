//! The full task form, ported from the web app's `TaskEditor.svelte`: a field for each part of a task (description,
//! project, priority, the dates, repeat, tags, your UDAs, what it depends on), and for a new one whether to start it
//! and a note. Adding begins as the one line; the form opens from it with the line broken into its fields. Modifying is
//! always the form. Either way only what changed is sent, as `add` or `modify` arguments, then `start` and `annotate`.

use crate::completer::{Completer, Input};
use crate::model;
use serde_json::Value;
use tc_core::preview::Preview;
use tc_core::taskrc::{Config, UdaType};

/// What a field holds, which decides how it is typed, completed and sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One value, sent as `name:value`.
    Value,
    /// A date, sent as `name:value` and shown as `task` will read it.
    Date,
    /// Words, each a tag: `+tag` for one added, `-tag` for one taken off.
    Tags,
    /// Task ids or uuids: `depends:x` for one added, `depends:-x` for one taken off.
    Depends,
    /// Sent apart from the rest: `annotate` after the task is made or changed.
    Note,
    /// A yes or no: start the new task.
    Start,
}

#[derive(Debug, Clone)]
pub struct Field {
    /// The attribute (`project`, `due`, a UDA's name), or `tags`, `depends`, `note`, `start`.
    pub key: String,
    pub label: String,
    pub kind: Kind,
    pub input: Input,
    /// What it held when the form opened: what a change is measured against.
    pub start: String,
    /// Shown but not changed here (the period of one task of a series).
    pub locked: bool,
}

impl Field {
    fn new(key: &str, label: &str, kind: Kind, start: &str, value: &str) -> Field {
        Field {
            key: key.to_owned(),
            label: label.to_owned(),
            kind,
            input: Input::new(value),
            start: start.to_owned(),
            locked: false,
        }
    }

    pub fn value(&self) -> &str {
        self.input.line.trim()
    }

    /// For completion: what goes in front of the word being typed (`project:`, `+`), and whether only the last word
    /// of the field is completed.
    pub fn completes_as(&self) -> Option<(String, bool)> {
        match self.kind {
            Kind::Value | Kind::Date if self.key != "description" => Some((format!("{}:", self.key), false)),
            Kind::Tags => Some(("+".into(), true)),
            Kind::Depends => Some(("depends:".into(), true)),
            _ => None,
        }
    }
}

/// The form, for a new task (`uuid` none) or one that exists.
#[derive(Debug)]
pub struct Form {
    pub uuid: Option<String>,
    /// The task's id, for the title.
    pub id: Option<u64>,
    pub fields: Vec<Field>,
    pub sel: usize,
    pub completer: Completer,
    /// Attributes the form has no field for (`entry:…` typed on the line), sent as they are.
    pub carried: Vec<String>,
    /// The task's annotations, shown under the form.
    pub notes: Vec<String>,
}

const DATES: [&str; 4] = ["due", "wait", "scheduled", "until"];

/// A moment as the form holds it: the local date, and the time when it is not midnight (`task` reads both back).
pub fn date_text(ts: i64) -> String {
    let local = ts + i64::from(model::local_offset(ts));
    let Some(d) = chrono::DateTime::from_timestamp(local, 0).map(|d| d.naive_utc()) else {
        return String::new();
    };
    if d.format("%H:%M:%S").to_string() == "00:00:00" {
        d.format("%Y-%m-%d").to_string()
    } else {
        d.format("%Y-%m-%dT%H:%M").to_string()
    }
}

/// The UDAs the form has a field for: all but `priority`, which has its own.
fn udas(cfg: &Config) -> Vec<(String, String, UdaType)> {
    let mut out: Vec<_> = cfg
        .udas
        .values()
        .filter(|u| u.name != "priority")
        .map(|u| (u.name.clone(), u.label.clone().unwrap_or_else(|| u.name.clone()), u.ty))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn uda_kind(ty: UdaType) -> Kind {
    if ty == UdaType::Date {
        Kind::Date
    } else {
        Kind::Value
    }
}

impl Form {
    /// A new task, from what the one line says (a preview of it), or empty.
    pub fn add(p: &Preview, cfg: &Config) -> Form {
        let date = |k: &str| p.dates.get(k).map(|ts| date_text(*ts)).unwrap_or_default();
        let blank = |key: &str, label: &str, kind: Kind, value: &str| Field::new(key, label, kind, "", value);
        let mut fields = vec![
            blank("description", "Description", Kind::Value, &p.description),
            blank("project", "Project", Kind::Value, p.project.as_deref().unwrap_or("")),
            blank("priority", "Priority", Kind::Value, p.priority.as_deref().unwrap_or("")),
        ];
        for k in DATES {
            fields.push(blank(k, &title(k), Kind::Date, &date(k)));
        }
        fields.push(blank("recur", "Repeat", Kind::Value, p.recur.as_deref().unwrap_or("")));
        fields.push(blank("tags", "Tags", Kind::Tags, &p.tags.join(" ")));
        for (name, label, ty) in udas(cfg) {
            let raw = p.udas.get(&name).cloned().unwrap_or_default();
            let value = match (ty, raw.parse::<i64>()) {
                (UdaType::Date, Ok(ts)) => date_text(ts),
                _ => raw,
            };
            fields.push(blank(&name, &label, uda_kind(ty), &value));
        }
        fields.push(blank("depends", "Depends on", Kind::Depends, &p.depends.join(" ")));
        fields.push(blank("start", "Start", Kind::Start, ""));
        fields.push(blank("note", "Note", Kind::Note, ""));
        let carried = p
            .dates
            .iter()
            .filter(|(k, _)| !DATES.contains(&k.as_str()))
            .map(|(k, ts)| format!("{k}:{}", date_text(*ts)))
            .collect();
        Form {
            uuid: None,
            id: None,
            fields,
            sel: 0,
            completer: Completer::default(),
            carried,
            notes: Vec::new(),
        }
    }

    /// A task as `export` wrote it; `pending` names what it depends on by id where it can.
    pub fn modify(t: &Value, cfg: &Config, pending: &[Value]) -> Form {
        let s = |k: &str| t.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let date = |k: &str| {
            t.get(k)
                .and_then(Value::as_str)
                .and_then(model::parse_date)
                .map(date_text)
                .unwrap_or_default()
        };
        let same = |key: &str, label: &str, kind: Kind, v: &str| Field::new(key, label, kind, v, v);
        let mut fields = vec![
            same("description", "Description", Kind::Value, &s("description")),
            same("project", "Project", Kind::Value, &s("project")),
            same("priority", "Priority", Kind::Value, &s("priority")),
        ];
        for k in DATES {
            fields.push(same(k, &title(k), Kind::Date, &date(k)));
        }
        let mut recur = same("recur", "Repeat", Kind::Value, &s("recur"));
        // One task of a series follows its template: the period is changed there.
        recur.locked = t.get("parent").is_some();
        fields.push(recur);
        let tags: Vec<&str> = t
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        fields.push(same("tags", "Tags", Kind::Tags, &tags.join(" ")));
        for (name, label, ty) in udas(cfg) {
            let value = match t.get(&name) {
                Some(Value::String(v)) if ty == UdaType::Date => {
                    model::parse_date(v).map(date_text).unwrap_or_default()
                }
                Some(Value::String(v)) => v.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
            fields.push(same(&name, &label, uda_kind(ty), &value));
        }
        let depends: Vec<String> = t
            .get("depends")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|u| {
                pending
                    .iter()
                    .find(|p| p.get("uuid").and_then(Value::as_str) == Some(u))
                    .and_then(|p| p.get("id").and_then(Value::as_u64))
                    .filter(|&i| i > 0)
                    .map_or_else(|| u.to_owned(), |i| i.to_string())
            })
            .collect();
        fields.push(same("depends", "Depends on", Kind::Depends, &depends.join(" ")));
        fields.push(same("note", "Annotate", Kind::Note, ""));
        let notes = t
            .get("annotations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|a| a.get("description").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        Form {
            uuid: Some(s("uuid")),
            id: t.get("id").and_then(Value::as_u64).filter(|&i| i > 0),
            fields,
            sel: 0,
            completer: Completer::default(),
            carried: Vec::new(),
            notes,
        }
    }

    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.key == key)
    }

    fn value(&self, key: &str) -> &str {
        self.field(key).map_or("", Field::value)
    }

    pub fn start_now(&self) -> bool {
        self.value("start") == "yes"
    }

    /// The `add` or `modify` arguments for what changed (the web form's `changes`).
    pub fn changes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in self.fields.iter().filter(|f| !f.locked) {
            let now = f.value();
            match f.kind {
                Kind::Value | Kind::Date if now != f.start.trim() => out.push(format!("{}:{now}", f.key)),
                Kind::Tags | Kind::Depends => {
                    let words = |s: &str| -> Vec<String> {
                        s.split([' ', ','])
                            .filter(|w| !w.is_empty())
                            .map(|w| w.trim_start_matches('+').to_owned())
                            .collect()
                    };
                    let (want, had) = (words(now), words(&f.start));
                    let (on, off) = if f.kind == Kind::Tags {
                        ("+".to_owned(), "-".to_owned())
                    } else {
                        ("depends:".to_owned(), "depends:-".to_owned())
                    };
                    out.extend(want.iter().filter(|w| !had.contains(w)).map(|w| format!("{on}{w}")));
                    out.extend(had.iter().filter(|w| !want.contains(w)).map(|w| format!("{off}{w}")));
                }
                _ => {}
            }
        }
        out.extend(self.carried.iter().cloned());
        out
    }

    /// Why the form can't be saved as it is, or `None`.
    pub fn problem(&self) -> Option<&'static str> {
        if self.uuid.is_none() && self.value("description").is_empty() {
            return Some("A task needs a description.");
        }
        let recur = self.field("recur");
        if recur.is_some_and(|r| !r.locked && !r.value().is_empty()) && self.value("due").is_empty() {
            return Some("A repeating task needs a due date.");
        }
        None
    }

    /// The `task` command lines that save it, each as its words, run one after the other while each succeeds. None
    /// when nothing changed.
    pub fn statements(&self) -> Vec<Vec<String>> {
        let words = |list: &[&str]| list.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        let note = self.value("note");
        let mut out = Vec::new();
        match &self.uuid {
            None => {
                out.push([words(&["add"]), self.changes()].concat());
                if self.start_now() {
                    out.push(words(&["+LATEST", "start"]));
                }
                if !note.is_empty() {
                    out.push(words(&["+LATEST", "annotate", note]));
                }
            }
            Some(u) => {
                let changes = self.changes();
                if !changes.is_empty() {
                    out.push([words(&[u, "modify"]), changes].concat());
                }
                if !note.is_empty() {
                    out.push(words(&[u, "annotate", note]));
                }
            }
        }
        out
    }

    /// The command lines, as the box shows them.
    pub fn command(&self) -> String {
        self.statements()
            .iter()
            .map(|s| s.iter().map(|w| crate::form::quote(w)).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(" && ")
    }

    pub fn next(&mut self) {
        self.sel = (self.sel + 1) % self.fields.len().max(1);
        self.completer.close();
    }

    pub fn prev(&mut self) {
        let n = self.fields.len().max(1);
        self.sel = (self.sel + n - 1) % n;
        self.completer.close();
    }

    pub fn current(&mut self) -> Option<&mut Field> {
        self.fields.get_mut(self.sel)
    }
}

fn title(key: &str) -> String {
    let mut c = key.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// A word as a command line needs it to come back the same: quoted when it has spaces or quotes in it.
pub fn quote(w: &str) -> String {
    if !w.is_empty() && !w.chars().any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | '\\')) {
        return w.to_owned();
    }
    if !w.contains('\'') {
        return format!("'{w}'");
    }
    if !w.contains('"') {
        return format!("\"{w}\"");
    }
    w.chars()
        .flat_map(|c| {
            let escape = c == ' ' || matches!(c, '\'' | '"' | '\\');
            escape.then_some('\\').into_iter().chain([c])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tc_core::filter::split_words;

    fn cfg() -> Config {
        tc_core::taskrc::parse("uda.estimate.type=numeric\nuda.estimate.label=Estimate\n").config
    }

    fn set(f: &mut Form, key: &str, value: &str) {
        let field = f.fields.iter_mut().find(|x| x.key == key).unwrap();
        field.input = Input::new(value);
    }

    #[test]
    fn a_new_task_starts_from_the_line_and_sends_what_is_filled_in() {
        let p = Preview {
            description: "Fix bike".into(),
            project: Some("Home".into()),
            tags: vec!["errand".into()],
            ..Preview::default()
        };
        let mut f = Form::add(&p, &cfg());
        assert_eq!(f.field("description").unwrap().value(), "Fix bike");
        assert!(f.field("estimate").is_some(), "a field for each UDA");
        set(&mut f, "estimate", "2");
        set(&mut f, "start", "yes");
        set(&mut f, "note", "the back wheel");
        assert_eq!(
            f.statements(),
            vec![
                vec!["add", "description:Fix bike", "project:Home", "+errand", "estimate:2"],
                vec!["+LATEST", "start"],
                vec!["+LATEST", "annotate", "the back wheel"],
            ]
        );
        assert_eq!(f.problem(), None);
        set(&mut f, "description", " ");
        assert_eq!(f.problem(), Some("A task needs a description."));
    }

    #[test]
    fn a_change_sends_only_what_changed() {
        let t = json!({
            "uuid": "u-1", "id": 3, "description": "Water plants", "project": "Home",
            "tags": ["soon", "home"], "recur": "weekly", "parent": "p-1",
            "due": "20261113T000000Z", "depends": ["d-1", "d-2"],
            "annotations": [{ "description": "rain" }]
        });
        let pending = vec![json!({ "uuid": "d-1", "id": 7 })];
        let mut f = Form::modify(&t, &cfg(), &pending);
        assert_eq!(f.id, Some(3));
        assert_eq!(f.notes, vec!["rain"]);
        assert_eq!(f.field("depends").unwrap().value(), "7 d-2");
        assert!(f.field("recur").unwrap().locked, "a task of a series keeps its period");
        assert!(f.statements().is_empty(), "nothing changed");
        set(&mut f, "project", "");
        set(&mut f, "tags", "home +garden");
        set(&mut f, "depends", "7 9");
        set(&mut f, "recur", "");
        assert_eq!(
            f.statements(),
            vec![vec![
                "u-1",
                "modify",
                "project:",
                "+garden",
                "-soon",
                "depends:9",
                "depends:-d-2"
            ]]
        );
        set(&mut f, "note", "moved");
        assert_eq!(f.statements().last().unwrap(), &vec!["u-1", "annotate", "moved"]);
    }

    #[test]
    fn a_repeating_task_needs_a_due_date() {
        let mut f = Form::add(&Preview::default(), &cfg());
        set(&mut f, "description", "Pay rent");
        set(&mut f, "recur", "monthly");
        assert_eq!(f.problem(), Some("A repeating task needs a due date."));
        set(&mut f, "due", "eom");
        assert_eq!(f.problem(), None);
    }

    #[test]
    fn quoted_words_split_back_the_same() {
        for w in ["plain", "two words", "it's", "say \"hi\"", "both ' and \" here", ""] {
            assert_eq!(split_words(&quote(w)), vec![w.to_owned()], "{w:?}");
        }
    }
}

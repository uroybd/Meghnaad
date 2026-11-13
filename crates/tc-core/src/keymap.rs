//! Keybindings, for every front end. An action has a dotted name grouped by where it acts (`app.prompt`,
//! `tasks.done`, `settings.commands.add`). Which actions a front end has, their default keys and the contexts its screens
//! put keys in are an asset each front end carries, as the colour themes are: `web/public/keymaps/default.json` (the web
//! app fetches it; the terminal app and the phone package it). It has the shape of `settings.json`, where the user's own
//! keys go:
//!
//! ```json
//! { "keys":   { "app.prompt": "/", "app.undo": null },
//!   "tui":    { "keys": { "tasks.filter": ["f", "ctrl+f"] } },
//!   "web":    { "keys": {} },
//!   "mobile": { "keys": {} } }
//! ```
//!
//! A front end has the actions its defaults name (in `keys` or its own `<front end>.keys`), and its defaults' `contexts`:
//! each a list of groups whose keys act at once, the most particular first. Then `keys` of `settings.json` (for every
//! front end) and the front end's own: a later one replaces an action's keys, and `null` or `[]` leaves it without any.
//! A key in a more particular group comes before the same key of a group around it (a key of `tasks` before the same key
//! of `app`, on that page only), as Zed's contexts do. A key the user gives one action is taken from an action that has it
//! by default and would otherwise come first, so rebinding needs no unbinding. What is wrong (a name that is no action, a
//! key that cannot be read, one key for two actions of one group) is said, and the rest still applies.
//!
//! A key is written as Zed and VS Code write it: modifiers then the key, with `+` or `-` (`ctrl+k`, `ctrl-shift-tab`),
//! the key a character (`G`, `?`, `/`) or a name (`enter`, `esc`, `space`, `tab`, `up`, `pagedown`, `f5`). A front end
//! turns a key it is given into the same form with [`from_event`], and looks the action up in the context it is in
//! ([`Keymap::action`]). Typing in a field, and a menu's or a dialog's own keys, are the platform's and not here.
//!
//! The asset's `about` says in a line what each action does, so every front end's Keybindings page can list and search
//! the actions of the others. That page is [`bindings`] (each section of `settings.json` that holds keys: what the user
//! set there and every action it can name, with its default keys) and [`with_binding`] (one action's keys set or taken
//! out).

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The front ends, as `settings.json` names their own sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Tui,
    Web,
    Mobile,
}

impl Platform {
    pub const ALL: [Platform; 3] = [Platform::Tui, Platform::Web, Platform::Mobile];

    /// Its section in `settings.json`.
    pub fn key(self) -> &'static str {
        match self {
            Platform::Tui => "tui",
            Platform::Web => "web",
            Platform::Mobile => "mobile",
        }
    }

    pub fn parse(s: &str) -> Option<Platform> {
        Platform::ALL.into_iter().find(|p| p.key() == s)
    }
}

/// The group an action is in: its name up to the last dot (`settings.commands` for `settings.commands.add`).
pub fn section_of(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(s, _)| s)
}

/// The keys of every action a front end has, the groups each of its contexts puts keys in, and what was wrong.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Keymap {
    /// Each action the front end has and its keys, as [`from_event`] writes them (none for an action left without).
    pub keys: BTreeMap<String, Vec<String>>,
    /// Each context's keys and the action each runs.
    pub contexts: BTreeMap<String, BTreeMap<String, String>>,
    pub problems: Vec<String>,
}

impl Keymap {
    /// The action `chord` runs in `context`, if any.
    pub fn action(&self, context: &str, chord: &str) -> Option<&str> {
        self.contexts.get(context)?.get(chord).map(String::as_str)
    }

    /// An action's keys (none when it has none, or no such action).
    pub fn keys_of(&self, action: &str) -> &[String] {
        self.keys.get(action).map_or(&[], Vec::as_slice)
    }

    /// An action's keys as a hint shows them (`e/↵`, or `/ :` where one of them is `/`); empty when it has none.
    pub fn label(&self, action: &str) -> String {
        let keys = self.keys_of(action);
        let between = if keys.iter().any(|k| k == "/") { " " } else { "/" };
        keys.iter().map(|k| display(k)).collect::<Vec<_>>().join(between)
    }
}

/// `<place>.keys` of a settings-shaped object (`keys` itself for none): its actions and their keys, read; `None` where it
/// says nothing.
fn layer(value: Option<&Value>, place: &str, problems: &mut Vec<String>) -> Option<Vec<(String, Vec<String>)>> {
    let value = value.filter(|v| !v.is_null())?;
    let Some(map) = value.as_object() else {
        problems.push(format!("{place}: not an object of actions and their keys"));
        return None;
    };
    let mut out = Vec::new();
    for (name, v) in map {
        let texts: Vec<&Value> = match v {
            Value::Null => Vec::new(),
            Value::Array(list) => list.iter().collect(),
            other => vec![other],
        };
        let mut list: Vec<String> = Vec::new();
        for t in texts {
            match t.as_str().map(parse) {
                Some(Ok(k)) if !list.contains(&k) => list.push(k),
                Some(Ok(_)) => {}
                Some(Err(e)) => problems.push(format!("{place}.{name}: {e}")),
                None => problems.push(format!("{place}.{name}: {t} is not a key")),
            }
        }
        out.push((name.clone(), list));
    }
    Some(out)
}

fn own_keys(v: &Value, platform: Platform) -> Option<&Value> {
    v.get(platform.key()).and_then(|s| s.get("keys"))
}

/// The keys of `platform`: its `defaults` (the keymap asset), then `keys` and `<platform>.keys` of `settings`.
pub fn resolve(defaults: &Value, settings: &Value, platform: Platform) -> Keymap {
    let mut problems = Vec::new();
    // What the defaults say is the asset's to get right (its tests check it); still said, never a failure.
    let mut quiet = Vec::new();
    let mut keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, list) in layer(defaults.get("keys"), "default keys", &mut quiet)
        .into_iter()
        .chain(layer(own_keys(defaults, platform), "default keys", &mut quiet))
        .flatten()
    {
        keys.insert(name, list);
    }
    // Every action of any front end, so a name for another one's is not taken for a mistake.
    let mut known: BTreeSet<String> = BTreeSet::new();
    for v in std::iter::once(defaults.get("keys")).chain(Platform::ALL.map(|p| own_keys(defaults, p))) {
        known.extend(v.and_then(Value::as_object).into_iter().flat_map(Map::keys).cloned());
    }

    let mut set_by_user: Vec<String> = Vec::new();
    let place = format!("{}.keys", platform.key());
    for (own, at, value) in [
        (false, "keys", settings.get("keys")),
        (true, place.as_str(), own_keys(settings, platform)),
    ] {
        for (name, list) in layer(value, at, &mut problems).into_iter().flatten() {
            if !keys.contains_key(&name) {
                if !known.contains(&name) {
                    problems.push(format!("{at}: “{name}” is no action"));
                } else if own {
                    // Shared `keys` may name actions of another front end; a front end's own may not.
                    problems.push(format!("{at}: the {} app has no “{name}”", platform.key()));
                }
                continue;
            }
            keys.insert(name.clone(), list);
            if !set_by_user.contains(&name) {
                set_by_user.push(name);
            }
        }
    }

    let contexts: Vec<(String, Vec<String>)> = defaults
        .get(platform.key())
        .and_then(|p| p.get("contexts"))
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, groups)| {
            let groups = groups.as_array().into_iter().flatten().filter_map(Value::as_str);
            (name.clone(), groups.map(str::to_owned).collect())
        })
        .collect();

    // A key the user gave one action leaves any action that has it by default and would come before it (its own group,
    // or a more particular one where both act), so rebinding needs no unbinding.
    let shadows = |default: &str, user: &str| {
        let (d, u) = (section_of(default), section_of(user));
        contexts.iter().any(|(_, groups)| {
            let at = |g: &str| groups.iter().position(|x| x == g);
            matches!((at(d), at(u)), (Some(i), Some(j)) if i <= j)
        })
    };
    let taken: Vec<(String, String)> = set_by_user
        .iter()
        .flat_map(|a| keys.get(a).into_iter().flatten().map(move |k| (a.clone(), k.clone())))
        .collect();
    for (name, list) in &mut keys {
        if !set_by_user.contains(name) {
            list.retain(|k| !taken.iter().any(|(a, t)| t == k && shadows(name, a)));
        }
    }

    // Two actions of one group with one key clash; the first by name is the one it runs.
    let mut seen: BTreeMap<(&str, &str), &str> = BTreeMap::new();
    for (name, list) in &keys {
        for k in list {
            match seen.get(&(section_of(name), k.as_str())) {
                Some(first) => {
                    problems.push(format!(
                        "keys: {} is both {first} and {name}; it runs {first}",
                        display(k)
                    ));
                }
                None => {
                    seen.insert((section_of(name), k.as_str()), name);
                }
            }
        }
    }

    // Each context's keys: a more particular group's key comes before the same key of a group around it.
    let mut by_context = BTreeMap::new();
    for (context, groups) in &contexts {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for group in groups {
            for (name, list) in keys.iter().filter(|(n, _)| section_of(n) == group) {
                for k in list {
                    map.entry(k.clone()).or_insert_with(|| name.clone());
                }
            }
        }
        by_context.insert(context.clone(), map);
    }

    Keymap {
        keys,
        contexts: by_context,
        problems,
    }
}

const MODIFIERS: [&str; 4] = ["ctrl", "alt", "shift", "meta"];

/// A modifier by any of its names.
fn modifier(word: &str) -> Option<&'static str> {
    Some(match word.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => "ctrl",
        "alt" | "option" | "opt" => "alt",
        "shift" => "shift",
        "meta" | "cmd" | "command" | "super" | "win" => "meta",
        _ => return None,
    })
}

/// A key's name, by any of its names (the web's `ArrowDown` and `Escape` too).
fn named(word: &str) -> Option<String> {
    let lower = word.to_ascii_lowercase();
    let name = match lower.as_str() {
        "enter" | "return" => "enter",
        "esc" | "escape" => "esc",
        "tab" => "tab",
        "space" | "spacebar" => "space",
        "backspace" => "backspace",
        "delete" | "del" => "delete",
        "insert" | "ins" => "insert",
        "up" | "arrowup" => "up",
        "down" | "arrowdown" => "down",
        "left" | "arrowleft" => "left",
        "right" | "arrowright" => "right",
        "home" => "home",
        "end" => "end",
        "pageup" | "pgup" => "pageup",
        "pagedown" | "pgdn" => "pagedown",
        f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => {
            return Some(f.to_owned());
        }
        _ => return None,
    };
    Some(name.to_owned())
}

/// The key as every front end writes it: the modifiers in one order, then the key. A character carries its own shift
/// (`G`, `?`), except a letter with Ctrl, Alt or Meta, which is written small with `shift` (`ctrl+shift+k`).
fn canonical(mods: &[&str], key: &str) -> Option<String> {
    let held = |m: &str| mods.contains(&m);
    let mut shift = held("shift");
    let mut chars = key.chars();
    let key = match (chars.next(), chars.next()) {
        (Some(' '), None) => "space".to_owned(),
        (Some(c), None) => {
            let other = held("ctrl") || held("alt") || held("meta");
            if other && c.is_alphabetic() {
                shift = shift || c.is_uppercase();
                c.to_lowercase().collect()
            } else {
                // `?` is shift and `/` on one keyboard and not on another: the character says which it is.
                let upper = shift && c.is_alphabetic();
                shift = false;
                if upper {
                    c.to_uppercase().collect()
                } else {
                    c.to_string()
                }
            }
        }
        _ => named(key)?,
    };
    let mut out = String::new();
    for m in MODIFIERS {
        if (m == "shift" && shift) || (m != "shift" && held(m)) {
            out.push_str(m);
            out.push('+');
        }
    }
    out.push_str(&key);
    Some(out)
}

/// A key as the user wrote it in `settings.json`, as [`from_event`] writes it.
///
/// # Errors
///
/// It is empty, or names a key or a modifier there is none of.
pub fn parse(text: &str) -> Result<String, String> {
    let bad = || format!("“{text}” is not a key");
    if text.is_empty() {
        return Err("an empty key".into());
    }
    let mut mods = Vec::new();
    let mut rest = text;
    // Strip `mod+` or `mod-` while what is left is more than one character (so `ctrl++` and `-` are keys).
    while rest.chars().count() > 1 {
        let Some(at) = rest.find(['+', '-']) else { break };
        let Some(m) = modifier(&rest[..at]) else { break };
        mods.push(m);
        rest = &rest[at + 1..];
    }
    if rest.is_empty() {
        return Err(bad());
    }
    let key = if rest.chars().count() == 1 {
        rest.to_owned()
    } else {
        named(rest).ok_or_else(bad)?
    };
    canonical(&mods, &key).ok_or_else(bad)
}

/// A key a front end was given: the key (a character, or a name such as the web's `ArrowDown` or the terminal's `Enter`)
/// and the modifiers held (`ctrl`, `alt`, `shift`, `meta`), as [`parse`] writes the same key. `None` for a key there is
/// no name for here (a modifier alone, a media key).
pub fn from_event(key: &str, mods: &[&str]) -> Option<String> {
    let mods: Vec<&str> = mods.iter().filter_map(|m| modifier(m)).collect();
    canonical(&mods, key)
}

/// A key as a hint shows it: `↵`, `↓`, `Space`, `Ctrl+K`, `G`.
pub fn display(chord: &str) -> String {
    let (mods, key) = match chord.rsplit_once('+') {
        Some((m, "")) => (m.strip_suffix('+').unwrap_or(m), "+"),
        Some((m, k)) => (m, k),
        None => ("", chord),
    };
    let key = match key {
        "enter" => "↵".to_owned(),
        "up" => "↑".to_owned(),
        "down" => "↓".to_owned(),
        "left" => "←".to_owned(),
        "right" => "→".to_owned(),
        "esc" => "Esc".to_owned(),
        "tab" => "Tab".to_owned(),
        "space" => "Space".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" => "Del".to_owned(),
        "insert" => "Ins".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "pageup" => "PgUp".to_owned(),
        "pagedown" => "PgDn".to_owned(),
        k if k.len() > 1 && k.starts_with('f') => k.to_ascii_uppercase(),
        k if mods.is_empty() => k.to_owned(),
        k => k.to_uppercase(),
    };
    let mut out: Vec<String> = mods
        .split('+')
        .filter(|m| !m.is_empty())
        .map(|m| match m {
            "ctrl" => "Ctrl".to_owned(),
            "alt" => "Alt".to_owned(),
            "shift" => "Shift".to_owned(),
            "meta" => "Meta".to_owned(),
            other => other.to_owned(),
        })
        .collect();
    out.push(key);
    out.join("+")
}

/// The sections of `settings.json` a person sets keys in, as a Keybindings page lists them: `general` is `keys`, for
/// every front end, and each front end's is its own `<front end>.keys`.
pub const SECTIONS: [&str; 4] = ["general", "web", "mobile", "tui"];

/// One action of a section, for a Keybindings page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub action: String,
    /// What it does: the asset's `about`, empty for a name that is no action.
    pub about: String,
    /// Its keys by default (for `general`, the shared defaults, else those of every front end that has it).
    pub defaults: Vec<String>,
    /// The keys the user set in this section (`[]` for none); `None` where they set nothing.
    pub keys: Option<Vec<String>>,
}

/// One section of a Keybindings page: every action it can name, those the user set first, each part by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingSection {
    pub id: &'static str,
    pub actions: Vec<Binding>,
}

/// What a Keybindings page shows: each section's actions, and what is wrong with the user's keys on any front end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bindings {
    pub sections: Vec<BindingSection>,
    pub problems: Vec<String>,
}

/// A layer of keys, read: each action and its keys.
type Layer = Vec<(String, Vec<String>)>;

/// The object a section's keys are in: `keys`, or `<front end>.keys`.
fn section_keys<'a>(v: &'a Value, section: &str) -> Option<&'a Value> {
    match Platform::parse(section) {
        Some(p) => own_keys(v, p),
        None => v.get("keys"),
    }
}

/// Every section of a Keybindings page, from the keymap asset and the whole of `settings.json`.
pub fn bindings(defaults: &Value, settings: &Value) -> Bindings {
    let about = |name: &str| {
        defaults
            .get("about")
            .and_then(|a| a.get(name))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut quiet = Vec::new();
    let read = |v: Option<&Value>, quiet: &mut Vec<String>| -> Layer { layer(v, "", quiet).unwrap_or_default() };
    let shared = read(defaults.get("keys"), &mut quiet);
    let own: Vec<(Platform, Layer)> = Platform::ALL
        .into_iter()
        .map(|p| (p, read(own_keys(defaults, p), &mut quiet)))
        .collect();
    let mut sections = Vec::new();
    for id in SECTIONS {
        // What the section can name, with its default keys.
        let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let platform = Platform::parse(id);
        for (p, list) in &own {
            if platform.is_none_or(|x| x == *p) {
                for (name, keys) in list {
                    let at = names.entry(name.clone()).or_default();
                    at.extend(keys.iter().filter(|k| !at.contains(k)).cloned().collect::<Vec<_>>());
                }
            }
        }
        for (name, keys) in &shared {
            // The shared defaults are every front end's, unless its own replace them.
            let replaced =
                platform.is_some_and(|p| own.iter().any(|(q, l)| *q == p && l.iter().any(|(n, _)| n == name)));
            if !replaced {
                names.insert(name.clone(), keys.clone());
            }
        }
        // The user's, as written; a key that cannot be read is kept as it is so it can still be seen and taken out.
        let mut set: Layer = Vec::new();
        if let Some(map) = section_keys(settings, id).and_then(Value::as_object) {
            for (name, v) in map {
                let texts: Vec<&Value> = match v {
                    Value::Null => Vec::new(),
                    Value::Array(list) => list.iter().collect(),
                    other => vec![other],
                };
                let list = texts
                    .into_iter()
                    .map(|t| {
                        t.as_str()
                            .map_or_else(|| t.to_string(), |s| parse(s).unwrap_or_else(|_| s.to_owned()))
                    })
                    .collect();
                set.push((name.clone(), list));
            }
        }
        let mut actions: Vec<Binding> = set
            .iter()
            .map(|(name, keys)| Binding {
                action: name.clone(),
                about: about(name),
                defaults: names.get(name).cloned().unwrap_or_default(),
                keys: Some(keys.clone()),
            })
            .collect();
        actions.extend(
            names
                .into_iter()
                .filter(|(n, _)| !set.iter().any(|(s, _)| s == n))
                .map(|(name, defaults)| Binding {
                    about: about(&name),
                    action: name,
                    defaults,
                    keys: None,
                }),
        );
        sections.push(BindingSection { id, actions });
    }
    let mut problems: Vec<String> = Vec::new();
    for p in Platform::ALL {
        for line in resolve(defaults, settings, p).problems {
            if !problems.contains(&line) {
                problems.push(line);
            }
        }
    }
    Bindings { sections, problems }
}

/// `settings` with `action`'s keys in `section` (`general`, `web`, `mobile` or `tui`) set to `keys` (none written
/// `null`, one as a string, more as a list), or with what the user set there taken out for `None` (and a `keys` or a
/// front end's section left empty by that taken out too). Everything else in `settings` is kept.
pub fn with_binding(settings: &Value, section: &str, action: &str, keys: Option<&[String]>) -> Value {
    let mut root = settings.as_object().cloned().unwrap_or_default();
    let platform = Platform::parse(section);
    let mut outer = match platform {
        Some(p) => root
            .get(p.key())
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        None => Map::new(),
    };
    let held = match platform {
        Some(_) => outer.get("keys"),
        None => root.get("keys"),
    };
    let mut map = held.and_then(Value::as_object).cloned().unwrap_or_default();
    match keys {
        Some([]) => {
            map.insert(action.to_owned(), Value::Null);
        }
        Some([one]) => {
            map.insert(action.to_owned(), Value::String(one.clone()));
        }
        Some(list) => {
            map.insert(action.to_owned(), list.iter().cloned().map(Value::String).collect());
        }
        None => {
            map.remove(action);
        }
    }
    match platform {
        Some(p) => {
            if map.is_empty() {
                outer.remove("keys");
            } else {
                outer.insert("keys".into(), Value::Object(map));
            }
            if outer.is_empty() {
                root.remove(p.key());
            } else {
                root.insert(p.key().into(), Value::Object(outer));
            }
        }
        None if map.is_empty() => {
            root.remove("keys");
        }
        None => {
            root.insert("keys".into(), Value::Object(map));
        }
    }
    Value::Object(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The keymap asset every front end carries.
    fn defaults() -> Value {
        serde_json::from_str(include_str!("../../../web/public/keymaps/default.json")).unwrap()
    }

    fn km(settings: &Value, p: Platform) -> Keymap {
        resolve(&defaults(), settings, p)
    }

    #[test]
    fn keys_are_read_as_zed_and_vs_code_write_them() {
        let p = |s: &str| parse(s).unwrap();
        assert_eq!(p("/"), "/");
        assert_eq!(p("G"), "G");
        assert_eq!(p("shift+g"), "G");
        assert_eq!(p("ctrl+k"), "ctrl+k");
        assert_eq!(p("Ctrl-K"), "ctrl+shift+k");
        assert_eq!(p("shift-ctrl+k"), "ctrl+shift+k");
        assert_eq!(p("cmd+enter"), "meta+enter");
        assert_eq!(p("Escape"), "esc");
        assert_eq!(p("PgDn"), "pagedown");
        assert_eq!(p("shift+tab"), "shift+tab");
        assert_eq!(p("ctrl++"), "ctrl++");
        assert_eq!(p("-"), "-");
        assert_eq!(p("ctrl+-"), "ctrl+-");
        assert_eq!(p("shift+?"), "?");
        assert_eq!(p("F5"), "f5");
        assert_eq!(p("space"), "space");
        assert!(parse("").is_err());
        assert!(parse("ctrl+").is_err());
        assert!(parse("hyper+k").is_err());
        assert!(parse("enterr").is_err());
        assert!(parse("f25").is_err());
    }
    #[test]
    fn a_key_given_is_written_as_the_same_key_in_settings() {
        // The terminal's and the browser's names for one key come to the same thing.
        assert_eq!(from_event("G", &["shift"]), Some(parse("G").unwrap()));
        assert_eq!(
            from_event("K", &["ctrl", "shift"]),
            Some(parse("ctrl+shift+k").unwrap())
        );
        assert_eq!(
            from_event("k", &["ctrl", "shift"]),
            Some(parse("ctrl+shift+k").unwrap())
        );
        assert_eq!(from_event("?", &["shift"]), Some("?".into()));
        assert_eq!(from_event("ArrowDown", &[]), Some("down".into()));
        assert_eq!(from_event(" ", &[]), Some("space".into()));
        assert_eq!(from_event("Tab", &["shift"]), Some("shift+tab".into()));
        assert_eq!(from_event("Shift", &["shift"]), None);
        assert_eq!(from_event("MediaPlayPause", &[]), None);
    }
    #[test]
    fn keys_are_shown_as_hints_show_them() {
        assert_eq!(display("enter"), "↵");
        assert_eq!(display("ctrl+shift+k"), "Ctrl+Shift+K");
        assert_eq!(display("ctrl++"), "Ctrl++");
        assert_eq!(display("G"), "G");
        assert_eq!(display("pagedown"), "PgDn");
    }

    #[test]
    fn the_asset_reads_as_written_and_every_context_names_groups_there_are() {
        let d = defaults();
        let mut quiet = Vec::new();
        for p in Platform::ALL {
            for (name, list) in layer(own_keys(&d, p), "x", &mut quiet).into_iter().flatten() {
                assert!(!section_of(&name).is_empty(), "{name}");
                assert!(
                    list.iter().all(|k| parse(k).as_deref() == Ok(k.as_str())),
                    "{name}: {list:?}"
                );
            }
            let m = km(&json!({}), p);
            assert!(!m.contexts.is_empty(), "{}", p.key());
            let groups: BTreeSet<&str> = m.keys.keys().map(|n| section_of(n)).collect();
            for g in d[p.key()]["contexts"]
                .as_object()
                .unwrap()
                .values()
                .flat_map(|v| v.as_array().unwrap())
            {
                assert!(groups.contains(g.as_str().unwrap()), "{}: {g}", p.key());
            }
        }
        assert_eq!(quiet, Vec::<String>::new());
    }

    #[test]
    fn the_defaults_have_no_clashes_and_shadow_nothing() {
        let d = defaults();
        for p in Platform::ALL {
            let m = km(&json!({}), p);
            assert_eq!(m.problems, Vec::<String>::new(), "{}", p.key());
            for (context, groups) in d[p.key()]["contexts"].as_object().unwrap() {
                let groups: Vec<&str> = groups.as_array().unwrap().iter().filter_map(Value::as_str).collect();
                let mut all: Vec<&String> = m
                    .keys
                    .iter()
                    .filter(|(n, _)| groups.contains(&section_of(n)))
                    .flat_map(|(_, k)| k)
                    .collect();
                let n = all.len();
                all.sort_unstable();
                all.dedup();
                assert_eq!(all.len(), n, "{context}");
            }
        }
    }

    #[test]
    fn slash_is_the_prompt_everywhere() {
        for p in Platform::ALL {
            let m = km(&json!({}), p);
            let ctx = if p == Platform::Tui { "tasks" } else { "app" };
            assert_eq!(m.action(ctx, "/"), Some("app.prompt"), "{}", p.key());
        }
        let tui = km(&json!({}), Platform::Tui);
        assert_eq!(tui.action("tasks", ":"), Some("app.prompt"));
        assert_eq!(tui.action("tasks", "f"), Some("tasks.filter"));
    }

    #[test]
    fn shared_keys_then_the_front_ends_own() {
        let s = json!({
            "keys": { "app.prompt": ["ctrl+k", "/"], "app.undo": "U" },
            "tui": { "keys": { "app.prompt": ":" } },
            "web": { "keys": { "app.prompt": null } }
        });
        let tui = km(&s, Platform::Tui);
        assert_eq!(tui.keys_of("app.prompt"), [":"]);
        assert_eq!(tui.keys_of("app.undo"), ["U"]);
        assert_eq!(tui.action("tasks", "/"), None);
        let web = km(&s, Platform::Web);
        assert!(web.keys_of("app.prompt").is_empty());
        assert_eq!(web.problems, Vec::<String>::new());
        assert!(!web.keys.contains_key("app.undo"), "the web app has no undo key");
        let mobile = km(&s, Platform::Mobile);
        assert_eq!(mobile.keys_of("app.prompt"), ["ctrl+k", "/"]);
        assert_eq!(mobile.action("app", "ctrl+k"), Some("app.prompt"));
    }

    #[test]
    fn a_key_given_to_one_action_leaves_the_default_that_had_it() {
        // `d` was done's; given to delete, done is left with none rather than a clash.
        let m = km(&json!({ "tui": { "keys": { "tasks.delete": "d" } } }), Platform::Tui);
        assert_eq!(m.action("tasks", "d"), Some("tasks.delete"));
        assert!(m.keys_of("tasks.done").is_empty());
        // Burndown's `d` acts elsewhere, so it keeps it.
        assert_eq!(m.action("burndown", "d"), Some("burndown.daily"));
        assert_eq!(m.problems, Vec::<String>::new());
        // A key of `app` comes before a page's own `d` and takes it; a page's key leaves `app`'s alone elsewhere.
        let m = km(
            &json!({ "keys": { "app.undo": "d", "tasks.start": "q" } }),
            Platform::Tui,
        );
        assert_eq!(m.action("tasks", "d"), Some("app.undo"));
        assert_eq!(m.action("burndown", "d"), Some("app.undo"));
        assert_eq!(m.action("tasks", "q"), Some("tasks.start"));
        assert_eq!(m.action("console", "q"), Some("app.quit"));
        assert_eq!(m.problems, Vec::<String>::new());
    }

    #[test]
    fn what_is_wrong_is_said_and_the_rest_applies() {
        let s = json!({
            "keys": { "tasks.nope": "z", "tasks.done": "hyper+d", "tasks.stop": 3 },
            "tui": { "keys": { "tasks.start": "a", "tasks.add": "a" } },
            "web": { "keys": { "tasks.done": "d" } },
            "mobile": { "keys": [] }
        });
        let tui = km(&s, Platform::Tui);
        assert_eq!(
            tui.problems,
            [
                "keys.tasks.done: “hyper+d” is not a key",
                "keys.tasks.stop: 3 is not a key",
                "keys: “tasks.nope” is no action",
                "keys: a is both tasks.add and tasks.start; it runs tasks.add",
            ]
        );
        assert_eq!(tui.action("tasks", "a"), Some("tasks.add"));
        let web = km(&s, Platform::Web);
        assert!(web
            .problems
            .contains(&"web.keys: the web app has no “tasks.done”".to_owned()));
        let mobile = km(&s, Platform::Mobile);
        assert!(mobile
            .problems
            .contains(&"mobile.keys: not an object of actions and their keys".to_owned()));
    }

    #[test]
    fn every_action_says_what_it_does() {
        let d = defaults();
        for p in Platform::ALL {
            for name in km(&json!({}), p).keys.keys() {
                assert!(d["about"][name].as_str().is_some_and(|s| !s.is_empty()), "{name}");
            }
        }
    }

    #[test]
    fn a_keybindings_page_has_the_users_keys_then_the_rest() {
        let s = json!({
            "keys": { "app.prompt": ["ctrl+k", "Ctrl-J"], "tasks.done": "hyper+d" },
            "tui": { "keys": { "tasks.done": null, "tasks.nope": "z" } },
        });
        let b = bindings(&defaults(), &s);
        let ids: Vec<&str> = b.sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, SECTIONS);
        let general = &b.sections[0].actions;
        assert_eq!(general[0].action, "app.prompt");
        assert_eq!(general[0].keys, Some(vec!["ctrl+k".into(), "ctrl+shift+j".into()]));
        assert_eq!(general[0].defaults, ["/"]);
        assert!(general[0].about.starts_with("put the cursor"));
        assert_eq!(general[1].keys, Some(vec!["hyper+d".into()]), "kept as written");
        assert!(general[2..].iter().all(|a| a.keys.is_none()));
        assert!(general.iter().any(|a| a.action == "tasks.filter"), "every front end's");
        let web: Vec<(&str, &[String])> = b.sections[1]
            .actions
            .iter()
            .map(|a| (a.action.as_str(), a.defaults.as_slice()))
            .collect();
        assert!(
            web.contains(&("app.prompt", &["/".to_owned()][..])),
            "the shared default"
        );
        assert!(web.contains(&("app.add", &["a".to_owned()][..])), "its own");
        assert!(
            !web.iter().any(|(a, _)| *a == "tasks.done"),
            "only the web app's actions"
        );
        let tui = &b.sections[3].actions;
        assert_eq!(tui[0].keys, Some(vec![]));
        assert_eq!((tui[1].action.as_str(), tui[1].about.as_str()), ("tasks.nope", ""));
        let prompt = tui.iter().find(|a| a.action == "app.prompt").unwrap();
        assert_eq!(prompt.defaults, ["/", ":"], "the terminal app's own replace the shared");
        assert!(b
            .problems
            .contains(&"keys.tasks.done: “hyper+d” is not a key".to_owned()));
        assert!(b.problems.contains(&"tui.keys: “tasks.nope” is no action".to_owned()));
        assert_eq!(
            b.problems.iter().filter(|p| p.contains("hyper+d")).count(),
            1,
            "said once"
        );
    }

    #[test]
    fn a_key_is_set_and_taken_out_and_nothing_else_moves() {
        let s = json!({ "taskCommands": [1], "web": { "theme": "dark" } });
        let one = with_binding(&s, "web", "app.prompt", Some(&["ctrl+k".into()]));
        assert_eq!(
            one,
            json!({ "taskCommands": [1], "web": { "theme": "dark", "keys": { "app.prompt": "ctrl+k" } } })
        );
        let two = with_binding(&one, "general", "app.undo", Some(&["u".into(), "U".into()]));
        assert_eq!(two["keys"], json!({ "app.undo": ["u", "U"] }));
        let none = with_binding(&two, "tui", "tasks.done", Some(&[]));
        assert_eq!(none["tui"], json!({ "keys": { "tasks.done": null } }));
        let back = with_binding(
            &with_binding(&none, "tui", "tasks.done", None),
            "general",
            "app.undo",
            None,
        );
        let back = with_binding(&back, "web", "app.prompt", None);
        assert_eq!(back, s);
        assert_eq!(with_binding(&Value::Null, "mobile", "app.prompt", None), json!({}));
    }

    #[test]
    fn no_defaults_is_no_keys() {
        let m = resolve(&Value::Null, &json!({ "keys": { "app.prompt": "/" } }), Platform::Web);
        assert!(m.keys.is_empty() && m.contexts.is_empty());
        assert_eq!(m.problems, ["keys: “app.prompt” is no action"]);
    }
}

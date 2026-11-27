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

/// The group a binding's name is in: an action's (`tasks.done`), or that of the action an expression runs (`filter
/// +home` is in `tasks`, as `tasks.filter` is).
pub fn group_of(name: &str) -> String {
    if name.contains(' ') || !name.contains('.') {
        if let Some(call) = call_of(name) {
            return section_of(&call.action).to_owned();
        }
    }
    section_of(name).to_owned()
}

/// The keys of every action a front end has, the groups each of its contexts puts keys in, and what was wrong.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Keymap {
    /// Each action the front end has and its keys, as [`from_event`] writes them (none for an action left without).
    pub keys: BTreeMap<String, Vec<String>>,
    /// Each context's keys and the action each runs.
    pub contexts: BTreeMap<String, BTreeMap<String, String>>,
    /// Each context's groups, the most particular first: where a command typed there acts ([`command_line_in`]).
    pub groups: BTreeMap<String, Vec<String>>,
    pub problems: Vec<String>,
    /// The user's bindings that run no command, skipped (each is among `problems` too).
    pub bad: Vec<BadKey>,
}

/// A binding in `settings.json` that runs no command: it is skipped, and a Keybindings page warns of it with a way to
/// take it out ([`without_key`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BadKey {
    /// Where: `general` (`keys`) or a front end's own (`tui`, `web`, `mobile`).
    pub section: String,
    pub group: String,
    /// The key as written.
    pub key: String,
    /// What it runs, as written.
    pub runs: String,
    pub why: String,
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

/// `<place>.keys` of the keymap asset (`keys` itself for none): its actions and their keys, read; `None` where it says
/// nothing. The asset lists each action with its keys; `settings.json` binds keys (see [`resolve`]).
fn layer(value: Option<&Value>, place: &str, problems: &mut Vec<String>) -> Option<Vec<(String, Vec<String>)>> {
    let value = value.filter(|v| !v.is_null())?;
    let Some(map) = value.as_object() else {
        problems.push(format!("{place}: not an object of actions and their keys"));
        return None;
    };
    let mut out = Vec::new();
    for (name, v) in map {
        let mut list: Vec<String> = Vec::new();
        for t in texts(v) {
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

/// A value that is one key, a list of them, or none (`null`).
fn texts(v: &Value) -> Vec<&Value> {
    match v {
        Value::Null => Vec::new(),
        Value::Array(list) => list.iter().collect(),
        other => vec![other],
    }
}

fn own_keys(v: &Value, platform: Platform) -> Option<&Value> {
    v.get(platform.key()).and_then(|s| s.get("keys"))
}

/// `name`'s keys in an asset layer, read; `None` where the layer does not name it.
fn keys_in(v: Option<&Value>, name: &str) -> Option<Vec<String>> {
    let v = v?.get(name)?;
    Some(
        texts(v)
            .into_iter()
            .filter_map(|t| t.as_str().and_then(|s| parse(s).ok()))
            .collect(),
    )
}

/// An action's default keys for a section of `settings.json`: a front end's own (else the shared ones), or for `None`
/// (`keys`, every front end) the shared ones and every front end's.
fn default_keys(defaults: &Value, section: Option<Platform>, name: &str) -> Vec<String> {
    match section {
        Some(p) => keys_in(own_keys(defaults, p), name)
            .or_else(|| keys_in(defaults.get("keys"), name))
            .unwrap_or_default(),
        None => {
            let mut out = keys_in(defaults.get("keys"), name).unwrap_or_default();
            for p in Platform::ALL {
                for k in keys_in(own_keys(defaults, p), name).unwrap_or_default() {
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
            out
        }
    }
}

/// The object a section of `settings.json` binds keys in: `keys`, or `<front end>.keys`.
fn section_of_settings(root: &Map<String, Value>, platform: Option<Platform>) -> Option<&Map<String, Value>> {
    match platform {
        Some(p) => root.get(p.key()).and_then(|o| o.get("keys")),
        None => root.get("keys"),
    }
    .and_then(Value::as_object)
}

/// `root` with a section's keys set to `map`, or taken out (and a front end's object left empty by that too).
fn put_section(root: &mut Map<String, Value>, platform: Option<Platform>, map: Map<String, Value>) {
    match platform {
        Some(p) => {
            let mut outer = root
                .get(p.key())
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
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
}

/// Version 1 of `settings.json` to version 2 (spec 0020): each section of keys was its actions and their keys
/// (`"tasks.filter": "F"`, which replaced the action's own keys); it is now each group's keys and what each runs
/// (`"tasks": { "F": "tasks.filter", "f": null }`), so a default key the old keys dropped is unbound. Groups already
/// written the new way are kept.
pub(crate) fn keys_v1_to_v2(root: &mut Map<String, Value>, defaults: &Value) {
    // A front end's own keys replaced an action's shared ones too, the user's included: those are unbound for it as well.
    let old_shared: Map<String, Value> = section_of_settings(root, None)
        .map(|m| {
            m.iter()
                .filter(|(_, v)| !v.is_object())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let read = |v: &Value| -> Vec<String> {
        texts(v)
            .into_iter()
            .filter_map(Value::as_str)
            .map(|s| parse(s).unwrap_or_else(|_| s.to_owned()))
            .collect()
    };
    for id in SECTIONS {
        let platform = Platform::parse(id);
        let Some(map) = section_of_settings(root, platform) else {
            continue;
        };
        if map.values().all(Value::is_object) {
            continue;
        }
        let mut out: Map<String, Value> = map
            .iter()
            .filter(|(_, v)| v.is_object())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (name, v) in map.iter().filter(|(_, v)| !v.is_object()) {
            // What was not a key at all (a number) is dropped; a key that cannot be read is kept, to be said.
            let keys = read(v);
            let mut unbound = default_keys(defaults, platform, name);
            if platform.is_some() {
                unbound.extend(old_shared.get(name).map(read).unwrap_or_default());
            }
            let Some(group) = out
                .entry(group_of(name))
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
            else {
                continue;
            };
            for k in &keys {
                group.insert(k.clone(), Value::String(name.clone()));
            }
            for d in unbound {
                if !keys.contains(&d) {
                    group.entry(d).or_insert(Value::Null);
                }
            }
        }
        put_section(root, platform, out);
    }
}

/// The keys of `platform`: its `defaults` (the keymap asset), then `keys` and `<platform>.keys` of `settings` (read as
/// version 2: each group's keys and what each runs, an action or a command with its words, `null` for none).
pub fn resolve(defaults: &Value, settings: &Value, platform: Platform) -> Keymap {
    let migrated = crate::settings_version::migrate(settings, defaults);
    let settings = migrated.as_ref().unwrap_or(settings);
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
    let known = known_actions(defaults);
    let contexts_of = |p: Platform| -> Vec<(String, Vec<String>)> {
        defaults
            .get(p.key())
            .and_then(|o| o.get("contexts"))
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(name, groups)| {
                let groups = groups.as_array().into_iter().flatten().filter_map(Value::as_str);
                (name.clone(), groups.map(str::to_owned).collect())
            })
            .collect()
    };
    let contexts = contexts_of(platform);
    let mut every_group: BTreeSet<String> = known.iter().map(|n| section_of(n).to_owned()).collect();
    for p in Platform::ALL {
        every_group.extend(contexts_of(p).into_iter().flat_map(|(_, g)| g));
    }

    // Each group's keys and what each runs, from the defaults.
    let mut groups: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (name, list) in &keys {
        for k in list {
            groups
                .entry(group_of(name))
                .or_default()
                .entry(k.clone())
                .or_insert_with(|| name.clone());
        }
    }
    // A group comes before another where both act, in some context: its keys are looked at first there.
    let before = |a: &str, b: &str| {
        contexts.iter().any(|(_, gs)| {
            let at = |g: &str| gs.iter().position(|x| x == g);
            matches!((at(a), at(b)), (Some(i), Some(j)) if i < j)
        })
    };

    let mut by_user: BTreeSet<(String, String)> = BTreeSet::new();
    let mut bad: Vec<BadKey> = Vec::new();
    let place = format!("{}.keys", platform.key());
    for (own, at, value) in [
        (false, "keys", settings.get("keys")),
        (true, place.as_str(), own_keys(settings, platform)),
    ] {
        let Some(value) = value.filter(|v| !v.is_null()) else {
            continue;
        };
        let Some(map) = value.as_object() else {
            problems.push(format!("{at}: not an object of groups and their keys"));
            continue;
        };
        for (group, entries) in map {
            if !every_group.contains(group) {
                problems.push(format!("{at}: “{group}” is no group of keys"));
                continue;
            }
            let Some(entries) = entries.as_object() else {
                problems.push(format!("{at}.{group}: not an object of keys and what each runs"));
                continue;
            };
            for (text, v) in entries {
                let k = match parse(text) {
                    Ok(k) => k,
                    Err(e) => {
                        problems.push(format!("{at}.{group}: {e}"));
                        continue;
                    }
                };
                let name = match v {
                    Value::Null => None,
                    Value::String(s) => match check(s, &[group.as_str()], Some(&known)).and_then(|c| {
                        // A command another front end's action runs is not wrong here; one with no action is.
                        known
                            .contains(&c.action)
                            .then_some(c.action)
                            .ok_or_else(|| format!("“{}” is no action", s.trim()))
                    }) {
                        Ok(a) if keys.contains_key(&a) => in_group(s, group).ok(),
                        // Shared keys may name another front end's actions; a front end's own may not.
                        Ok(a) => {
                            if own {
                                problems.push(format!("{at}: the {} app has no “{a}”", platform.key()));
                            }
                            continue;
                        }
                        Err(why) => {
                            let section = if own { platform.key() } else { "general" };
                            problems.push(bad_line(at, group, text, s, &why));
                            bad.push(BadKey {
                                section: section.to_owned(),
                                group: group.clone(),
                                key: text.clone(),
                                runs: s.clone(),
                                why,
                            });
                            continue;
                        }
                    },
                    other => {
                        problems.push(format!("{at}.{group}: {other} is not an action"));
                        continue;
                    }
                };
                let mut unbind = |g: &str, groups: &mut BTreeMap<String, BTreeMap<String, String>>| {
                    if let Some(old) = groups.get_mut(g).and_then(|m| m.remove(&k)) {
                        if let Some(list) = keys.get_mut(&old) {
                            list.retain(|x| *x != k);
                        }
                    }
                };
                unbind(group, &mut groups);
                // A key given to one group is taken from a default of a group that comes before it where both act,
                // so rebinding needs no unbinding.
                if name.is_some() {
                    let others: Vec<String> = groups
                        .keys()
                        .filter(|g| *g != group && before(g, group) && !by_user.contains(&((*g).clone(), k.clone())))
                        .cloned()
                        .collect();
                    for g in others {
                        unbind(&g, &mut groups);
                    }
                }
                if let Some(n) = name {
                    groups.entry(group.clone()).or_default().insert(k.clone(), n.clone());
                    let list = keys.entry(n).or_default();
                    if !list.contains(&k) {
                        list.push(k.clone());
                    }
                }
                by_user.insert((group.clone(), k));
            }
        }
    }

    // Each context's keys: a more particular group's key comes before the same key of a group around it.
    let mut by_context = BTreeMap::new();
    for (context, in_groups) in &contexts {
        let mut map: BTreeMap<String, (String, String)> = BTreeMap::new();
        for g in in_groups {
            for (k, name) in groups.get(g).into_iter().flatten() {
                match map.get(k) {
                    // Two keys the user set, of groups that both act here: the more particular one runs.
                    Some((first, fg))
                        if fg != g
                            && by_user.contains(&(fg.clone(), k.clone()))
                            && by_user.contains(&(g.clone(), k.clone())) =>
                    {
                        let line = format!(
                            "keys: {} is both {first} ({fg}) and {name} ({g}) in {context}; it runs {first}",
                            display(k)
                        );
                        if !problems.contains(&line) {
                            problems.push(line);
                        }
                    }
                    Some(_) => {}
                    None => {
                        map.insert(k.clone(), (name.clone(), g.clone()));
                    }
                }
            }
        }
        let map: BTreeMap<String, String> = map.into_iter().map(|(k, (n, _))| (k, n)).collect();
        // A key that is also the start of a longer one: the longer one can never be pressed.
        for (k, name) in &map {
            let start = format!("{k} ");
            if let Some((long, other)) = map.iter().find(|(l, _)| l.starts_with(&start)) {
                let line = format!(
                    "keys: {} ({name}) is the start of {} ({other}) in {context}, which cannot be pressed",
                    display(k),
                    display(long)
                );
                if !problems.contains(&line) {
                    problems.push(line);
                }
            }
        }
        by_context.insert(context.clone(), map);
    }

    let groups = contexts.into_iter().collect();
    Keymap {
        keys,
        contexts: by_context,
        groups,
        problems,
        bad,
    }
}

/// How a binding that runs no command is said, among a keymap's problems.
fn bad_line(at: &str, group: &str, key: &str, runs: &str, why: &str) -> String {
    format!("{at}.{group}: {key} runs “{}”, which is skipped: {why}", runs.trim())
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

/// A key as the user wrote it in `settings.json`, as [`from_event`] writes it. Keys pressed one after another are a
/// sequence, written with a space between them (`g g`, `ctrl+k ctrl+s`, as Zed and VS Code write them) or, for two
/// characters, together (`gg`, `g1`; a longer word is more likely a misspelt name); either way it is written back with
/// spaces.
///
/// # Errors
///
/// It is empty, or names a key or a modifier there is none of.
pub fn parse(text: &str) -> Result<String, String> {
    let bad = || format!("“{text}” is not a key");
    let text = text.trim_matches(|c| c == '\t' || c == '\n');
    if text.contains(' ') && text.trim() == text && !text.is_empty() {
        let keys: Result<Vec<String>, String> = text.split(' ').filter(|w| !w.is_empty()).map(chord).collect();
        return keys.map(|k| k.join(" ")).map_err(|_| bad());
    }
    match chord(text) {
        Ok(k) => Ok(k),
        // Two characters together, neither a modifier's `+` or `-`: one key after the other.
        Err(e) if text.chars().count() == 2 && !text.contains(['+', '-']) && !text.contains(char::is_whitespace) => {
            let keys: Result<Vec<String>, String> = text.chars().map(|c| chord(&c.to_string())).collect();
            keys.map(|k| k.join(" ")).map_err(|_| e)
        }
        Err(e) => Err(e),
    }
}

/// One key, with its modifiers.
fn chord(text: &str) -> Result<String, String> {
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

/// A key as a hint shows it: `↵`, `↓`, `Space`, `Ctrl+K`, `G`; a sequence of characters together (`gg`, `g1`), else with
/// a space (`Ctrl+K Ctrl+S`).
pub fn display(key: &str) -> String {
    if key.contains(' ') && key != " " {
        let keys: Vec<String> = key.split(' ').filter(|k| !k.is_empty()).map(display_one).collect();
        let between = if keys.iter().all(|k| k.chars().count() == 1) {
            ""
        } else {
            " "
        };
        return keys.join(between);
    }
    display_one(key)
}

fn display_one(chord: &str) -> String {
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

/// A command of the command line (`:tabn 3`) and of a key bound to one (`"g h": "filter +home"`), as vim names it where
/// vim has it. Commands are what keys run (spec 0025): every action but `app.commandLine` (the `:` itself) is run by
/// one, and an action's name in `settings.json` is read as that command, its preset ([`preset_of`]). What a command
/// does, the words it takes and since when are written for people in `web/public/keymaps/commands.json`, which every
/// front end carries; a test keeps its names, shortest forms and whether it takes words the same as these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub name: &'static str,
    /// How much of the name must be written (vim's `tabn[ext]` is 4).
    pub shortest: usize,
    /// What it runs with nothing after it: one action, or one for each place it acts in (`next` is the calendar's
    /// next month and a board's next board), the one of the place it is used in. None: it needs a word.
    pub bare: &'static [&'static str],
    /// A word that runs an action of its own, alone after the name (`info details`, `sort raise`).
    pub subs: &'static [(&'static str, &'static str)],
    /// What other words it takes.
    pub words: Words,
}

/// The words a command takes, besides its [`Command::subs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    None,
    /// Any words, for this action (`filter +home`).
    Any(&'static str),
    /// One of these, for this action (`lock pin`).
    OneOf(&'static str, &'static [&'static str]),
    /// A page's number (1 to 9) or the start of its name, for `page.go`.
    Page,
    /// Changes to the report's sort, for `tasks.sort` ([`crate::sortspec::edit`]).
    Sort,
}

const fn cmd(name: &'static str, shortest: usize, bare: &'static [&'static str]) -> Command {
    Command {
        name,
        shortest,
        bare,
        subs: &[],
        words: Words::None,
    }
}

const fn with(
    name: &'static str,
    shortest: usize,
    bare: &'static [&'static str],
    subs: &'static [(&'static str, &'static str)],
    words: Words,
) -> Command {
    Command {
        name,
        shortest,
        bare,
        subs,
        words,
    }
}

/// The pages `tabnext` names, in the order of the top bar, Settings last.
pub const PAGES: [&str; 9] = [
    "tasks", "projects", "tags", "summary", "calendar", "burndown", "boards", "console", "settings",
];

/// Every command, vim's first (with vim's shortest forms), then the ones vim has none of. A row number (`:5`), `:$`,
/// `:+3` and `:-3` move as in vim and are not in the table.
pub const COMMANDS: [Command; 59] = [
    with("tabnext", 4, &["page.next"], &[], Words::Page),
    cmd("tabprevious", 4, &["page.prev"]),
    cmd("tabNext", 4, &["page.prev"]),
    cmd("tabfirst", 6, &["page.first"]),
    cmd("tabrewind", 4, &["page.first"]),
    cmd("tablast", 4, &["page.last"]),
    cmd("quit", 1, &["app.quit"]),
    cmd("undo", 1, &["app.undo"]),
    cmd("write", 1, &["app.sync"]),
    with(
        "edit",
        1,
        &["app.reload"],
        &[("settings", "settings.editFile")],
        Words::None,
    ),
    cmd("help", 1, &["app.help"]),
    with(
        "filter",
        4,
        &["tasks.filter", "app.filter", "kanban.filter"],
        &[],
        Words::Any("tasks.filter"),
    ),
    with(
        "sort",
        3,
        &["tasks.sort"],
        &[
            ("+", "sort.add"),
            ("--", "sort.remove"),
            ("=", "sort.reverse"),
            ("/", "sort.group"),
            ("raise", "sort.raise"),
            ("lower", "sort.lower"),
            ("reset", "sort.reset"),
            ("apply", "sort.apply"),
            ("cancel", "sort.cancel"),
        ],
        Words::Sort,
    ),
    with(
        "delete",
        1,
        &["tasks.delete"],
        &[("completed", "tasks.clearCompleted")],
        Words::None,
    ),
    with(
        "mark",
        2,
        &["tasks.mark"],
        &[("all", "tasks.markAll"), ("range", "tasks.markRange")],
        Words::None,
    ),
    cmd(
        "open",
        1,
        &[
            "tasks.open",
            "lists.open",
            "boards.list.open",
            "settings.activate",
            "kanban.list.open",
            "settings.kanban.open",
        ],
    ),
    cmd("done", 2, &["tasks.done", "info.done"]),
    cmd("start", 3, &["tasks.start"]),
    cmd("stop", 3, &["tasks.stop"]),
    cmd("toggle", 2, &["tasks.toggle", "info.toggle"]),
    with(
        "modify",
        3,
        &["tasks.modify", "info.modify"],
        &[],
        Words::Any("tasks.modify"),
    ),
    with(
        "annotate",
        2,
        &["tasks.annotate", "info.annotate"],
        &[],
        Words::Any("tasks.annotate"),
    ),
    with(
        "denotate",
        3,
        &["tasks.denotate", "info.denotate"],
        &[],
        Words::Any("tasks.denotate"),
    ),
    cmd("duplicate", 3, &["tasks.duplicate"]),
    with(
        "info",
        2,
        &["tasks.info", "kanban.info"],
        &[
            ("details", "info.details"),
            ("recurrence", "info.recurrence"),
            ("timelog", "info.timeLog"),
            ("annotations", "info.annotations"),
            ("unknown", "info.unknown"),
            ("raw", "info.raw"),
            ("modify", "info.modify"),
            ("links", "info.links"),
        ],
        Words::None,
    ),
    with("add", 2, &["tasks.add", "app.add"], &[], Words::Any("tasks.add")),
    with(
        "report",
        3,
        &["tasks.report", "sidebar.open", "kanban.report"],
        &[],
        Words::Any("tasks.report"),
    ),
    with("context", 3, &["app.context"], &[], Words::Any("app.context")),
    cmd("sync", 2, &["app.sync"]),
    cmd("review", 3, &["app.review"]),
    with("lock", 4, &["app.lock"], &[], Words::OneOf("app.lock", &["pin", "key"])),
    cmd("next", 1, &["calendar.next", "boards.next", "kanban.next"]),
    cmd("previous", 4, &["calendar.prev", "boards.prev", "kanban.prev"]),
    cmd("snext", 2, &["move.sectionNext"]),
    cmd("sprevious", 3, &["move.sectionPrev"]),
    cmd("pagedown", 5, &["move.pageDown"]),
    cmd("pageup", 5, &["move.pageUp"]),
    cmd("halfdown", 5, &["move.halfPageDown"]),
    cmd("halfup", 5, &["move.halfPageUp"]),
    cmd("prompt", 3, &["app.prompt"]),
    cmd("sidebar", 2, &["app.sidebar"]),
    cmd("hidden", 3, &["sidebar.hidden"]),
    cmd("back", 2, &["app.back", "boards.close", "kanban.close"]),
    cmd("stopall", 5, &["app.stopTimer"]),
    cmd("reminders", 3, &["app.reminders"]),
    cmd("export", 2, &["tasks.export", "boards.export"]),
    cmd("clear", 2, &["console.clear"]),
    with(
        "burndown",
        2,
        &[],
        &[
            ("day", "burndown.daily"),
            ("week", "burndown.weekly"),
            ("month", "burndown.monthly"),
        ],
        Words::None,
    ),
    with(
        "command",
        3,
        &["tasks.command"],
        &[
            ("add", "settings.commands.add"),
            ("edit", "settings.commands.edit"),
            ("delete", "settings.commands.delete"),
        ],
        Words::None,
    ),
    with(
        "key",
        3,
        &[],
        &[
            ("add", "settings.keys.add"),
            ("remove", "settings.keys.remove"),
            ("none", "settings.keys.none"),
            ("defaults", "settings.keys.defaults"),
            ("search", "settings.keys.search"),
        ],
        Words::None,
    ),
    cmd("home", 2, &["settings.general.home"]),
    cmd("apply", 2, &["settings.apply"]),
    cmd("revert", 4, &["settings.revert"]),
    with(
        "rule",
        2,
        &[],
        &[
            ("add", "settings.appearance.add"),
            ("delete", "settings.appearance.delete"),
        ],
        Words::None,
    ),
    cmd("syncwhen", 5, &["settings.sync.change"]),
    cmd("theme", 2, &["settings.theme.preset"]),
    cmd("shading", 2, &["settings.theme.shading"]),
    with(
        "kanban",
        2,
        &["page.kanban"],
        &[
            ("since", "kanban.since"),
            ("other", "kanban.other"),
            ("fold", "kanban.fold"),
            ("new", "settings.kanban.new"),
            ("column", "settings.kanban.add"),
            ("edit", "settings.kanban.edit"),
            ("delete", "settings.kanban.delete"),
            ("raise", "settings.kanban.raise"),
            ("lower", "settings.kanban.lower"),
            ("write", "settings.kanban.write"),
            ("off", "settings.kanban.off"),
        ],
        Words::None,
    ),
    with(
        "move",
        3,
        &["kanban.move"],
        &[("left", "kanban.moveLeft"), ("right", "kanban.moveRight")],
        Words::None,
    ),
];

/// What a key or a command line asks for: an action, with the words after it and a count (`5j`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Call {
    pub action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

impl Call {
    fn of(action: &str, args: Vec<String>) -> Call {
        Call {
            action: action.to_owned(),
            args,
            count: None,
        }
    }
}

/// The command `word` names: its full name, or as much of it as vim allows (`tabn`, `tabne`, ... `tabnext`).
pub fn command_named(word: &str) -> Option<&'static Command> {
    COMMANDS
        .iter()
        .find(|c| word.len() >= c.shortest && c.name.starts_with(word))
}

/// A command as vim writes it, the part that may be left out in brackets: `tabn[ext]`.
pub fn written(c: &Command) -> String {
    if c.shortest < c.name.len() {
        format!("{}[{}]", &c.name[..c.shortest], &c.name[c.shortest..])
    } else {
        c.name.to_owned()
    }
}

/// `a, b or c`.
fn either(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [one] => format!("“{one}”"),
        [rest @ .., last] => format!(
            "{} or “{last}”",
            rest.iter().map(|w| format!("“{w}”")).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Of `actions`, the one acting in the first of `groups` that one acts in; the first where none does.
fn in_place<'a>(actions: &[&'a str], groups: &[&str]) -> Option<&'a str> {
    groups
        .iter()
        .find_map(|g| actions.iter().find(|a| section_of(a) == *g))
        .or_else(|| actions.first())
        .copied()
}

/// Whether `args` are words `words` takes, and the action they are for.
fn words_for(name: &str, words: Words, args: &[String]) -> Result<&'static str, String> {
    match words {
        Words::None => Err(format!("{name} takes no words")),
        Words::Any(a) => Ok(a),
        Words::OneOf(a, list) => match args {
            [w] if list.contains(&w.as_str()) => Ok(a),
            _ => Err(format!("{name} takes {}", either(list))),
        },
        Words::Page => match args {
            [w] if page_named(w).is_some() => Ok("page.go"),
            _ => Err(format!(
                "{name} takes a page's number (1 to 9) or the start of its name"
            )),
        },
        Words::Sort => crate::sortspec::edit_words(args).map(|()| "tasks.sort"),
    }
}

/// A page by its number (1 to 9) or the start of its name, in any case.
pub fn page_named(word: &str) -> Option<&'static str> {
    if let Ok(n) = word.parse::<usize>() {
        return n.checked_sub(1).and_then(|i| PAGES.get(i)).copied();
    }
    let w = word.to_lowercase();
    (!w.is_empty()).then(|| PAGES.iter().find(|p| p.starts_with(&w)).copied())?
}

/// What `c` with `args` after it runs, in a place where `groups` act.
fn command_call(c: &Command, args: Vec<String>, groups: &[&str]) -> Result<Call, String> {
    let subs: Vec<&str> = c.subs.iter().map(|(w, _)| *w).collect();
    if args.is_empty() {
        return match in_place(c.bare, groups) {
            Some(a) => Ok(Call::of(a, args)),
            None => Err(format!("{} needs a word: {}", c.name, either(&subs))),
        };
    }
    if let [w] = args.as_slice() {
        if let Some((_, a)) = c.subs.iter().find(|(s, _)| s == w) {
            return Ok(Call::of(a, Vec::new()));
        }
    }
    match (c.words, subs.is_empty()) {
        (Words::None, false) => Err(format!("{} takes {}", c.name, either(&subs))),
        (words, _) => words_for(c.name, words, &args).map(|a| Call::of(a, args)),
    }
}

/// What an expression asks for, or why it is no command: a command and its words (`tabn 3`, `filter +home`, split as
/// the engine splits a line, so a word needs quotes only when it has a space), a row as vim writes one (`5`, `$`, `+3`,
/// `-3`), or an action's own name, with words where its command takes them (`tasks.done`, `tasks.filter +home`).
/// `groups` are the groups acting where it is used, most particular first: a command that acts in several places
/// (`next`) runs the action of the first of them it acts in. `known`, when given, are the actions there are: another
/// dotted name is no action.
///
/// # Errors
///
/// The reason it is no command: an empty one, a name that is no command or not enough of one, words it does not take,
/// or none where it needs one.
pub fn check(expression: &str, groups: &[&str], known: Option<&BTreeSet<String>>) -> Result<Call, String> {
    let mut words = crate::filter::split_words(expression).into_iter();
    let Some(first) = words.next() else {
        return Err("an empty command".into());
    };
    let args: Vec<String> = words.collect();
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u32>().ok())
            .flatten()
            .filter(|n| *n > 0)
    };
    if args.is_empty() {
        if first == "$" {
            return Ok(Call::of("move.last", Vec::new()));
        }
        // Row 1 is the first, which a count before it makes a row (`5gg`, as in vim).
        if first == "1" {
            return Ok(Call::of("move.first", Vec::new()));
        }
        if let Some(n) = number(&first) {
            return Ok(Call::of("move.row", vec![n.to_string()]));
        }
        for (sign, action) in [('+', "move.down"), ('-', "move.up")] {
            if let Some(n) = first.strip_prefix(sign).and_then(number) {
                return Ok(Call {
                    count: Some(n),
                    ..Call::of(action, Vec::new())
                });
            }
        }
    }
    if first.contains('.') && !first.starts_with('.') && !first.ends_with('.') {
        if known.is_some_and(|k| !k.contains(&first)) {
            return Err(format!("“{first}” is no action"));
        }
        if args.is_empty() {
            return Ok(Call::of(&first, args));
        }
        // Words go to an action whose command takes them, as they would after the command.
        let taker = COMMANDS.iter().find(|c| match c.words {
            Words::Any(a) | Words::OneOf(a, _) => a == first,
            Words::Page => first == "page.go",
            Words::Sort => first == "tasks.sort",
            Words::None => false,
        });
        return match taker {
            Some(c) => words_for(&first, c.words, &args).map(|_| Call::of(&first, args)),
            None => Err(format!("{first} takes no words")),
        };
    }
    let Some(c) = command_named(&first) else {
        let could: Vec<&str> = COMMANDS
            .iter()
            .filter(|c| c.name.starts_with(&first))
            .map(|c| c.name)
            .collect();
        return Err(if could.is_empty() {
            format!("“{first}” is no command")
        } else {
            format!("“{first}” is not enough to say which: {}", either(&could))
        });
    };
    command_call(c, args, groups)
}

/// What an expression asks for (see [`check`]), wherever it is used; `None` for anything that is no command.
pub fn call_of(expression: &str) -> Option<Call> {
    check(expression, &[], None).ok()
}

/// The command an action runs, as a key bound to it is saved and the Keybindings page shows it: `done` for
/// `tasks.done`, `info details` for `info.details`, `tabnext tasks` for `page.tasks`. `None` for `app.commandLine`
/// (the `:` itself) and a name that is no action.
pub fn preset_of(action: &str) -> Option<String> {
    let own = match action {
        "move.first" => Some("1"),
        "move.last" => Some("$"),
        "move.down" => Some("+1"),
        "move.up" => Some("-1"),
        "move.row" => Some("5"),
        "page.go" => Some("tabnext 1"),
        _ => None,
    };
    if let Some(p) = own {
        return Some(p.to_owned());
    }
    if let Some(page) = action.strip_prefix("page.").filter(|p| PAGES.contains(p)) {
        return Some(format!("tabnext {page}"));
    }
    for c in &COMMANDS {
        if let Some((w, _)) = c.subs.iter().find(|(_, a)| *a == action) {
            return Some(format!("{} {w}", c.name));
        }
    }
    // The command named as the action is (`sync` before vim's `write`), else the first that runs it.
    let last = action.rsplit('.').next().unwrap_or(action).to_lowercase();
    let runs: Vec<&Command> = COMMANDS.iter().filter(|c| c.bare.contains(&action)).collect();
    runs.iter()
        .find(|c| c.name == last)
        .or_else(|| runs.first())
        .map(|c| c.name.to_owned())
}

/// What a binding is listed as on a Keybindings page, in `group`: the action it runs when it is an action alone
/// (`done` and `tasks.done` are both `tasks.done`), else the command as written (`filter +home`).
pub fn row_name(runs: &str, group: &str) -> String {
    match check(runs, &[group], None) {
        Ok(c) if c.args.is_empty() && c.count.is_none() => c.action,
        _ => runs.trim().to_owned(),
    }
}

/// A binding as a context keeps it, in `group`: the action alone where it is one (`sort raise` is `sort.raise`, and
/// `next` bound for boards `boards.next`, so a screen matching on the action finds it), else the expression as written
/// (`filter +home`, `+3`), whose words say the same wherever it is.
fn in_group(runs: &str, group: &str) -> Result<String, String> {
    let c = check(runs, &[group], None)?;
    Ok(if c.count.is_none() && c.args.is_empty() {
        c.action
    } else {
        runs.trim().to_owned()
    })
}

/// A command line that runs a command rather than going to `task`: `:` first, joined to a command (`:tabn Tasks`,
/// `:5`, `:done`). `None` for any other line, which goes to `task` as it is (`: tabn`, `:foo`, `!(+home)`).
pub fn command_line(line: &str) -> Option<Call> {
    command_line_in(line, &[])
}

/// [`command_line`] where `groups` act (a [`Keymap`]'s `groups` of the context it is typed in): `:next` on the
/// calendar is its next month.
pub fn command_line_in(line: &str, groups: &[&str]) -> Option<Call> {
    let rest = line.strip_prefix(':')?;
    if rest.starts_with(char::is_whitespace) {
        return None;
    }
    check(rest, groups, None).ok()
}

/// The keys pressed so far that have not run anything yet: a count (`5` of `5j`) and the start of a longer key (`g` of
/// `gg`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Pending {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.count.is_none() && self.keys.is_empty()
    }

    /// As the status line shows it: `5g`.
    pub fn label(&self) -> String {
        let count = self.count.map(|n| n.to_string()).unwrap_or_default();
        count + &display(&self.keys.join(" "))
    }
}

/// What a key does after the keys pending before it: waits for more (a count, or the start of a longer key), runs a
/// call, or does nothing (a key bound to nothing there, or Esc, which drops what was pending).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyStep {
    pub pending: Pending,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<Call>,
}

impl Keymap {
    /// The key `chord` pressed in `context`, after `pending`.
    pub fn step(&self, context: &str, pending: &Pending, chord: &str) -> KeyStep {
        let empty = BTreeMap::new();
        step(self.contexts.get(context).unwrap_or(&empty), pending, chord)
    }
}

/// The key `chord` pressed after `pending`, where `map` is a context's keys (a [`Keymap`]'s `contexts` entry).
pub fn step(map: &BTreeMap<String, String>, pending: &Pending, chord: &str) -> KeyStep {
    let none = || KeyStep {
        pending: Pending::default(),
        call: None,
    };
    if chord == "esc" && !pending.is_empty() {
        return none();
    }
    let bound_from = |seq: &str| {
        let start = format!("{seq} ");
        map.contains_key(seq) || map.keys().any(|k| k.starts_with(&start))
    };
    // A digit bound to nothing is a count, as in vim (`0` only after another digit).
    if pending.keys.is_empty() && !bound_from(chord) {
        if let Some(d) = chord.parse::<u32>().ok().filter(|_| chord.len() == 1) {
            if d > 0 || pending.count.is_some() {
                let count = pending
                    .count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(d)
                    .min(9999);
                return KeyStep {
                    pending: Pending {
                        count: Some(count),
                        keys: Vec::new(),
                    },
                    call: None,
                };
            }
        }
    }
    let mut keys = pending.keys.clone();
    keys.push(chord.to_owned());
    let seq = keys.join(" ");
    if let Some(name) = map.get(&seq) {
        let Some(mut call) = call_of(name) else {
            return none();
        };
        if let Some(n) = pending.count {
            call.count = Some(call.count.unwrap_or(1).saturating_mul(n));
        }
        return KeyStep {
            pending: Pending::default(),
            call: Some(call),
        };
    }
    if bound_from(&seq) {
        return KeyStep {
            pending: Pending {
                count: pending.count,
                keys,
            },
            call: None,
        };
    }
    none()
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
    /// The command it runs, as a key bound to it is saved (`done` for `tasks.done`); empty for `app.commandLine`.
    pub command: String,
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
    /// The bindings that run no command, skipped, for a warning with a way to take each out.
    pub bad: Vec<BadKey>,
}

/// A layer of keys, read: each action and its keys.
type Layer = Vec<(String, Vec<String>)>;

/// Every section of a Keybindings page, from the keymap asset and the whole of `settings.json`.
pub fn bindings(defaults: &Value, settings: &Value) -> Bindings {
    let migrated = crate::settings_version::migrate(settings, defaults);
    let settings = migrated.as_ref().unwrap_or(settings);
    let root = settings.as_object().cloned().unwrap_or_default();
    let known = known_actions(defaults);
    // What an action does; for a command with its words (`filter +home`), what the action it runs does.
    let about = |name: &str| {
        let line = |n: &str| defaults.get("about").and_then(|a| a.get(n)).and_then(Value::as_str);
        line(name)
            .or_else(|| call_of(name).and_then(|c| line(&c.action)))
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
        // What the user bound here, by what it runs: the keys bound to it, and `[]` for an action whose default key
        // they unbound and gave it no other. A key that cannot be read is kept as written so it can be seen and taken out.
        let mut set: Layer = Vec::new();
        let mut add = |name: &str, key: Option<String>| {
            let i = match set.iter().position(|(n, _)| n == name) {
                Some(i) => i,
                None => {
                    set.push((name.to_owned(), Vec::new()));
                    set.len() - 1
                }
            };
            if let Some(k) = key {
                if !set[i].1.contains(&k) {
                    set[i].1.push(k);
                }
            }
        };
        for (group, entries) in section_of_settings(&root, platform).into_iter().flatten() {
            for (text, v) in entries.as_object().into_iter().flatten() {
                let key = parse(text).unwrap_or_else(|_| text.clone());
                match v {
                    // One that runs no command is warned of instead (`bad`).
                    Value::String(runs) if check(runs, &[group.as_str()], Some(&known)).is_err() => {}
                    Value::String(runs) => add(&row_name(runs, group), Some(key)),
                    Value::Null => {
                        let unbound = names.iter().find(|(n, ks)| group_of(n) == *group && ks.contains(&key));
                        if let Some((n, _)) = unbound {
                            add(&n.clone(), None);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut actions: Vec<Binding> = set
            .iter()
            .map(|(name, keys)| Binding {
                action: name.clone(),
                about: about(name),
                command: preset_of(name).unwrap_or_else(|| name.clone()),
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
                    command: preset_of(&name).unwrap_or_default(),
                    action: name,
                    defaults,
                    keys: None,
                }),
        );
        sections.push(BindingSection { id, actions });
    }
    let mut problems: Vec<String> = Vec::new();
    let mut bad: Vec<BadKey> = Vec::new();
    let mut said: BTreeSet<String> = BTreeSet::new();
    for p in Platform::ALL {
        let m = resolve(defaults, settings, p);
        for b in m.bad {
            let at = if b.section == "general" {
                "keys".to_owned()
            } else {
                format!("{}.keys", b.section)
            };
            said.insert(bad_line(&at, &b.group, &b.key, &b.runs, &b.why));
            if !bad.contains(&b) {
                bad.push(b);
            }
        }
        for line in m.problems {
            if !problems.contains(&line) && !said.contains(&line) {
                problems.push(line);
            }
        }
    }
    Bindings {
        sections,
        problems,
        bad,
    }
}

/// Every action the keymap asset names, of any front end.
pub fn known_actions(defaults: &Value) -> BTreeSet<String> {
    let mut known: BTreeSet<String> = BTreeSet::new();
    for v in std::iter::once(defaults.get("keys")).chain(Platform::ALL.map(|p| own_keys(defaults, p))) {
        known.extend(v.and_then(Value::as_object).into_iter().flat_map(Map::keys).cloned());
    }
    known
}

/// What a Keybindings page says of a command typed for a key: what it runs and does, and the places it can be bound in
/// (several for one that acts in several places, as `next`), or why it is no command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Checked {
    Runs {
        call: Call,
        about: String,
        places: Vec<Place>,
    },
    Problem {
        problem: String,
    },
}

/// A group a command can be bound in, and the action it runs there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    pub group: String,
    pub action: String,
}

/// [`Checked`] for `runs`, against the keymap asset `defaults`.
pub fn check_command(defaults: &Value, runs: &str) -> Checked {
    let known = known_actions(defaults);
    let call = check(runs, &[], Some(&known)).and_then(|c| {
        known
            .contains(&c.action)
            .then_some(c)
            .ok_or_else(|| format!("“{}” is no action", runs.trim()))
    });
    match call {
        Err(problem) => Checked::Problem { problem },
        Ok(call) => {
            let place = |a: &str| Place {
                group: section_of(a).to_owned(),
                action: a.to_owned(),
            };
            let mut places = vec![place(&call.action)];
            // A command alone that acts in several places can be bound in each.
            if call.args.is_empty() && call.count.is_none() {
                let first = crate::filter::split_words(runs).into_iter().next().unwrap_or_default();
                if let Some(c) = command_named(&first) {
                    for a in c.bare.iter().filter(|a| known.contains(**a) && **a != call.action) {
                        places.push(place(a));
                    }
                }
            }
            let about = defaults
                .get("about")
                .and_then(|a| a.get(&call.action))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Checked::Runs { call, about, places }
        }
    }
}

/// `settings` (brought to the latest version) with keys bound to `runs` in `section` (`general`, `web`, `mobile` or
/// `tui`): a command (`done`, `filter +home`) or an action's name, saved as given. It is bound in `group`, else the
/// group of what it runs. What the user bound there to the same thing (`done` and `tasks.done` alike) is replaced by
/// `keys`, and the default keys of the action it runs that are not among them are unbound (`null`). `None` takes out
/// what the user bound to it there, so it has its defaults again. A section or a front end's object left empty is
/// taken out; everything else in `settings` is kept.
pub fn with_binding(
    settings: &Value,
    defaults: &Value,
    section: &str,
    runs: &str,
    group: Option<&str>,
    keys: Option<&[String]>,
) -> Value {
    let migrated = crate::settings_version::migrate(settings, defaults).unwrap_or_else(|| settings.clone());
    let mut root = migrated.as_object().cloned().unwrap_or_default();
    let platform = Platform::parse(section);
    let home = group.map_or_else(|| group_of(runs), str::to_owned);
    let row = row_name(runs, &home);
    let own_defaults = default_keys(defaults, platform, &row);
    let mut map = section_of_settings(&root, platform).cloned().unwrap_or_default();
    for (g, entries) in &mut map {
        if let Some(entries) = entries.as_object_mut() {
            entries.retain(|k, v| {
                let runs_it = v.as_str().is_some_and(|v| row_name(v, g) == row);
                let unbinds_its_own = v.is_null() && *g == home && own_defaults.contains(k);
                !runs_it && !unbinds_its_own
            });
        }
    }
    if let Some(list) = keys {
        if let Some(entries) = map
            .entry(home)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
        {
            for k in list {
                entries.insert(k.clone(), Value::String(runs.trim().to_owned()));
            }
            for d in &own_defaults {
                if !list.contains(d) {
                    entries.entry(d.clone()).or_insert(Value::Null);
                }
            }
        }
    }
    map.retain(|_, v| v.as_object().is_none_or(|m| !m.is_empty()));
    put_section(&mut root, platform, map);
    Value::Object(root)
}

/// `settings` without one binding: `key`, as written, in `group` of `section` (a [`BadKey`] taken out).
pub fn without_key(settings: &Value, defaults: &Value, section: &str, group: &str, key: &str) -> Value {
    let migrated = crate::settings_version::migrate(settings, defaults).unwrap_or_else(|| settings.clone());
    let mut root = migrated.as_object().cloned().unwrap_or_default();
    let platform = Platform::parse(section);
    let mut map = section_of_settings(&root, platform).cloned().unwrap_or_default();
    if let Some(entries) = map.get_mut(group).and_then(Value::as_object_mut) {
        entries.remove(key);
    }
    map.retain(|_, v| v.as_object().is_none_or(|m| !m.is_empty()));
    put_section(&mut root, platform, map);
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
                // Two shadows are meant: a section's own add (a task command, a kanban column, a key) wins over adding
                // a task, which every other page of the terminal gives `a`; and on Settings, `u` reverts the page's
                // edits rather than undoing a task's change (spec 0033).
                let own: Vec<&String> = m
                    .keys
                    .iter()
                    .filter(|(n, _)| groups.first() == Some(&section_of(n)))
                    .flat_map(|(_, k)| k)
                    .collect();
                let own = &own;
                let mut all: Vec<&String> = m
                    .keys
                    .iter()
                    .filter(|(n, _)| groups.contains(&section_of(n)))
                    .flat_map(|(n, k)| k.iter().filter(move |k| *n != "app.add" || !own.contains(k)))
                    .filter(|k| {
                        !(groups.contains(&"settings")
                            && k.as_str() == "u"
                            && m.keys.get("settings.revert").is_some_and(|r| r.contains(k)))
                    })
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
        // `:` opens it with `:` typed, for a command (spec 0020).
        assert_eq!(tui.action("tasks", ":"), Some("app.commandLine"));
        assert_eq!(tui.action("tasks", "f"), Some("tasks.filter"));
    }

    #[test]
    fn shared_keys_then_the_front_ends_own() {
        let s = json!({
            "version": 3,
            "keys": { "app": { "ctrl+k": "app.prompt", "U": "app.stopTimer" } },
            "tui": { "keys": { "app": { ":": "app.prompt", "ctrl+k": null, "/": null } } },
            "web": { "keys": { "app": { "/": null, "ctrl+k": null } } }
        });
        let tui = km(&s, Platform::Tui);
        assert_eq!(tui.keys_of("app.prompt"), [":"]);
        assert!(
            tui.keys_of("app.commandLine").is_empty(),
            "`:` was its, in the same group"
        );
        // A key bound adds to the action's defaults; only a key bound to null takes one away.
        assert_eq!(tui.keys_of("app.stopTimer"), ["T", "U"]);
        assert_eq!(tui.action("tasks", "/"), None);
        let web = km(&s, Platform::Web);
        assert!(web.keys_of("app.prompt").is_empty());
        assert_eq!(web.problems, Vec::<String>::new());
        assert!(
            !web.keys.contains_key("app.stopTimer"),
            "the web app has no stop-timer key"
        );
        let mobile = km(&s, Platform::Mobile);
        assert_eq!(mobile.keys_of("app.prompt"), ["/", "ctrl+k"]);
        assert_eq!(mobile.action("app", "ctrl+k"), Some("app.prompt"));
    }

    #[test]
    fn a_key_given_to_one_action_leaves_the_default_that_had_it() {
        // `d` was done's; given to delete, done is left with none rather than a clash.
        let m = km(
            &json!({ "version": 3, "tui": { "keys": { "tasks": { "d": "tasks.delete" } } } }),
            Platform::Tui,
        );
        assert_eq!(m.action("tasks", "d"), Some("tasks.delete"));
        assert!(m.keys_of("tasks.done").is_empty());
        // Burndown's `d` acts elsewhere, so it keeps it.
        assert_eq!(m.action("burndown", "d"), Some("burndown.daily"));
        assert_eq!(m.problems, Vec::<String>::new());
        // A key of `app` comes before a page's own `d` and takes it; a page's key leaves `app`'s alone elsewhere.
        let m = km(
            &json!({ "version": 3, "keys": { "app": { "d": "app.undo" }, "tasks": { "q": "tasks.start" } } }),
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
            "version": 3,
            "keys": {
                "tasks": { "z": "tasks.nope", "hyper+d": "tasks.done", "x": 3 },
                "nowhere": { "a": "app.help" }
            },
            "tui": { "keys": { "tasks": { "a": "tasks.start" } } },
            "web": { "keys": { "tasks": { "e": "tasks.export" } } },
            "mobile": { "keys": [] }
        });
        let tui = km(&s, Platform::Tui);
        for line in [
            "keys: “nowhere” is no group of keys",
            "keys.tasks: “hyper+d” is not a key",
            "keys.tasks: 3 is not an action",
            "keys.tasks: z runs “tasks.nope”, which is skipped: “tasks.nope” is no action",
        ] {
            assert!(tui.problems.contains(&line.to_owned()), "{line}: {:?}", tui.problems);
        }
        assert_eq!(tui.problems.len(), 4, "{:?}", tui.problems);
        assert_eq!(tui.action("tasks", "a"), Some("tasks.start"));
        let web = km(&s, Platform::Web);
        assert!(web
            .problems
            .contains(&"web.keys: the web app has no “tasks.export”".to_owned()));
        let mobile = km(&s, Platform::Mobile);
        assert!(mobile
            .problems
            .contains(&"mobile.keys: not an object of groups and their keys".to_owned()));
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
            "version": 3,
            "keys": {
                "app": { "ctrl+k": "app.prompt", "Ctrl-J": "app.prompt", "/": null },
                "tasks": { "hyper+d": "tasks.done" }
            },
            "tui": { "keys": { "tasks": { "d": null, "z": "tasks.nope" } } },
        });
        let b = bindings(&defaults(), &s);
        let ids: Vec<&str> = b.sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, SECTIONS);
        let general = &b.sections[0].actions;
        assert_eq!(general[0].action, "app.prompt");
        assert_eq!(general[0].keys, Some(vec!["ctrl+shift+j".into(), "ctrl+k".into()]));
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
            !web.iter().any(|(a, _)| *a == "tasks.export"),
            "only the web app's actions"
        );
        let tui = &b.sections[3].actions;
        assert_eq!(
            (tui[0].action.as_str(), tui[0].keys.as_deref()),
            ("tasks.done", Some(&[][..]))
        );
        assert!(!tui.iter().any(|a| a.action == "tasks.nope"), "warned of, not listed");
        assert_eq!(
            b.bad,
            [BadKey {
                section: "tui".into(),
                group: "tasks".into(),
                key: "z".into(),
                runs: "tasks.nope".into(),
                why: "“tasks.nope” is no action".into(),
            }]
        );
        assert_eq!(tui[0].command, "done", "the command it runs");
        let bound = bindings(
            &defaults(),
            &json!({ "version": 3, "keys": { "tasks": { "g h": "filter +home" } } }),
        );
        let cmd = &bound.sections[0].actions[0];
        assert_eq!(
            (cmd.action.as_str(), cmd.keys.as_deref()),
            ("filter +home", Some(&["g h".to_owned()][..]))
        );
        assert_eq!(
            cmd.about,
            defaults()["about"]["tasks.filter"].as_str().unwrap(),
            "what its action does"
        );
        let modify = tui.iter().find(|a| a.action == "tasks.modify").unwrap();
        assert_eq!(modify.defaults, ["m", "M"]);
        assert!(b.problems.contains(&"keys.tasks: “hyper+d” is not a key".to_owned()));
        assert!(
            !b.problems.iter().any(|p| p.contains("tasks.nope")),
            "a warning, not a problem"
        );
        assert_eq!(
            b.problems.iter().filter(|p| p.contains("hyper+d")).count(),
            1,
            "said once"
        );
    }

    #[test]
    fn a_key_is_set_and_taken_out_and_nothing_else_moves() {
        let d = defaults();
        let s = json!({ "version": 7, "taskCommands": [1], "web": { "theme": "dark" } });
        let one = with_binding(&s, &d, "web", "app.prompt", None, Some(&["ctrl+k".into()]));
        // The key runs the action, in its group; the default key it no longer has is unbound.
        assert_eq!(
            one,
            json!({ "version": 7, "taskCommands": [1],
                    "web": { "theme": "dark", "keys": { "app": { "ctrl+k": "app.prompt", "/": null } } } })
        );
        assert_eq!(km(&one, Platform::Web).keys_of("app.prompt"), ["ctrl+k"]);
        let two = with_binding(&one, &d, "general", "app.undo", None, Some(&["u".into(), "U".into()]));
        assert_eq!(two["keys"], json!({ "app": { "u": "app.undo", "U": "app.undo" } }));
        let none = with_binding(&two, &d, "tui", "tasks.done", None, Some(&[]));
        assert_eq!(none["tui"], json!({ "keys": { "tasks": { "d": null } } }));
        assert!(km(&none, Platform::Tui).keys_of("tasks.done").is_empty());
        let back = with_binding(
            &with_binding(&none, &d, "tui", "tasks.done", None, None),
            &d,
            "general",
            "app.undo",
            None,
            None,
        );
        let back = with_binding(&back, &d, "web", "app.prompt", None, None);
        assert_eq!(back, s);
        // An old file is brought up to date first.
        assert_eq!(
            with_binding(
                &json!({ "keys": { "app.undo": "U" } }),
                &d,
                "general",
                "app.help",
                None,
                None
            ),
            json!({ "version": 7, "keys": { "app": { "U": "app.undo", "u": null } } })
        );
    }

    #[test]
    fn no_defaults_is_no_keys() {
        let m = resolve(
            &Value::Null,
            &json!({ "version": 3, "keys": { "app": { "/": "app.prompt" } } }),
            Platform::Web,
        );
        assert!(m.keys.is_empty() && m.contexts.is_empty());
        assert_eq!(m.problems, ["keys: “app” is no group of keys"]);
    }

    #[test]
    fn a_sequence_is_keys_one_after_another() {
        let p = |s: &str| parse(s).unwrap();
        assert_eq!(p("g g"), "g g");
        assert_eq!(p("gg"), "g g");
        assert_eq!(p("g1"), "g 1");
        assert_eq!(p("gT"), "g T");
        assert_eq!(p("Ctrl-K Ctrl-S"), "ctrl+shift+k ctrl+shift+s");
        assert_eq!(p("up"), "up", "a name is one key");
        assert!(parse("enterr").is_err(), "longer words are names, misspelt");
        assert!(parse("g nope").is_err());
        assert_eq!(display("g g"), "gg");
        assert_eq!(display("g 1"), "g1");
        assert_eq!(display("ctrl+k ctrl+s"), "Ctrl+K Ctrl+S");
    }

    #[test]
    fn commands_are_vims_with_vims_short_forms() {
        let c = |s: &str| call_of(s).map(|c| (c.action, c.args, c.count));
        let plain = |a: &str| Some((a.to_owned(), Vec::<String>::new(), None));
        for w in ["tabn", "tabne", "tabnext"] {
            assert_eq!(c(w), plain("page.next"), "{w}");
        }
        assert_eq!(c("tab"), None, "shorter than vim allows");
        assert_eq!(c("tabn 3"), Some(("page.go".into(), vec!["3".into()], None)));
        assert_eq!(c("tabn Tasks"), Some(("page.go".into(), vec!["Tasks".into()], None)));
        assert_eq!(c("tabN"), plain("page.prev"), "vim's tabN is the page before");
        assert_eq!(c("tabp"), plain("page.prev"));
        assert_eq!(c("tabl"), plain("page.last"));
        assert_eq!(c("tabfir"), plain("page.first"));
        assert_eq!(c("q"), plain("app.quit"));
        assert_eq!(c("u"), plain("app.undo"));
        assert_eq!(c("w"), plain("app.sync"));
        assert_eq!(c("e"), plain("app.reload"));
        assert_eq!(c("d"), plain("tasks.delete"), "vim's d is delete");
        assert_eq!(c("do"), plain("tasks.done"));
        assert_eq!(c("st"), None, "start or stop: not enough to say");
        assert_eq!(c("sta"), plain("tasks.start"));
        assert_eq!(c("sto"), plain("tasks.stop"));
        assert_eq!(c("a"), None, "add or annotate");
        assert_eq!(
            c("filter 'project:big house' +home"),
            Some((
                "tasks.filter".into(),
                vec!["project:big house".into(), "+home".into()],
                None
            ))
        );
        assert_eq!(c("5"), Some(("move.row".into(), vec!["5".into()], None)));
        assert_eq!(c("$"), plain("move.last"));
        assert_eq!(c("+3"), Some(("move.down".into(), vec![], Some(3))));
        assert_eq!(c("-3"), Some(("move.up".into(), vec![], Some(3))));
        assert_eq!(c("0"), None);
        assert_eq!(c("tasks.done"), plain("tasks.done"), "an action's own name");
        assert_eq!(c("nope"), None);
        assert_eq!(c(""), None);
    }

    #[test]
    fn the_commands_described_for_people_are_these() {
        let doc: Value = serde_json::from_str(include_str!("../../../web/public/keymaps/commands.json")).unwrap();
        let described: Vec<(&str, u64)> = doc["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["name"].as_str().unwrap(), c["shortest"].as_u64().unwrap()))
            .collect();
        let here: Vec<(&str, u64)> = COMMANDS.iter().map(|c| (c.name, c.shortest as u64)).collect();
        assert_eq!(described, here, "web/public/keymaps/commands.json");
        for (c, d) in COMMANDS.iter().zip(doc["commands"].as_array().unwrap()) {
            let takes = c.words != Words::None || !c.subs.is_empty();
            assert_eq!(!d["takes"].as_str().unwrap().is_empty(), takes, "{}: its takes", c.name);
        }
    }

    #[test]
    fn a_colon_runs_a_command_only_first_and_joined_to_it() {
        let a = |s: &str| command_line(s).map(|c| c.action);
        assert_eq!(a(":tabn Tasks").as_deref(), Some("page.go"));
        assert_eq!(a(":5").as_deref(), Some("move.row"));
        assert_eq!(a(":done").as_deref(), Some("tasks.done"));
        // Everything else goes to `task` as it is.
        assert_eq!(a(": tabn"), None);
        assert_eq!(a(":nope"), None);
        assert_eq!(a(":"), None);
        assert_eq!(a("tabn"), None);
        assert_eq!(a("list :done"), None);
        assert_eq!(a("!(+home)"), None);
    }

    #[test]
    fn keys_wait_for_the_rest_of_a_sequence_and_take_a_count() {
        let m = km(&json!({}), Platform::Tui);
        let mut p = Pending::default();
        fn press_on(m: &Keymap, p: &mut Pending, k: &str) -> Option<(String, Option<u32>)> {
            let s = m.step("tasks", p, k);
            *p = s.pending;
            s.call.map(|c| (c.action, c.count))
        }
        assert_eq!(press_on(&m, &mut p, "5"), None);
        assert_eq!(press_on(&m, &mut p, "j"), Some(("move.down".into(), Some(5))));
        assert_eq!(press_on(&m, &mut p, "1"), None);
        assert_eq!(press_on(&m, &mut p, "2"), None);
        assert_eq!(p.label(), "12");
        assert_eq!(press_on(&m, &mut p, "G"), Some(("move.last".into(), Some(12))));
        assert_eq!(press_on(&m, &mut p, "g"), None);
        assert_eq!(p.label(), "g");
        assert_eq!(press_on(&m, &mut p, "g"), Some(("move.first".into(), None)));
        assert_eq!(press_on(&m, &mut p, "g"), None);
        assert_eq!(
            press_on(&m, &mut p, "3"),
            Some(("page.tags".into(), None)),
            "a digit after g is the key, not a count"
        );
        assert_eq!(press_on(&m, &mut p, "g"), None);
        assert_eq!(press_on(&m, &mut p, "esc"), None);
        assert!(p.is_empty(), "Esc drops what was pending");
        assert_eq!(press_on(&m, &mut p, "j"), Some(("move.down".into(), None)));
        assert_eq!(press_on(&m, &mut p, "0"), None);
        assert!(p.is_empty(), "0 alone is no count");
        assert_eq!(press_on(&m, &mut p, "g"), None);
        assert_eq!(press_on(&m, &mut p, "z"), None);
        assert!(p.is_empty(), "a sequence bound to nothing is dropped");
    }

    #[test]
    fn a_key_can_run_an_expression() {
        let s = json!({ "version": 3, "tui": { "keys": {
            "tasks": { "g h": "filter +home", "z": "frobnicate 3" },
            "page": { "T": "tabn Tasks" }
        } } });
        let m = km(&s, Platform::Tui);
        let step = |p: &Pending, k: &str| m.step("tasks", p, k);
        let g = step(&Pending::default(), "g");
        let call = step(&g.pending, "h").call.unwrap();
        assert_eq!(
            (call.action.as_str(), call.args.as_slice()),
            ("tasks.filter", &["+home".to_owned()][..])
        );
        // `tabn Tasks` is a page's, so it acts wherever the pages' keys do.
        assert_eq!(m.action("summary", "T"), Some("tabn Tasks"));
        assert_eq!(m.action("summary", "g h"), None, "filter is the Tasks page's");
        assert!(
            m.problems.contains(
                &"tui.keys.tasks: z runs “frobnicate 3”, which is skipped: “frobnicate” is no command".to_owned()
            ),
            "{:?}",
            m.problems
        );
    }

    #[test]
    fn a_key_that_starts_a_longer_one_is_said() {
        let m = km(
            &json!({ "version": 3, "tui": { "keys": { "move": { "g": "move.first" } } } }),
            Platform::Tui,
        );
        assert!(
            m.problems
                .iter()
                .any(|p| p.starts_with("keys: g (move.first) is the start of g0 (page.settings)")),
            "{:?}",
            m.problems
        );
    }

    #[test]
    fn a_malformed_command_says_why() {
        let why = |s: &str| check(s, &[], None).err();
        assert_eq!(why("done 3").as_deref(), Some("done takes no words"));
        assert_eq!(why("lock now").as_deref(), Some("lock takes “pin” or “key”"));
        assert_eq!(why("lock pin"), None);
        assert_eq!(
            why("burndown").as_deref(),
            Some("burndown needs a word: “day”, “week” or “month”")
        );
        assert_eq!(
            why("info nope").as_deref(),
            Some("info takes “details”, “recurrence”, “timelog”, “annotations”, “unknown”, “raw”, “modify” or “links”")
        );
        assert_eq!(
            why("st").as_deref(),
            Some("“st” is not enough to say which: “start”, “stop” or “stopall”")
        );
        assert_eq!(why("frob").as_deref(), Some("“frob” is no command"));
        assert_eq!(
            why("tabn 12").as_deref(),
            Some("tabnext takes a page's number (1 to 9) or the start of its name")
        );
        assert_eq!(why("tasks.done 3").as_deref(), Some("tasks.done takes no words"));
        assert_eq!(why("tasks.filter +home"), None, "its command takes words");
        assert!(why("sort due").is_some_and(|w| w.contains("no change of the sort")));
        assert_eq!(why("  ").as_deref(), Some("an empty command"));
        let known: BTreeSet<String> = ["tasks.done".to_owned()].into();
        assert_eq!(
            check("tasks.nope", &[], Some(&known)).err().as_deref(),
            Some("“tasks.nope” is no action")
        );
    }

    #[test]
    fn words_run_actions_of_their_own() {
        let a = |s: &str| call_of(s).map(|c| (c.action, c.args));
        let alone = |x: &str| Some((x.to_owned(), Vec::<String>::new()));
        assert_eq!(a("info details"), alone("info.details"));
        assert_eq!(a("info"), alone("tasks.info"));
        assert_eq!(a("mark all"), alone("tasks.markAll"));
        assert_eq!(a("delete completed"), alone("tasks.clearCompleted"));
        assert_eq!(a("burndown week"), alone("burndown.weekly"));
        assert_eq!(a("sort raise"), alone("sort.raise"));
        assert_eq!(a("sort ="), alone("sort.reverse"));
        assert_eq!(a("sort"), alone("tasks.sort"));
        assert_eq!(
            a("sort due- project--"),
            Some(("tasks.sort".into(), vec!["due-".into(), "project--".into()]))
        );
        assert_eq!(
            a("sort replace due+,urgency-"),
            Some(("tasks.sort".into(), vec!["replace".into(), "due+,urgency-".into()]))
        );
        assert_eq!(a("edit settings"), alone("settings.editFile"));
        assert_eq!(
            a("1"),
            alone("move.first"),
            "row 1 is the first, so 5 before it is row 5"
        );
    }

    #[test]
    fn a_command_of_several_places_runs_the_one_where_it_is() {
        let a = |s: &str, g: &[&str]| check(s, g, None).map(|c| c.action).ok();
        assert_eq!(a("next", &["calendar", "app"]).as_deref(), Some("calendar.next"));
        assert_eq!(a("next", &["boards", "app"]).as_deref(), Some("boards.next"));
        assert_eq!(
            a("next", &[]).as_deref(),
            Some("calendar.next"),
            "the first where none is given"
        );
        assert_eq!(a("back", &["boards"]).as_deref(), Some("boards.close"));
        assert_eq!(a("open", &["lists", "app"]).as_deref(), Some("lists.open"));
        assert_eq!(a("report", &["sidebar", "app"]).as_deref(), Some("sidebar.open"));
        // A binding acts in its group; a `:` line where its context's groups do.
        let s = json!({ "version": 3, "tui": { "keys": { "calendar": { "n": "next" }, "boards": { "n": "next" } } } });
        let m = km(&s, Platform::Tui);
        assert_eq!(m.action("calendar", "n"), Some("calendar.next"));
        assert_eq!(m.action("boards", "n"), Some("boards.next"));
        let groups: Vec<&str> = m.groups["boards"].iter().map(String::as_str).collect();
        assert_eq!(
            command_line_in(":next", &groups).map(|c| c.action).as_deref(),
            Some("boards.next")
        );
    }

    #[test]
    fn every_action_has_a_command_that_runs_it() {
        let d = defaults();
        for action in known_actions(&d) {
            if action == "app.commandLine" {
                assert_eq!(preset_of(&action), None, "the : itself");
                continue;
            }
            let preset = preset_of(&action).unwrap_or_else(|| panic!("{action} has no command"));
            let c = check(&preset, &[section_of(&action)], Some(&known_actions(&d)))
                .unwrap_or_else(|e| panic!("{action}: {preset}: {e}"));
            let page = action.strip_prefix("page.").filter(|p| PAGES.contains(p));
            match page {
                Some(p) => assert_eq!((c.action.as_str(), c.args.as_slice()), ("page.go", &[p.to_owned()][..])),
                None => assert_eq!(c.action, action, "{preset}"),
            }
        }
    }

    #[test]
    fn every_action_described_is_one_a_key_can_run() {
        let d = defaults();
        let known = known_actions(&d);
        let about = d
            .get("about")
            .and_then(Value::as_object)
            .expect("the asset describes its actions");
        for action in about.keys() {
            assert!(
                known.contains(action),
                "{action} is described but listed under no front end's keys"
            );
        }
        assert!(matches!(check_command(&d, "lock"), Checked::Runs { .. }));
    }

    #[test]
    fn the_short_forms_name_one_command_each() {
        for c in &COMMANDS {
            for n in c.shortest..=c.name.len() {
                let w = &c.name[..n];
                assert_eq!(command_named(w).map(|x| x.name), Some(c.name), "{w}");
            }
        }
    }

    #[test]
    fn a_key_is_saved_as_its_command_and_old_names_still_read() {
        let d = defaults();
        let s = json!({ "version": 3, "tui": { "keys": { "tasks": { "x": "tasks.done" } } } });
        assert_eq!(
            km(&s, Platform::Tui).action("tasks", "x"),
            Some("tasks.done"),
            "an action's name still works"
        );
        let one = with_binding(&s, &d, "tui", "done", None, Some(&["x".into(), "y".into()]));
        assert_eq!(
            one["tui"]["keys"]["tasks"],
            json!({ "x": "done", "y": "done", "d": null }),
            "tasks.done and done are one: rewritten as the command, the default unbound"
        );
        let m = km(&one, Platform::Tui);
        assert_eq!(m.keys_of("tasks.done"), ["x", "y"]);
        let b = bindings(&d, &one);
        let row = b.sections[3].actions.iter().find(|a| a.action == "tasks.done").unwrap();
        assert_eq!(row.keys, Some(vec!["x".into(), "y".into()]));
        // A command of several places is bound in the group given.
        let next = with_binding(&s, &d, "tui", "next", Some("boards"), Some(&["N".into()]));
        assert_eq!(
            next["tui"]["keys"]["boards"],
            json!({ "N": "next", "l": null, "right": null })
        );
        assert_eq!(km(&next, Platform::Tui).action("boards", "N"), Some("boards.next"));
        // Taking out what runs it, by its row.
        let back = with_binding(&one, &d, "tui", "tasks.done", None, None);
        assert_eq!(back["tui"], json!(null), "{back}");
    }

    #[test]
    fn a_skipped_key_can_be_taken_out() {
        let d = defaults();
        let s = json!({ "version": 7, "keys": { "tasks": { "z": "done 3", "x": "done" } } });
        let b = bindings(&d, &s);
        assert_eq!(b.bad.len(), 1, "{:?}", b.bad);
        assert_eq!(
            (b.bad[0].key.as_str(), b.bad[0].why.as_str()),
            ("z", "done takes no words")
        );
        let out = without_key(&s, &d, "general", "tasks", "z");
        assert_eq!(out, json!({ "version": 7, "keys": { "tasks": { "x": "done" } } }));
        assert!(bindings(&d, &out).bad.is_empty());
    }

    #[test]
    fn a_command_typed_for_a_key_is_checked() {
        let d = defaults();
        let ok = serde_json::to_value(check_command(&d, "filter +home")).unwrap();
        assert_eq!(ok["call"], json!({ "action": "tasks.filter", "args": ["+home"] }));
        assert_eq!(ok["places"], json!([{ "group": "tasks", "action": "tasks.filter" }]));
        assert_eq!(ok["about"], d["about"]["tasks.filter"]);
        let next = serde_json::to_value(check_command(&d, "next")).unwrap();
        assert_eq!(
            next["places"],
            json!([
                { "group": "calendar", "action": "calendar.next" },
                { "group": "boards", "action": "boards.next" },
                { "group": "kanban", "action": "kanban.next" }
            ]),
            "where it can be bound"
        );
        let bad = serde_json::to_value(check_command(&d, "done 3")).unwrap();
        assert_eq!(bad, json!({ "problem": "done takes no words" }));
        let nope = serde_json::to_value(check_command(&d, "tasks.nope")).unwrap();
        assert_eq!(nope, json!({ "problem": "“tasks.nope” is no action" }));
    }
}

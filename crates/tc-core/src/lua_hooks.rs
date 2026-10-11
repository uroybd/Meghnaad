//! Hooks written in Lua: a script of up to four functions, `on_launch(command)`, `on_add(task)`,
//! `on_modify(old, new)` and `on_exit(changed)`, run in a sandbox.
//!
//! [`LuaRunner`] is the one place the script is run. It takes JSON in and gives JSON out (a task is the plain table
//! `task_to_json` writes, see [`crate::hooks`]), so it does not matter who calls it: the Worker (which keeps the script
//! and, later, the secrets a script may use), the engine on Android (in the same process), or a test.
//!
//! The script gets a small library (`lua/prelude.lua`): a task object, tags as a set, dates and durations with
//! arithmetic, `say`, `warn` and `refuse`. It gets no files, no clock but the one it is handed, and no network. A run
//! is bounded: it stops after a budget of instructions and memory, so a script that loops cannot hold a command up.

use crate::dates::{format_vague, parse_date_expr, parse_duration, Clock, DateFormat};
use crate::hooks::{Kind, Line};
use omnilua::{Lua, Sandbox, SandboxConfig, Table, Value};
use serde_json::Value as Json;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const PRELUDE: &str = include_str!("lua/prelude.lua");

/// The most VM instructions one hook call may run (a few milliseconds' worth), and the most memory it may hold.
const INSTRUCTIONS: u64 = 5_000_000;
const MEMORY: usize = 32 * 1024 * 1024;

/// What a call to a hook came to when it did not finish: why, and whatever the script printed first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    pub message: String,
    pub lines: Vec<Line>,
}

/// What a call to a hook gave: the value it returned (a task, or null) and what it printed.
#[derive(Debug, Clone, PartialEq)]
pub struct Called {
    pub value: Json,
    pub lines: Vec<Line>,
}

/// What is wrong with a script, and where (a line of the script, when Lua says).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: Option<u32>,
    pub message: String,
}

/// The four hook points, by the name a script defines them under.
pub const HOOKS: [&str; 4] = ["on_launch", "on_add", "on_modify", "on_exit"];

pub struct LuaRunner {
    lua: Lua,
    sandbox: Sandbox,
    lines: Rc<RefCell<Vec<Line>>>,
    /// The time the script sees; [`set_clock`](Self::set_clock) moves it, so one runner serves many calls.
    clock: Rc<Cell<Clock>>,
}

impl std::fmt::Debug for LuaRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaRunner").finish_non_exhaustive()
    }
}

/// "hooks:12: message" (what Lua writes for an error in the script) as a line and a message.
fn problem_of(text: &str) -> Problem {
    let text = text.trim();
    let text = text.strip_prefix("runtime error: ").unwrap_or(text);
    let first = text.lines().next().unwrap_or(text);
    if let Some(rest) = first
        .strip_prefix("[string \"hooks\"]:")
        .or_else(|| first.strip_prefix("hooks:"))
    {
        if let Some((line, message)) = rest.split_once(':') {
            if let Ok(line) = line.trim().parse::<u32>() {
                return Problem {
                    line: Some(line),
                    message: message.trim().to_owned(),
                };
            }
        }
    }
    Problem {
        line: None,
        message: first.to_owned(),
    }
}

fn lua_err(e: omnilua::Error) -> Problem {
    problem_of(&e.to_string())
}

impl LuaRunner {
    /// Load `source`: the library, then the script, which defines the hook functions. `clock` is the time the script
    /// sees (`now()`, `today()`, how dates are read and written).
    pub fn load(source: &str, clock: Clock) -> Result<LuaRunner, Problem> {
        let mut config = SandboxConfig::strict();
        config.instruction_limit = Some(INSTRUCTIONS);
        config.memory_limit_bytes = Some(MEMORY);
        let (lua, sandbox) = Lua::sandboxed(config).map_err(lua_err)?;
        let lines: Rc<RefCell<Vec<Line>>> = Rc::default();
        let clock = Rc::new(Cell::new(clock));
        install_host(&lua, &clock, &lines).map_err(lua_err)?;
        lua.load(PRELUDE).set_name("prelude").exec().map_err(lua_err)?;
        lua.load(source).set_name("hooks").exec().map_err(lua_err)?;
        sandbox.reset();
        Ok(LuaRunner {
            lua,
            sandbox,
            lines,
            clock,
        })
    }

    /// From now on the script sees this time (`now()`, `today()`, the time zone and date formats it reads and writes).
    pub fn set_clock(&self, clock: Clock) {
        self.clock.set(clock);
    }

    /// Which of the four hooks the script defines.
    pub fn defined(&self) -> Vec<&'static str> {
        let globals = self.lua.globals();
        HOOKS
            .into_iter()
            .filter(|name| matches!(globals.get::<_, Value>(*name), Ok(Value::Function(_))))
            .collect()
    }

    /// Call a hook: `function` is one of [`HOOKS`], `args` are what it is given (`on_launch`: the command as text;
    /// `on_add`: a task; `on_modify`: the task as it was and as it would be; `on_exit`: a list of tasks).
    /// The value is what it returned: for `on_add` and `on_modify`, the task, as the plain table.
    ///
    /// (`async` so that a script can later call out to the network without any caller changing.)
    pub async fn call(&self, function: &str, args: &[Json]) -> Result<Called, Failed> {
        let entry = match function {
            "on_launch" => "__hook_launch",
            "on_add" => "__hook_add",
            "on_modify" => "__hook_modify",
            "on_exit" => "__hook_exit",
            other => {
                return Err(Failed {
                    message: format!("there is no hook called {other}"),
                    lines: Vec::new(),
                })
            }
        };
        self.lines.borrow_mut().clear();
        self.sandbox.reset();
        let result = self.invoke(entry, args);
        let lines = std::mem::take(&mut *self.lines.borrow_mut());
        match result {
            Ok(value) => Ok(Called { value, lines }),
            Err(message) => Err(Failed { message, lines }),
        }
    }

    fn invoke(&self, entry: &str, args: &[Json]) -> Result<Json, String> {
        let f: omnilua::Function = self.lua.globals().get(entry).map_err(|e| lua_err(e).message)?;
        let mut values = Vec::with_capacity(args.len());
        for a in args {
            values.push(to_lua(&self.lua, a).map_err(|e| lua_err(e).message)?);
        }
        let called: omnilua::Result<Value> = match values.len() {
            0 => f.call(()),
            1 => f.call(values.remove(0)),
            _ => {
                let second = values.remove(1);
                f.call((values.remove(0), second))
            }
        };
        match called {
            Ok(v) => to_json(&v, 0),
            Err(e) => {
                if self.sandbox.tripped().is_some() {
                    Err("A hook ran for too long (or used too much memory) and was stopped.".into())
                } else {
                    Err(problem_of(&e.to_string()).message)
                }
            }
        }
    }
}

/// Is this script all right to keep? It must load (the library, then the script), and the hook functions it defines
/// are named. What is wrong is reported with a line, for the editor.
pub fn check(source: &str) -> Result<Vec<&'static str>, Problem> {
    LuaRunner::load(source, Clock::utc(0)).map(|r| r.defined())
}

fn install_host(lua: &Lua, clock: &Rc<Cell<Clock>>, lines: &Rc<RefCell<Vec<Line>>>) -> omnilua::Result<()> {
    let host = lua.create_table()?;
    let c = Rc::clone(clock);
    host.set("now", lua.create_function(move |_, ()| Ok(c.get().now))?)?;
    let c = Rc::clone(clock);
    host.set(
        "parse_date",
        lua.create_function(move |_, text: String| Ok(parse_date_expr(&text, &c.get()).map(|d| d.ts)))?,
    )?;
    host.set(
        "parse_duration",
        lua.create_function(|_, text: String| Ok(parse_duration(&text)))?,
    )?;
    host.set(
        "format_duration",
        lua.create_function(|_, secs: f64| Ok(format_vague(secs as i64)))?,
    )?;
    let c = Rc::clone(clock);
    host.set(
        "parts",
        lua.create_function(move |lua, ts: i64| {
            let clock = c.get();
            let (y, m, d, h, mi, s) = clock.ymd_hms(ts);
            let t = lua.create_table()?;
            let parts = [
                i64::from(y),
                i64::from(m),
                i64::from(d),
                i64::from(h),
                i64::from(mi),
                i64::from(s),
                i64::from(clock.day_of_week(ts)),
            ];
            for (i, v) in parts.into_iter().enumerate() {
                t.set(i as i64 + 1, v)?;
            }
            Ok(t)
        })?,
    )?;
    let c = Rc::clone(clock);
    host.set(
        "from_parts",
        lua.create_function(move |_, p: Vec<i64>| {
            let clock = c.get();
            let part = |i: usize| p.get(i).and_then(|v| u32::try_from(*v).ok());
            Ok(
                match (
                    p.first().and_then(|v| i32::try_from(*v).ok()),
                    part(1),
                    part(2),
                    part(3),
                    part(4),
                    part(5),
                ) {
                    (Some(y), Some(m), Some(d), Some(h), Some(mi), Some(s)) => clock.from_ymd_hms(y, m, d, h, mi, s),
                    _ => None,
                },
            )
        })?,
    )?;
    let c = Rc::clone(clock);
    host.set(
        "format",
        lua.create_function(move |_, (ts, pattern): (i64, String)| Ok(DateFormat::new(&pattern).format(ts, &c.get())))?,
    )?;
    let c = Rc::clone(clock);
    host.set(
        "start_of",
        lua.create_function(move |_, (ts, unit): (i64, String)| {
            let clock = c.get();
            Ok(match unit.as_str() {
                "day" => Some(clock.start_of_day(ts)),
                "week" => Some(clock.start_of_week(ts)),
                "month" => Some(clock.start_of_month(ts)),
                "year" => Some(clock.start_of_year(ts)),
                _ => None,
            })
        })?,
    )?;
    let say = Rc::clone(lines);
    host.set(
        "say",
        lua.create_function(move |_, text: String| {
            say.borrow_mut().push(Line { kind: Kind::Info, text });
            Ok(())
        })?,
    )?;
    let warn = Rc::clone(lines);
    host.set(
        "warn",
        lua.create_function(move |_, text: String| {
            warn.borrow_mut().push(Line { kind: Kind::Warn, text });
            Ok(())
        })?,
    )?;
    lua.globals().set("__host", host)
}

/// JSON as a Lua value: objects and arrays become tables, null is nil.
fn to_lua(lua: &Lua, v: &Json) -> omnilua::Result<Value> {
    Ok(match v {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::Boolean(*b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Number(n.as_f64().unwrap_or(0.0)),
        },
        Json::String(s) => Value::String(lua.create_string(s)?),
        Json::Array(items) => {
            let t = lua.create_table()?;
            for (i, item) in items.iter().enumerate() {
                t.set(i as i64 + 1, to_lua(lua, item)?)?;
            }
            Value::Table(t)
        }
        Json::Object(map) => {
            let t = lua.create_table()?;
            for (k, item) in map {
                t.set(k.as_str(), to_lua(lua, item)?)?;
            }
            Value::Table(t)
        }
    })
}

/// A Lua value as JSON: a table that is a list (keys 1, 2, 3, …) becomes an array, any other an object. An empty table
/// is an array; whoever reads a task back accepts that where an object is meant (see `task_from_json`).
fn to_json(v: &Value, depth: usize) -> Result<Json, String> {
    if depth > 16 {
        return Err("a hook returned something nested too deeply".into());
    }
    Ok(match v {
        Value::Nil => Json::Null,
        Value::Boolean(b) => Json::Bool(*b),
        Value::Integer(i) => Json::from(*i),
        Value::Number(n) => {
            if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 {
                Json::from(*n as i64)
            } else {
                serde_json::Number::from_f64(*n).map_or(Json::Null, Json::Number)
            }
        }
        Value::String(s) => Json::String(s.to_str().map_err(|e| e.to_string())?),
        Value::Table(t) => table_to_json(t, depth)?,
        other => return Err(format!("a hook returned something that is not data ({other:?})")),
    })
}

fn table_to_json(t: &Table, depth: usize) -> Result<Json, String> {
    let pairs = t.raw_pairs().map_err(|e| e.to_string())?;
    let mut list: Vec<Option<Json>> = vec![None; pairs.len()];
    let mut is_list = true;
    for (k, v) in &pairs {
        match k {
            Value::Integer(i) if *i >= 1 && (*i as usize) <= pairs.len() => {
                list[*i as usize - 1] = Some(to_json(v, depth + 1)?);
            }
            _ => {
                is_list = false;
                break;
            }
        }
    }
    if is_list && list.iter().all(Option::is_some) {
        return Ok(Json::Array(list.into_iter().flatten().collect()));
    }
    let mut map = serde_json::Map::new();
    for (k, v) in &pairs {
        let key = match k {
            Value::String(s) => s.to_str().map_err(|e| e.to_string())?,
            Value::Integer(i) => i.to_string(),
            _ => return Err("a hook returned a table with a key that is not text or a number".into()),
        };
        map.insert(key, to_json(v, depth + 1)?);
    }
    Ok(Json::Object(map))
}

/// A script as engine hooks, run in this process: how Android uses hooks, and how the tests do. (On the web the
/// engine sends each call to the Worker instead, where a [`LuaRunner`] answers it.)
#[derive(Debug)]
pub struct LuaHooks {
    runner: LuaRunner,
    defined: Vec<&'static str>,
}

impl LuaHooks {
    pub fn load(source: &str, clock: Clock) -> Result<LuaHooks, Problem> {
        let runner = LuaRunner::load(source, clock)?;
        let defined = runner.defined();
        Ok(LuaHooks { runner, defined })
    }

    async fn call(
        &self,
        h: &mut crate::hooks::Hooked,
        name: &str,
        args: &[Json],
    ) -> Result<Json, crate::hooks::Reject> {
        if !self.defined.contains(&name) {
            return Ok(Json::Null);
        }
        let say = |h: &mut crate::hooks::Hooked, lines: Vec<Line>| {
            for l in lines {
                match l.kind {
                    Kind::Info => h.say(l.text),
                    Kind::Warn => h.warn(l.text),
                }
            }
        };
        match self.runner.call(name, args).await {
            Ok(done) => {
                say(h, done.lines);
                Ok(done.value)
            }
            Err(failed) => {
                say(h, failed.lines);
                Err(failed.message)
            }
        }
    }
}

#[async_trait::async_trait(?Send)]
impl crate::hooks::HookHost for LuaHooks {
    async fn on_launch(&self, h: &mut crate::hooks::Hooked, command: &str) -> Result<(), crate::hooks::Reject> {
        self.call(h, "on_launch", &[Json::String(command.to_owned())])
            .await
            .map(|_| ())
    }

    async fn on_add(
        &self,
        h: &mut crate::hooks::Hooked,
        task: crate::model::Facts,
    ) -> Result<crate::model::Facts, crate::hooks::Reject> {
        let value = self.call(h, "on_add", &[crate::hooks::task_to_json(&task)]).await?;
        if value.is_null() {
            return Ok(task);
        }
        crate::hooks::task_from_json(&value, &task)
    }

    async fn on_modify(
        &self,
        h: &mut crate::hooks::Hooked,
        old: &crate::model::Facts,
        new: crate::model::Facts,
    ) -> Result<crate::model::Facts, crate::hooks::Reject> {
        let value = self
            .call(
                h,
                "on_modify",
                &[crate::hooks::task_to_json(old), crate::hooks::task_to_json(&new)],
            )
            .await?;
        if value.is_null() {
            return Ok(new);
        }
        crate::hooks::task_from_json(&value, &new)
    }

    async fn on_exit(&self, h: &mut crate::hooks::Hooked, changed: &[crate::model::Facts]) {
        let tasks = Json::Array(changed.iter().map(crate::hooks::task_to_json).collect());
        if let Err(message) = self.call(h, "on_exit", &[tasks]).await {
            // `on_exit` can only talk: a failure in it is said, not a reason to stop.
            h.warn(message);
        }
    }
}

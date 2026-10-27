//! The hook protocol of `task`, spoken for the Lua script. `task` runs an executable per hook from `<data>/hooks/`:
//!
//! * `on-launch` gets no input and `args:task add …` among its arguments; exit non-zero to stop the command.
//! * `on-add` gets the new task as a line of JSON and prints it back (changed or not).
//! * `on-modify` gets the task as it was and as it will be, two lines, and prints the new one back.
//! * `on-exit` gets the tasks the command changed, a line each, and only prints.
//!
//! Anything else a hook prints is shown to the person running `task`; for `on-add`, `on-modify` and `on-launch` a
//! non-zero exit stops the command and shows what was printed. The script's `say` and `warn` are such lines, and
//! `refuse("why")` (or an error in the script) is the non-zero exit.

use crate::convert::{apply, changes_of, facts_of, table_of};
use crate::net::HostNet;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tc_core::dates::Clock;
use tc_core::hooks::{Kind, Line};
use tc_core::lua_hooks::{Called, Failed, LuaRunner};
use tc_core::taskrc::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    Launch,
    Add,
    Modify,
    Exit,
}

impl Hook {
    pub const ALL: [Hook; 4] = [Hook::Launch, Hook::Add, Hook::Modify, Hook::Exit];

    /// The hook a file called `on-add.meghnaad` (or `on-add`) is for.
    pub fn of_file(name: &str) -> Option<Hook> {
        Hook::ALL.into_iter().find(|h| name.starts_with(h.file()))
    }

    /// The file `task` looks for, before the part after the dot.
    pub fn file(self) -> &'static str {
        match self {
            Hook::Launch => "on-launch",
            Hook::Add => "on-add",
            Hook::Modify => "on-modify",
            Hook::Exit => "on-exit",
        }
    }

    /// The function the script defines.
    pub fn function(self) -> &'static str {
        match self {
            Hook::Launch => "on_launch",
            Hook::Add => "on_add",
            Hook::Modify => "on_modify",
            Hook::Exit => "on_exit",
        }
    }
}

/// What `task` says about itself in a hook's arguments: `api:2 args:task add x command:add rc:/home/me/.taskrc
/// data:/home/me/.task version:3.5.0`.
#[derive(Debug, Default, Clone)]
pub struct Invocation {
    pub rc: Option<PathBuf>,
    pub data: Option<PathBuf>,
    /// The command line, without the `task` in front of it.
    pub command: String,
}

impl Invocation {
    pub fn parse(argv: &[String]) -> Invocation {
        let mut inv = Invocation::default();
        for a in argv {
            if let Some(v) = a.strip_prefix("rc:") {
                inv.rc = Some(PathBuf::from(v));
            } else if let Some(v) = a.strip_prefix("data:") {
                inv.data = Some(PathBuf::from(v));
            } else if let Some(v) = a.strip_prefix("args:") {
                inv.command = v
                    .split_once(char::is_whitespace)
                    .map_or("", |(_, rest)| rest)
                    .trim()
                    .to_owned();
            }
        }
        inv
    }

    /// Where the script is: `MEGHNAAD_HOOKS`, else `hooks.lua` in the data directory `task` named, else `~/.task`.
    pub fn script(&self) -> PathBuf {
        if let Some(p) = std::env::var_os("MEGHNAAD_HOOKS") {
            return PathBuf::from(p);
        }
        data_dir(self.data.as_deref()).join("hooks.lua")
    }

    /// The taskrc, as a `Config` (an include is not followed, nor an unreadable file a reason to stop).
    pub fn config(&self) -> Config {
        let rc = self
            .rc
            .clone()
            .or_else(|| std::env::var_os("TASKRC").map(PathBuf::from));
        let rc = rc.unwrap_or_else(|| home().join(".taskrc"));
        std::fs::read_to_string(rc)
            .map(|text| tc_core::taskrc::parse(&text).config)
            .unwrap_or_default()
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// The data directory: the one given, else `TASKDATA`, else `~/.task`.
pub fn data_dir(given: Option<&Path>) -> PathBuf {
    given
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("TASKDATA").map(PathBuf::from))
        .unwrap_or_else(|| home().join(".task"))
}

/// The clock a script sees: the moment, this computer's zone, and how the taskrc reads and writes dates.
pub fn clock(cfg: &Config, now: i64, tz_offset: i32) -> Clock {
    Clock {
        now,
        tz_offset,
        week_starts_monday: cfg.week_starts_monday(),
        iso: cfg.date_iso(),
        format: cfg.date_format(),
    }
}

/// What the hook prints and whether `task` should go on.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub stdout: String,
    pub ok: bool,
}

fn fail(message: impl Into<String>) -> Failed {
    Failed {
        message: message.into(),
        lines: Vec::new(),
    }
}

fn feedback(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .map(|l| match l.kind {
            Kind::Info => l.text.clone(),
            Kind::Warn => format!("Warning: {}", l.text),
        })
        .collect()
}

fn call(runner: &LuaRunner, hook: Hook, args: &[Value]) -> Result<Called, Failed> {
    pollster::block_on(runner.call(hook.function(), args))
}

fn parse(line: Option<&&str>) -> Result<Value, Failed> {
    let line = line.ok_or_else(|| fail("a hook was given no task"))?;
    serde_json::from_str(line).map_err(|e| fail(format!("a hook was given something that is not a task: {e}")))
}

/// Run one hook for what `task` sent on its standard input.
pub fn run(hook: Hook, inv: &Invocation, stdin: &str, now: i64, tz_offset: i32) -> Outcome {
    let cfg = inv.config();
    let script = inv.script();
    let tasks: Vec<&str> = stdin.lines().filter(|l| !l.trim().is_empty()).collect();
    // What goes back when there is nothing to run: the task as it came.
    let unchanged = |tasks: &[&str]| match hook {
        Hook::Add => tasks.first().map(|t| (*t).to_owned()),
        Hook::Modify => tasks.get(1).map(|t| (*t).to_owned()),
        _ => None,
    };
    let source = match std::fs::read_to_string(&script) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return finish(
                hook,
                vec![format!("meghnaad-hook: there is no script at {}", script.display())],
                unchanged(&tasks),
            );
        }
        Err(e) => {
            return refuse(
                hook,
                &format!("meghnaad-hook: {} cannot be read: {e}", script.display()),
            )
        }
    };
    let runner = match LuaRunner::load_with(&source, clock(&cfg, now, tz_offset), Some(Rc::new(HostNet))) {
        Ok(r) => r,
        Err(p) => {
            let at = p.line.map_or(String::new(), |l| format!(" (line {l})"));
            return refuse(hook, &format!("meghnaad-hook: {}{at}: {}", script.display(), p.message));
        }
    };
    if !runner.defined().contains(&hook.function()) {
        return finish(hook, Vec::new(), unchanged(&tasks));
    }
    match handle(hook, inv, &tasks, &runner, &cfg) {
        Ok((lines, task)) => finish(hook, feedback(&lines), task),
        Err(failed) => {
            let mut out = feedback(&failed.lines);
            out.push(failed.message);
            refuse(hook, &out.join("\n"))
        }
    }
}

/// The hook ran: what it printed, then the task (for the hooks that give one back).
fn finish(hook: Hook, lines: Vec<String>, task: Option<String>) -> Outcome {
    let mut stdout = String::new();
    for l in lines
        .into_iter()
        .chain(task.filter(|_| matches!(hook, Hook::Add | Hook::Modify)))
    {
        stdout.push_str(&l);
        stdout.push('\n');
    }
    Outcome { stdout, ok: true }
}

/// The hook stops the command (`on-exit` runs after it, so it can only say so).
fn refuse(hook: Hook, message: &str) -> Outcome {
    Outcome {
        stdout: format!("{message}\n"),
        ok: hook == Hook::Exit,
    }
}

type Done = (Vec<Line>, Option<String>);

fn handle(hook: Hook, inv: &Invocation, tasks: &[&str], runner: &LuaRunner, cfg: &Config) -> Result<Done, Failed> {
    match hook {
        Hook::Launch => {
            let called = call(runner, hook, &[Value::String(inv.command.clone())])?;
            Ok((called.lines, None))
        }
        Hook::Add => {
            let sent = parse(tasks.first())?;
            let before = facts_of(&sent, cfg).map_err(fail)?;
            let called = call(runner, hook, &[table_of(&before)])?;
            let task = returned(&sent, &before, &called, cfg)?;
            Ok((called.lines, Some(task)))
        }
        Hook::Modify => {
            let old = facts_of(&parse(tasks.first())?, cfg).map_err(fail)?;
            let sent = parse(tasks.get(1))?;
            let new = facts_of(&sent, cfg).map_err(fail)?;
            let called = call(runner, hook, &[table_of(&old), table_of(&new)])?;
            let task = returned(&sent, &new, &called, cfg)?;
            Ok((called.lines, Some(task)))
        }
        Hook::Exit => {
            let changed = tasks
                .iter()
                .map(|t| {
                    parse(Some(t))
                        .and_then(|v| facts_of(&v, cfg).map_err(fail))
                        .map(|f| table_of(&f))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let called = call(runner, hook, &[Value::Array(changed)])?;
            Ok((called.lines, None))
        }
    }
}

/// The task the script gave back, as a line of JSON in the form `task` sent it. Returning nothing leaves it as it is.
fn returned(sent: &Value, before: &tc_core::model::Facts, called: &Called, cfg: &Config) -> Result<String, Failed> {
    let changes = if called.value.is_null() {
        Vec::new()
    } else {
        changes_of(before, &called.value).map_err(|m| Failed {
            message: m,
            lines: called.lines.clone(),
        })?
    };
    Ok(apply(sent.clone(), &changes, cfg).to_string())
}

//! `meghnaad-hook`: the Lua hooks script of Meghnaad as hooks for the real `task` CLI.
//!
//! `task` runs executables from `<data>/hooks/`. `meghnaad-hook install` links this one program there under the names
//! `task` looks for (`on-add.meghnaad`, …), for the hooks the script defines; run as one of them it does what the web
//! app and the phone do with the same script (`tc_core::lua_hooks`), and speaks `task`'s hook protocol (`run.rs`).
//! The network works as in the web app; `secret("NAME")` reads the environment variable `NAME` (`net.rs`).

mod convert;
mod net;
mod run;

use run::{data_dir, Hook, Invocation};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "meghnaad-hook: your Meghnaad hooks script, as hooks for `task`

  meghnaad-hook check [SCRIPT]                 say which hooks a script defines, or what is wrong with it
  meghnaad-hook install [SCRIPT] [--data DIR]  link the hooks the script defines into DIR/hooks
  meghnaad-hook uninstall [--data DIR]         remove those links

The script is SCRIPT, else $MEGHNAAD_HOOKS, else DIR/hooks.lua. DIR is --data, else $TASKDATA, else ~/.task.
`install SCRIPT` makes DIR/hooks.lua a link to SCRIPT, so the script can live anywhere. Run install again after the
script defines another hook. In a script, secret(\"NAME\") is the environment variable NAME.";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let name = argv
        .first()
        .and_then(|a| Path::new(a).file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = std::io::stdout().lock();
    // Whatever cannot be written, `task` is not there to read: there is nobody to tell.
    let mut say = |text: &str| {
        writeln!(out, "{text}").ok();
    };
    if let Some(hook) = Hook::of_file(&name) {
        let mut stdin = String::new();
        if hook != Hook::Launch {
            std::io::stdin().read_to_string(&mut stdin).ok();
        }
        let (now, tz) = moment();
        let done = run::run(hook, &Invocation::parse(&argv[1..]), &stdin, now, tz);
        out.write_all(done.stdout.as_bytes()).ok();
        return if done.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
    let rest: Vec<&str> = argv.iter().skip(1).map(String::as_str).collect();
    let result = match rest.split_first() {
        Some((&"check", args)) => check(args),
        Some((&"install", args)) => install(args),
        Some((&"uninstall", args)) => uninstall(args),
        _ => {
            say(USAGE);
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(lines) => {
            lines.iter().for_each(|l| say(l));
            ExitCode::SUCCESS
        }
        Err(message) => {
            say(&message);
            ExitCode::FAILURE
        }
    }
}

/// Now, in seconds since the epoch, and this computer's offset from UTC.
fn moment() -> (i64, i32) {
    let now = chrono::Local::now();
    (now.timestamp(), now.offset().local_minus_utc())
}

/// `--data DIR` taken out of the arguments, and the rest.
fn split_data<'a>(args: &[&'a str]) -> Result<(Option<PathBuf>, Vec<&'a str>), String> {
    let mut data = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if *a == "--data" {
            data = Some(PathBuf::from(it.next().ok_or("--data needs a directory")?));
        } else {
            rest.push(*a);
        }
    }
    Ok((data, rest))
}

fn read(script: &Path) -> Result<String, String> {
    std::fs::read_to_string(script).map_err(|e| format!("{} cannot be read: {e}", script.display()))
}

/// The hooks a script defines, or what is wrong with it.
fn defined(script: &Path) -> Result<Vec<Hook>, String> {
    let names = tc_core::lua_hooks::check(&read(script)?).map_err(|p| {
        let at = p.line.map_or(String::new(), |l| format!(" (line {l})"));
        format!("{}{at}: {}", script.display(), p.message)
    })?;
    Ok(Hook::ALL
        .into_iter()
        .filter(|h| names.contains(&h.function()))
        .collect())
}

fn check(args: &[&str]) -> Result<Vec<String>, String> {
    let script = args
        .first()
        .map_or_else(|| Invocation::default().script(), PathBuf::from);
    let hooks = defined(&script)?;
    let names: Vec<_> = hooks.iter().map(|h| h.function()).collect();
    Ok(vec![if names.is_empty() {
        format!("{} loads and defines no hooks.", script.display())
    } else {
        format!("{} loads and defines {}.", script.display(), names.join(", "))
    }])
}

/// The links in `<data>/hooks` that are ours.
fn ours(hooks_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(hooks_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "meghnaad")
                && p.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink())
        })
        .collect()
}

fn install(args: &[&str]) -> Result<Vec<String>, String> {
    let (data, rest) = split_data(args)?;
    let data = data_dir(data.as_deref());
    let mut lines = Vec::new();
    let place = data.join("hooks.lua");
    let script = match rest.first() {
        None => std::env::var_os("MEGHNAAD_HOOKS").map_or(place.clone(), PathBuf::from),
        Some(given) => {
            let given = std::fs::canonicalize(given).map_err(|e| format!("{given}: {e}"))?;
            if std::fs::canonicalize(&place).ok().as_ref() != Some(&given) {
                if place.symlink_metadata().is_ok_and(|m| !m.file_type().is_symlink()) {
                    return Err(format!(
                        "{} is already a file; move it aside or install it by name.",
                        place.display()
                    ));
                }
                std::fs::create_dir_all(&data).map_err(|e| format!("{}: {e}", data.display()))?;
                std::fs::remove_file(&place).ok(); // a link to somewhere else, or nothing
                std::os::unix::fs::symlink(&given, &place).map_err(|e| format!("{}: {e}", place.display()))?;
                lines.push(format!("{} -> {}", place.display(), given.display()));
            }
            given
        }
    };
    let hooks = defined(&script)?;
    let me = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| format!("cannot find this program: {e}"))?;
    let dir = data.join("hooks");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let wanted: Vec<PathBuf> = hooks
        .iter()
        .map(|h| dir.join(format!("{}.meghnaad", h.file())))
        .collect();
    for old in ours(&dir).into_iter().filter(|p| !wanted.contains(p)) {
        std::fs::remove_file(&old).map_err(|e| format!("{}: {e}", old.display()))?;
        lines.push(format!("removed {}", old.display()));
    }
    for link in &wanted {
        if link.symlink_metadata().is_ok_and(|m| !m.file_type().is_symlink()) {
            return Err(format!("{} is already a file; not replacing it.", link.display()));
        }
        std::fs::remove_file(link).ok(); // an older link to this program, or nothing
        std::os::unix::fs::symlink(&me, link).map_err(|e| format!("{}: {e}", link.display()))?;
        lines.push(format!("{} -> {}", link.display(), me.display()));
    }
    if wanted.is_empty() {
        lines.push(format!("{} defines no hooks, so nothing is linked.", script.display()));
    }
    Ok(lines)
}

fn uninstall(args: &[&str]) -> Result<Vec<String>, String> {
    let (data, _) = split_data(args)?;
    let dir = data_dir(data.as_deref()).join("hooks");
    let links = ours(&dir);
    for link in &links {
        std::fs::remove_file(link).map_err(|e| format!("{}: {e}", link.display()))?;
    }
    Ok(vec![format!("removed {} link(s) from {}.", links.len(), dir.display())])
}

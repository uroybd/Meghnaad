use std::io::{self, Write};
use std::process::ExitCode;

const HELP: &str = "meghnaad: Meghnaad in the terminal, around the real task (Taskwarrior 3.6.0; 3.5 works too).

Usage: meghnaad                open it on the tasks task reads (TASKRC and TASKDATA are honoured)
       meghnaad <line>         run a command line as the app's command line does: task commands written out,
                               '&&' '||' ';' (quoted) between statements, everything else straight to task
       meghnaad export ...     task export, with the Export dialog's choices (meghnaad export --help)
       meghnaad --sync <line>  the same, with task sync before it (waited for) and, if it changes tasks, after it
                               (in the background)
       meghnaad --dry-run <line>
                               print the statements <line> would run (task commands written out); run nothing
       meghnaad skill          print the skill that teaches an AI agent to use meghnaad and task safely
       meghnaad --help         this
       meghnaad --version      the release it is from

Its task commands are kept in settings.json in $MEGHNAAD_CONFIG_DIR, else ~/.config/meghnaad.
Press ? in the app for every key.
";

/// The exit status for a result: 0, or 1 with the error said, as a `main` returning `io::Result` would.
fn status(r: io::Result<()>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Standard error itself may be what failed; there is nowhere else to say it.
            writeln!(io::stderr(), "meghnaad: {e}").ok();
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "export") {
        let task = meghnaad_tui::task::Task::from_env();
        return match meghnaad_tui::export::run(&task, args.get(1..).unwrap_or_default()) {
            Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            Err(e) => status(Err(e)),
        };
    }
    match args.first().map(String::as_str) {
        None => status(meghnaad_tui::run()),
        Some("--help" | "-h") => status(io::stdout().write_all(HELP.as_bytes())),
        Some("skill") => status(io::stdout().write_all(meghnaad_tui::SKILL.as_bytes())),
        Some("--dry-run" | "-n") => match meghnaad_tui::run::dry_run(args.get(1..).unwrap_or_default()) {
            Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            Err(e) => status(Err(e)),
        },
        Some("--version" | "-V") => status(writeln!(io::stdout(), "meghnaad {}", env!("CARGO_PKG_VERSION"))),
        Some("--sync") if args.len() == 1 => {
            // Standard error itself may be what failed; the status says it either way.
            writeln!(
                io::stderr(),
                "meghnaad: --sync goes before a command line: meghnaad --sync <line>"
            )
            .ok();
            ExitCode::from(2)
        }
        Some(first) => {
            let task = meghnaad_tui::task::Task::from_env();
            let sync = first == "--sync";
            match meghnaad_tui::run::run(&task, args.get(usize::from(sync)..).unwrap_or_default(), sync) {
                Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
                Err(e) => status(Err(e)),
            }
        }
    }
}

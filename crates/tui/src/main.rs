use std::io::{self, Write};
use std::process::ExitCode;

const HELP: &str = "meghnaad: Meghnaad in the terminal, around the real task (Taskwarrior 3.5.0).

Usage: meghnaad                open it on the tasks task reads (TASKRC and TASKDATA are honoured)
       meghnaad export ...     task export, with the Export dialog's choices (meghnaad export --help)
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
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return status(io::stdout().write_all(HELP.as_bytes()));
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        return status(writeln!(io::stdout(), "meghnaad {}", env!("CARGO_PKG_VERSION")));
    }
    status(meghnaad_tui::run())
}

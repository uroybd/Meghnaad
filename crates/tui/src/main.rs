use std::io::{self, Write};

const HELP: &str = "meghnaad: Meghnaad in the terminal, around the real task (Taskwarrior 3.5.0).

Usage: meghnaad                open it on the tasks task reads (TASKRC and TASKDATA are honoured)
       meghnaad --help         this
       meghnaad --version      the release it is from

Its task commands are kept in settings.json in $MEGHNAAD_CONFIG_DIR, else ~/.config/meghnaad.
Press ? in the app for every key.
";

fn main() -> io::Result<()> {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        return io::stdout().write_all(HELP.as_bytes());
    }
    if std::env::args().skip(1).any(|a| a == "--version" || a == "-V") {
        return writeln!(io::stdout(), "meghnaad {}", env!("CARGO_PKG_VERSION"));
    }
    meghnaad_tui::run()
}

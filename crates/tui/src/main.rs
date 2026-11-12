use std::io::{self, Write};

const HELP: &str = "meghnaad-tui: Meghnaad in the terminal, around the real task (Taskwarrior 3.5.0).

Usage: meghnaad-tui            open it on the tasks task reads (TASKRC and TASKDATA are honoured)
       meghnaad-tui --help     this
       meghnaad-tui --version  the release it is from

Its task commands are kept in settings.json in $MEGHNAAD_CONFIG_DIR, else ~/.config/meghnaad.
Press ? in the app for every key.
";

fn main() -> io::Result<()> {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        return io::stdout().write_all(HELP.as_bytes());
    }
    if std::env::args().skip(1).any(|a| a == "--version" || a == "-V") {
        return writeln!(io::stdout(), "meghnaad-tui {}", env!("CARGO_PKG_VERSION"));
    }
    meghnaad_tui::run()
}

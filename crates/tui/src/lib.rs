//! Meghnaad on the desktop: a terminal app around the real `task` 3.5.0. Every command and every read is a `task`
//! command line, and its tasks are the desktop's own (`~/.task`, synced by `task sync`). `tc-core` comes in only for what
//! `task` has no command for, the same helpers the page and the phone call: completion, the add preview, and report
//! cells. Its own `settings.json` (task commands, boards, and its settings under `tui`) is kept on this machine only.

pub mod ansi;
pub mod app;
pub mod appearance;
pub mod boards;
pub mod boardui;
pub mod chain;
pub mod commands;
pub mod completer;
pub mod form;
pub mod lists;
pub mod model;
pub mod remind;
pub mod review;
pub mod settings;
pub mod sync;
pub mod task;
pub mod theme;
pub mod ui;

use app::{Action, App};
use chain::runs;
use ratatui::crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::DefaultTerminal;
use std::io::{self, BufRead, Write};
use std::time::Duration;
use tc_core::filter::split_words;

/// Take the terminal, and ask it to say when it gets the focus back (to look at its colours again).
fn init() -> DefaultTerminal {
    let terminal = ratatui::init();
    // A terminal that cannot say is drawn as it is.
    execute!(io::stdout(), EnableFocusChange).ok();
    terminal
}

/// Give the terminal back as it was.
fn restore() {
    // As in `init`: nothing to undo where it was not done.
    execute!(io::stdout(), DisableFocusChange).ok();
    ratatui::restore();
}

/// Hand the terminal to `task` for each statement that runs (as a shell would run the chain), then take it back:
/// after a key when `pause` (to read what it said), at once otherwise (an editor that was closed).
fn run_on_terminal(app: &App, steps: &[chain::Statement], pause: bool) -> io::Result<()> {
    restore();
    let mut out = io::stdout();
    let mut ok = true;
    for s in steps {
        if !runs(s.join, ok) {
            continue;
        }
        writeln!(out, "\n$ task {}", s.line)?;
        ok = match app.task.interactive(&split_words(&s.line)) {
            Ok(done) => done,
            Err(e) => {
                writeln!(out, "{e}")?;
                false
            }
        };
    }
    if !pause {
        return Ok(());
    }
    write!(out, "\n[Enter to go back]")?;
    out.flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(())
}

/// Run until quit; what the closing sync said when it failed.
fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<Option<String>> {
    loop {
        (app.width, app.height) = ui::page_size(terminal.size()?.into());
        terminal.draw(|f| ui::draw(f, app))?;
        // A second without a key still moves the timer and checks the reminders.
        if !event::poll(Duration::from_secs(1))? {
            app.tick();
            continue;
        }
        let key = match event::read()? {
            Event::Key(key) => key,
            Event::FocusGained => {
                app.focus_gained();
                continue;
            }
            _ => continue,
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match app.key(key) {
            Action::None => {}
            Action::Quit => {
                if app.syncer.running() || app.sync_mode.at_start_and_close() {
                    "Syncing before closing…".clone_into(&mut app.status);
                    terminal.draw(|f| ui::draw(f, app))?;
                }
                return Ok(app.sync_at_close());
            }
            Action::Run(steps) => {
                run_on_terminal(app, &steps, true)?;
                *terminal = init();
                app.status.clear();
                app.reload();
                app.ran(&steps);
            }
            Action::Edit(uuid) => {
                let edit = chain::Statement {
                    line: format!("{uuid} edit"),
                    join: None,
                };
                run_on_terminal(app, &[edit], false)?;
                *terminal = init();
                app.edited();
            }
        }
    }
}

/// Run the app until it is quit.
///
/// # Errors
///
/// `task` is missing, or the terminal could not be used.
pub fn run() -> io::Result<()> {
    let task = task::Task::from_env();
    let version = task.version().map_err(io::Error::other)?;
    let mut app = App::new(task);
    if version != task::VERSION {
        app.status = format!("task {version} found; this app follows Taskwarrior {}.", task::VERSION);
    }
    // Asked before the screen is taken, while the terminal answers only this.
    app.appearance = appearance::detect();
    app.system_mode = appearance::system();
    app.reload();
    if app.sync_mode.at_start_and_close() {
        app.syncer.start(&app.task);
    }
    let mut terminal = init();
    let result = event_loop(&mut terminal, &mut app);
    restore();
    if let Some(why) = result? {
        writeln!(io::stderr(), "meghnaad-tui: {why}")?;
    }
    Ok(())
}

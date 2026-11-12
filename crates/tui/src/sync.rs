//! Syncing: `task sync`, with `task`'s own sync settings, run in the background so the screen never waits on the
//! network and the terminal is never handed over for it. When it runs is the `sync` setting in `settings.json`:
//! by hand only (`S`), also when the app opens and closes, or also after every change made from it.

use crate::task::{Output, Task};
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// When the app syncs by itself, as `settings.json` says (`"sync": "manual"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncMode {
    /// Only when asked (`S`, or `sync` at the command line).
    #[default]
    Manual,
    /// Also when the app opens and when it closes.
    StartAndClose,
    /// Also after every command that changes a task.
    EveryChange,
}

impl SyncMode {
    pub const ALL: [SyncMode; 3] = [SyncMode::Manual, SyncMode::StartAndClose, SyncMode::EveryChange];

    /// The value in `settings.json`.
    pub fn key(self) -> &'static str {
        match self {
            SyncMode::Manual => "manual",
            SyncMode::StartAndClose => "startAndClose",
            SyncMode::EveryChange => "everyChange",
        }
    }

    pub fn parse(s: &str) -> Option<SyncMode> {
        SyncMode::ALL.into_iter().find(|m| m.key() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            SyncMode::Manual => "by hand only (S)",
            SyncMode::StartAndClose => "when the app opens and closes",
            SyncMode::EveryChange => "when the app opens and closes, and after every change",
        }
    }

    pub fn next(self) -> SyncMode {
        match self {
            SyncMode::Manual => SyncMode::StartAndClose,
            SyncMode::StartAndClose => SyncMode::EveryChange,
            SyncMode::EveryChange => SyncMode::Manual,
        }
    }

    pub fn at_start_and_close(self) -> bool {
        self != SyncMode::Manual
    }

    pub fn after_changes(self) -> bool {
        self == SyncMode::EveryChange
    }
}

/// How the last sync went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Last {
    pub at: i64,
    pub ok: bool,
}

/// A sync running in the background, and whether another is wanted after it (a change made while it ran).
#[derive(Debug, Default)]
pub struct Syncer {
    running: Option<Receiver<Result<Output, String>>>,
    again: bool,
    pub last: Option<Last>,
}

impl Syncer {
    pub fn running(&self) -> bool {
        self.running.is_some()
    }

    /// Start `task sync` in the background; while one runs, another follows it.
    pub fn start(&mut self, task: &Task) {
        if self.running.is_some() {
            self.again = true;
            return;
        }
        let (tx, rx) = mpsc::channel();
        let task = task.clone();
        std::thread::spawn(move || {
            // The screen may be gone by the time it ends; nobody is left to tell.
            tx.send(task.capture(&["sync"])).ok();
        });
        self.running = Some(rx);
    }

    /// How the running sync ended, once it has; a follow-up starts then if one was wanted.
    pub fn poll(&mut self, task: &Task, now: i64) -> Option<Result<Output, String>> {
        let rx = self.running.as_ref()?;
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("the sync stopped without a word".into()),
        };
        self.finish(task, now, &result);
        Some(result)
    }

    /// Wait for the running sync, if there is one, and say how it ended.
    pub fn wait(&mut self, now: i64) -> Option<Result<Output, String>> {
        let rx = self.running.take()?;
        let result = rx
            .recv()
            .unwrap_or_else(|_| Err("the sync stopped without a word".into()));
        self.again = false;
        self.last = Some(Last {
            at: now,
            ok: matches!(&result, Ok(o) if o.ok),
        });
        Some(result)
    }

    fn finish(&mut self, task: &Task, now: i64, result: &Result<Output, String>) {
        self.running = None;
        self.last = Some(Last {
            at: now,
            ok: matches!(result, Ok(o) if o.ok),
        });
        if std::mem::take(&mut self.again) {
            self.start(task);
        }
    }
}

/// The line to show for how a sync ended.
pub fn said(result: &Result<Output, String>) -> String {
    match result {
        Ok(o) if o.ok => "Synced.".into(),
        Ok(o) => match o.summary() {
            why if why.is_empty() => "Sync failed: task sync said no.".into(),
            why => format!("Sync failed: {why}"),
        },
        Err(e) => format!("Sync failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_reads_as_it_is_written() {
        for m in SyncMode::ALL {
            assert_eq!(SyncMode::parse(m.key()), Some(m));
        }
        assert_eq!(SyncMode::parse("hourly"), None);
        assert!(!SyncMode::Manual.at_start_and_close());
        assert!(SyncMode::EveryChange.at_start_and_close() && SyncMode::EveryChange.after_changes());
        assert!(!SyncMode::StartAndClose.after_changes());
        assert_eq!(SyncMode::EveryChange.next(), SyncMode::Manual);
    }

    #[cfg(unix)]
    #[test]
    fn a_sync_runs_in_the_background_and_one_asked_for_meanwhile_follows_it() {
        // `true` stands in for `task`: it succeeds whatever it is asked.
        let task = Task::new("true");
        let mut s = Syncer::default();
        s.start(&task);
        s.start(&task);
        assert!(s.running());
        let first = loop {
            if let Some(r) = s.poll(&task, 1) {
                break r;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(said(&first), "Synced.");
        assert!(s.running(), "the second one started");
        assert!(s.wait(2).is_some());
        assert!(!s.running());
        assert_eq!(s.last, Some(Last { at: 2, ok: true }));
        assert_eq!(
            said(&Task::new("false").capture(&["sync"])),
            "Sync failed: task sync said no."
        );
    }
}

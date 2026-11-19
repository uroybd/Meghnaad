//! Syncing: `task sync`, with `task`'s own sync settings, run in the background so the screen never waits on the
//! network and the terminal is never handed over for it. When it runs is the `sync` setting in `settings.json`:
//! by hand only (`S`), also when the app opens and closes, or also after every change made from it.
//!
//! The sync at close is handed to `sh`, so the app quits at once however long the store takes: it waits for a sync the
//! app left running, syncs, and writes what went wrong to `$XDG_STATE_HOME/meghnaad/close-sync.txt`, which the next
//! start reads and reports.

use crate::task::{Output, Task};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

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
    running: Option<Running>,
    again: bool,
    pub last: Option<Last>,
}

/// A sync under way: how it ends, and its process once it has started.
#[derive(Debug)]
struct Running {
    ended: Receiver<Result<Output, String>>,
    pid: Arc<Mutex<Option<u32>>>,
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
        let pid = Arc::<Mutex<Option<u32>>>::default();
        let task = task.clone();
        let started = Arc::clone(&pid);
        std::thread::spawn(move || {
            let result = task.capture_lasting(&["sync"], |id| {
                *started.lock().unwrap_or_else(PoisonError::into_inner) = Some(id);
            });
            // The screen may be gone by the time it ends; nobody is left to tell.
            tx.send(result).ok();
        });
        self.running = Some(Running { ended: rx, pid });
    }

    /// The process of the running sync, waiting a moment for one about to start (it waits its turn with the data).
    pub fn pid(&self) -> Option<u32> {
        let running = self.running.as_ref()?;
        let until = Instant::now() + Duration::from_secs(1);
        loop {
            let pid = *running.pid.lock().unwrap_or_else(PoisonError::into_inner);
            if pid.is_some() || Instant::now() >= until {
                return pid;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// How the running sync ended, once it has; a follow-up starts then if one was wanted.
    pub fn poll(&mut self, task: &Task, now: i64) -> Option<Result<Output, String>> {
        let rx = &self.running.as_ref()?.ended;
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
        let rx = self.running.take()?.ended;
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

/// The sync at close, as `sh -c` runs it with `$1` the sync the app left running (or empty), `$2` the `task` binary and
/// `$3` the failure file: wait for the first, two minutes at most, then sync; the file is removed when it works and
/// holds the time and what `task` said when it does not.
const AT_CLOSE: &str = r#"i=0; while [ -n "$1" ] && [ $i -lt 120 ] && kill -0 "$1" 2>/dev/null; do sleep 1; i=$((i+1)); done
out=$("$2" sync 2>&1) && rm -f "$3" || { mkdir -p "${3%/*}" && { date +%s; printf '%s\n' "$out"; } > "$3"; }"#;

/// Hand the sync at close to `sh`, in a process group of its own so it outlives the app and its terminal; it waits for
/// the sync `after` (one the app left running) first.
///
/// # Errors
///
/// There is no state folder, or `sh` could not be started.
pub fn hand_over(task: &Task, after: Option<u32>) -> Result<(), String> {
    let file = failure_file().ok_or("Could not sync at close: no state folder (set XDG_STATE_HOME or HOME).")?;
    let mut c = Command::new("sh");
    c.args(["-c", AT_CLOSE, "sh", &after.map(|p| p.to_string()).unwrap_or_default()])
        .arg(task.bin())
        .arg(file);
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut c, 0);
    c.spawn().map(drop).map_err(|e| format!("Could not sync at close: {e}"))
}

/// The app's state folder: `$XDG_STATE_HOME/meghnaad`, else `~/.local/state/meghnaad`.
pub fn state_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    state_dir_from(var("XDG_STATE_HOME"), var("HOME"))
}

fn state_dir_from(xdg: Option<std::ffi::OsString>, home: Option<std::ffi::OsString>) -> Option<PathBuf> {
    // The spec has a relative path ignored.
    xdg.map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| PathBuf::from(h).join(".local").join("state")))
        .map(|d| d.join("meghnaad"))
}

fn failure_file() -> Option<PathBuf> {
    state_dir().map(|d| d.join("close-sync.txt"))
}

/// What the last sync at close said, if it failed, as a line to show; read once (the file is removed).
pub fn take_failure() -> Option<String> {
    let file = failure_file()?;
    let text = std::fs::read_to_string(&file).ok()?;
    // Read once: a later failure writes it again.
    std::fs::remove_file(&file).ok();
    failure_line(&text)
}

/// The file's time, then what `task` said.
fn failure_line(text: &str) -> Option<String> {
    let (at, said) = text.split_once('\n')?;
    let when = at
        .trim()
        .parse()
        .ok()
        .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
        .map(|t| t.with_timezone(&chrono::Local).format(" (%Y-%m-%d %H:%M)").to_string())
        .unwrap_or_default();
    let why = crate::task::first_line(said, "");
    let said = if why.is_empty() {
        "task sync said no.".to_owned()
    } else {
        why
    };
    Some(format!("At the last close{when}, sync failed: {said}"))
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

    #[cfg(unix)]
    #[test]
    fn a_running_sync_says_its_process() {
        let task = Task::new("true");
        let mut s = Syncer::default();
        assert_eq!(s.pid(), None);
        s.start(&task);
        assert!(s.pid().is_some());
        assert!(s.wait(1).is_some());
    }

    #[test]
    fn the_state_folder_is_xdg_s() {
        let os = |s: &str| Some(std::ffi::OsString::from(s));
        assert_eq!(
            state_dir_from(os("/x/state"), os("/home/a")),
            Some(PathBuf::from("/x/state/meghnaad"))
        );
        assert_eq!(
            state_dir_from(os("rel"), os("/home/a")),
            Some(PathBuf::from("/home/a/.local/state/meghnaad"))
        );
        assert_eq!(state_dir_from(None, None), None);
    }

    #[test]
    fn a_failed_close_is_said_with_when() {
        let line = failure_line("0\nCould not reach the server.\n").unwrap();
        assert!(line.starts_with("At the last close ("), "{line}");
        assert!(line.ends_with("), sync failed: Could not reach the server."), "{line}");
        assert!(failure_line("0\n\n").unwrap().ends_with("task sync said no."));
        assert_eq!(failure_line(""), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_sync_at_close_waits_and_keeps_a_failure_and_clears_it_on_success() {
        let dir = std::env::temp_dir().join(format!("meghnaad-close-{}", std::process::id()));
        let file = dir.join("state").join("close-sync.txt");
        let run = |bin: &str, after: &str| {
            Command::new("sh")
                .args(["-c", AT_CLOSE, "sh", after, bin])
                .arg(&file)
                .status()
                .unwrap()
        };
        let mut sleeper = Command::new("sleep").arg("1").spawn().unwrap();
        let pid = sleeper.id().to_string();
        // Reaped as it ends, as the system reaps the app's sync once the app has gone (a zombie still answers kill -0).
        let reaper = std::thread::spawn(move || sleeper.wait());
        let started = Instant::now();
        run("false", &pid);
        reaper.join().unwrap().unwrap();
        assert!(started.elapsed() < Duration::from_secs(30), "not the whole two minutes");
        assert!(
            started.elapsed() >= Duration::from_millis(900),
            "waited for the running sync"
        );
        assert!(failure_line(&std::fs::read_to_string(&file).unwrap()).is_some());
        run("true", "");
        assert!(!file.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}

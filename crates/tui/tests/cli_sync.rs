#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! `meghnaad --sync <line>`, the binary itself, over a stand-in for `task` that writes down what it is asked: a sync
//! before the line, waited for, and one after it in the background only when the line changes tasks.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A folder with the stand-in `task`, its log, an empty config folder and a state folder.
fn stand_in(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("meghnaad-cli-sync-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("config")).unwrap();
    let log = dir.join("asked");
    let script = dir.join("task");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in *_commands*) printf 'add\\nmodify\\nnext\\nsync\\n' ;; esac\nexit 0\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (dir, log)
}

fn meghnaad(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_meghnaad"))
        .args(args)
        .env("MEGHNAAD_TASK", dir.join("task"))
        .env("MEGHNAAD_CONFIG_DIR", dir.join("config"))
        .env("XDG_STATE_HOME", dir.join("state"))
        .output()
        .unwrap()
}

/// What the stand-in was asked, a line each, once `want` lines are there (the sync after is in the background) or
/// after five seconds.
fn asked(log: &Path, want: usize) -> Vec<String> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let lines: Vec<String> = std::fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .map(|l| {
                l.split_whitespace()
                    .filter(|w| !w.starts_with("rc."))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        if lines.len() >= want || Instant::now() > until {
            return lines;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_line_that_changes_tasks_is_synced_before_and_after() {
    let (dir, log) = stand_in("change");
    let out = meghnaad(&dir, &["--sync", "add", "Call the plumber"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(asked(&log, 4), ["sync", "_commands", "add Call the plumber", "sync"]);
}

#[test]
fn a_line_that_changes_nothing_is_synced_only_before() {
    let (dir, log) = stand_in("read");
    let out = meghnaad(&dir, &["--sync", "next"]);
    assert!(out.status.success(), "{out:?}");
    // Long enough for a sync after to have been written down, were there one.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(asked(&log, 3), ["sync", "_commands", "next"]);
}

#[test]
fn without_sync_a_line_runs_alone_and_sync_needs_a_line() {
    let (dir, log) = stand_in("plain");
    assert!(meghnaad(&dir, &["add", "x"]).status.success());
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(asked(&log, 1), ["add x"]);
    let alone = meghnaad(&dir, &["--sync"]);
    assert_eq!(alone.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&alone.stderr).contains("--sync goes before a command line"));
}

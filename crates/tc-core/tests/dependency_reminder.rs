//! `dependency.reminder`, as the real `task` 3.5.0 prints it (checked on a scratch taskrc and data folder): starting,
//! finishing or deleting a task that still waits on open tasks names them, and, where finishing or deleting it breaks a
//! chain, the tasks waiting on it, before the repair question.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]

use taskchampion::Replica;
use tc_core::cli::{execute, CliResult, Done, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::hooks::Kind;
use tc_core::taskrc::{parse, Config};
use tc_core::LiveStorage;

type R = Replica<LiveStorage>;

async fn run(r: &mut R, cfg: &Config, line: &str, o: Options) -> Done {
    thread_local! {
        static TICK: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    }
    let tick = TICK.with(|t| t.replace(t.get() + 1));
    let clock = Clock::utc(taskchampion::chrono::Utc::now().timestamp() + tick);
    execute(r, cfg, clock, &split_words(line), o, &mut UndoStack::default()).await
}

/// a, b, c waiting on a and b, d waiting on c: as `task add a; task add b; task add c depends:1,2; task add d depends:3`.
async fn chain(cfg: &Config) -> R {
    let mut r = Replica::new(LiveStorage::new());
    for line in ["add a", "add b", "add c depends:1,2", "add d depends:3"] {
        assert!(run(&mut r, cfg, line, Options::default()).await.wrote, "{line}");
    }
    r
}

fn said(d: &Done) -> Vec<&str> {
    assert!(d.feedback.iter().all(|l| l.kind == Kind::Warn), "{:?}", d.feedback);
    d.feedback.iter().map(|l| l.text.as_str()).collect()
}

const BLOCKED_BY: [&str; 3] = ["Task 3 is blocked by:", "  1 a", "  2 b"];
const AND_BLOCKING: [&str; 5] = ["Task 3 is blocked by:", "  1 a", "  2 b", "and is blocking:", "  4 d"];

#[tokio::test]
async fn starting_a_blocked_task_names_what_it_waits_on() {
    let cfg = Config::default();
    let mut r = chain(&cfg).await;
    let d = run(&mut r, &cfg, "3 start", Options::default()).await;
    assert!(d.wrote);
    assert_eq!(said(&d), BLOCKED_BY);
    // Waiting on nothing open: nothing to say.
    let d = run(&mut r, &cfg, "1 start", Options::default()).await;
    assert!(d.wrote && d.feedback.is_empty(), "{:?}", d.feedback);
}

#[tokio::test]
async fn off_says_nothing_but_still_asks_about_the_chain() {
    let cfg = parse("dependency.reminder=off\n").config;
    let mut r = chain(&cfg).await;
    let d = run(&mut r, &cfg, "3 start", Options::default()).await;
    assert!(d.wrote && d.feedback.is_empty(), "{:?}", d.feedback);
    let d = run(&mut r, &cfg, "3 done", Options::default()).await;
    let CliResult::Confirm { items, .. } = &d.result else {
        panic!("{:?}", d.result)
    };
    assert_eq!(items[0].question, "Would you like the dependency chain fixed?");
    assert!(items[0].note.is_empty());
}

#[tokio::test]
async fn breaking_a_chain_says_it_before_the_question_and_not_again() {
    let cfg = Config::default();
    let mut r = chain(&cfg).await;
    let d = run(&mut r, &cfg, "3 done", Options::default()).await;
    assert!(!d.wrote);
    let CliResult::Confirm { items, .. } = &d.result else {
        panic!("{:?}", d.result)
    };
    assert_eq!(items[0].note, AND_BLOCKING);
    // Answered (no): the change is made, and the reminder, shown with the question, isn't repeated.
    let no = Options {
        extras: Some(vec![]),
        ..Options::default()
    };
    let d = run(&mut r, &cfg, "3 done", no).await;
    assert!(d.wrote && d.feedback.is_empty(), "{:?}", d.feedback);
}

#[tokio::test]
async fn with_no_question_the_whole_reminder_goes_under_the_result() {
    let cfg = parse("dependency.confirmation=off\n").config;
    let mut r = chain(&cfg).await;
    let d = run(&mut r, &cfg, "3 done", Options::default()).await;
    assert!(d.wrote, "{:?}", d.result);
    assert_eq!(said(&d), AND_BLOCKING);
    // Repaired: d now waits on a and b, and finishing it says so (the end of the chain: no question).
    let d = run(&mut r, &cfg, "description:d done", Options::default()).await;
    assert!(d.wrote, "{:?}", d.result);
    let lines = said(&d);
    assert_eq!(lines[1..], ["  1 a", "  2 b"], "{lines:?}");
    assert!(lines[0].starts_with("Task ") && lines[0].ends_with(" is blocked by:"));
}

#[tokio::test]
async fn deleting_reminds_as_finishing_does() {
    let cfg = parse("dependency.confirmation=off\nconfirmation=off\n").config;
    let mut r = chain(&cfg).await;
    let d = run(&mut r, &cfg, "3 delete", Options::default()).await;
    assert!(d.wrote, "{:?}", d.result);
    assert_eq!(said(&d), AND_BLOCKING);
}

//! Iterative tasks (Taskwarrior 3.6.0's `iter`), end to end: command lines against a real replica, checked against
//! what `task` 3.6.0 does (its `test/iterative.test.py`, and runs of it on scratch data).
#![cfg(feature = "iterative")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use taskchampion::Replica;
use tc_core::cli::{execute, load_facts, Ask, CliResult, Options, UndoStack};
use tc_core::dates::{Clock, DAY};
use tc_core::filter::split_words;
use tc_core::model::Facts;
use tc_core::taskrc::{parse, Config};
use tc_core::LiveStorage;
use uuid::Uuid;

type R = Replica<LiveStorage>;

thread_local! {
    static UNDO: std::cell::RefCell<UndoStack> = std::cell::RefCell::new(UndoStack::default());
}

fn now() -> i64 {
    taskchampion::chrono::Utc::now().timestamp()
}

async fn run_opts(r: &mut R, cfg: &Config, line: &str, o: Options) -> CliResult {
    let mut undo = UNDO.with(|u| u.borrow().clone());
    let d = execute(r, cfg, Clock::utc(now()), &split_words(line), o, &mut undo).await;
    UNDO.with(|u| *u.borrow_mut() = undo);
    d.result
}

async fn run(r: &mut R, cfg: &Config, line: &str) -> CliResult {
    run_opts(r, cfg, line, Options::default()).await
}

fn message(res: &CliResult) -> String {
    match res {
        CliResult::Changed { message, .. } => message.clone(),
        CliResult::Error { message } => format!("ERROR: {message}"),
        CliResult::Text { lines } => lines.join("\n"),
        CliResult::Confirm { message, .. } => format!("CONFIRM: {message}"),
        other => format!("{other:?}"),
    }
}

async fn all(r: &mut R) -> Vec<Facts> {
    let mut v = load_facts(r).await.unwrap();
    v.sort_by_key(|f| (f.entry, f.iter_count.clone()));
    v
}

async fn one(r: &mut R, uuid: Uuid) -> Facts {
    all(r).await.into_iter().find(|f| f.uuid == uuid).unwrap()
}

/// Midnight UTC `days` from today.
fn midnight(days: i64) -> i64 {
    (now() / DAY + days) * DAY
}

fn cfg() -> Config {
    parse("confirmation=off\n").config
}

#[tokio::test]
async fn an_iter_and_a_date_make_an_iterative_task_numbered_and_listed() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    let res = run(
        &mut r,
        &cfg,
        "add Water the plants iter:daily iter_type:fixed due:tomorrow",
    )
    .await;
    assert_eq!(message(&res), "Created task 1.");
    let [f] = all(&mut r).await.try_into().unwrap();
    assert_eq!(f.status, "iterative");
    assert_eq!(f.iter.as_deref(), Some("daily"));
    assert_eq!(f.iter_type.as_deref(), Some("fixed"));
    assert_eq!(f.iter_count.as_deref(), Some("1"));
    for filter in [
        "+PENDING",
        "+ITERATIVE",
        "status:iterative",
        "iter_count:1",
        "iter_type:fixed",
        "+READY",
    ] {
        let res = run(&mut r, &cfg, &format!("{filter} count")).await;
        assert_eq!(message(&res), "1", "{filter}");
    }
    // `status:pending` is the status itself, which an iterative task does not have.
    assert_eq!(message(&run(&mut r, &cfg, "status:pending count").await), "0");
    let CliResult::Report(o) = run(&mut r, &cfg, "next").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
}

#[tokio::test]
async fn what_task_refuses_it_refuses_with_the_same_words() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    for (line, error) in [
        (
            "add a iter:daily iter_type:fixed",
            "Usage Error: Iterative tasks require a 'due', 'scheduled' or 'wait' date.",
        ),
        (
            "add a iter:daily due:tomorrow",
            "Usage Error: Iterative tasks require an 'iter_type' of fixed, fixed+ or chained.",
        ),
        (
            "add a iter:blah iter_type:fixed due:tomorrow",
            "Usage Error: Could not parse iteration value \"blah\"",
        ),
        (
            "add a iter:daily iter_type:FIXED due:tomorrow",
            "The 'iter_type' attribute does not allow a value of 'FIXED'.",
        ),
        (
            "add a iter:daily iter_type:fixed due:tomorrow iter_count:5",
            "The 'iter_count' attribute does not allow a value of '5'.",
        ),
        (
            "add a iter:daily iter_type:fixed due:tomorrow recur:daily",
            "A task cannot be both recurring and iterative.",
        ),
        (
            "add a status:iterative due:tomorrow",
            "Usage Error: Iterative tasks require an 'iter' value.",
        ),
    ] {
        let res = run(&mut r, &cfg, line).await;
        assert!(message(&res).contains(error), "{line}: {}", message(&res));
    }
    assert!(all(&mut r).await.is_empty());

    run(&mut r, &cfg, "add b iter:daily iter_type:fixed due:tomorrow").await;
    for (line, error) in [
        (
            "1 modify iter_type:",
            "You cannot remove the iteration type from an iterative task.",
        ),
        (
            "1 modify due:",
            "You cannot remove the last of the due, scheduled and wait dates from an iterative task.",
        ),
        (
            "1 modify status:pending",
            "You cannot change the status of an iterative task. Clear iter to end the series instead.",
        ),
        ("1 modify end:now", "You cannot set an end date on a pending task."),
    ] {
        let res = run(&mut r, &cfg, line).await;
        assert!(message(&res).contains(error), "{line}: {}", message(&res));
    }
}

#[tokio::test]
async fn done_completes_it_and_makes_the_next_under_a_uuid_derived_from_its_own() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    let due = midnight(1);
    run(
        &mut r,
        &cfg,
        &format!("add Water iter:daily iter_type:fixed due:{due} project:Home +garden"),
    )
    .await;
    let first = all(&mut r).await[0].uuid;
    run(&mut r, &cfg, "1 annotate wet the roots").await;
    let res = run(&mut r, &cfg, "1 done").await;
    assert_eq!(message(&res), "Completed 1 task.");

    let done = one(&mut r, first).await;
    assert_eq!(done.status, "completed");
    assert!(done.end.is_some());
    assert_eq!((done.iter.as_deref(), done.iter_type.as_deref()), (None, None));
    assert_eq!(done.iter_count.as_deref(), Some("1"));

    // TaskChampion's own name for it, so two replicas completing the same task make the same one.
    let second = Uuid::new_v5(&tc_core::iter::NAMESPACE, first.as_bytes());
    let next = one(&mut r, second).await;
    assert_eq!(next.status, "iterative");
    assert_eq!(next.iter_count.as_deref(), Some("2"));
    assert_eq!(next.due, Some(due + DAY));
    assert_eq!(
        (next.iter.as_deref(), next.iter_type.as_deref()),
        (Some("daily"), Some("fixed"))
    );
    assert_eq!(
        (next.project.as_deref(), next.description.as_str()),
        (Some("Home"), "Water")
    );
    assert!(next.tags.contains("garden"));
    assert!(
        next.annotations.is_empty(),
        "annotations stay with the one they were made on"
    );
    assert!(next.end.is_none());
}

#[tokio::test]
async fn a_count_in_the_rule_ends_the_series() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(
        &mut r,
        &cfg,
        "add Twice iter:FREQ=DAILY;COUNT=2 iter_type:fixed due:tomorrow",
    )
    .await;
    run(&mut r, &cfg, "+ITERATIVE done").await;
    assert_eq!(message(&run(&mut r, &cfg, "+ITERATIVE count").await), "1");
    run(&mut r, &cfg, "+ITERATIVE done").await;
    assert_eq!(message(&run(&mut r, &cfg, "+ITERATIVE count").await), "0");
    assert_eq!(all(&mut r).await.len(), 2);
}

#[tokio::test]
async fn one_undo_takes_back_the_completion_and_the_next_task() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(&mut r, &cfg, "add Water iter:daily iter_type:fixed due:tomorrow").await;
    run(&mut r, &cfg, "1 done").await;
    assert_eq!(all(&mut r).await.len(), 2);
    run(&mut r, &cfg, "undo").await;
    let [f] = all(&mut r).await.try_into().unwrap();
    assert_eq!(f.status, "iterative");
    assert_eq!(f.iter.as_deref(), Some("daily"));
}

#[tokio::test]
async fn clearing_iter_asks_and_ends_the_series() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = parse("confirmation=on\n").config;
    run(&mut r, &cfg, "add Pay rent iter:monthly iter_type:fixed due:tomorrow").await;
    let res = run(&mut r, &cfg, "1 modify iter:").await;
    let CliResult::Confirm { ask, items, .. } = &res else {
        panic!("{res:?}")
    };
    assert_eq!(*ask, Ask::Extras);
    assert_eq!(items[0].question, "End the iterative series for task 1 'Pay rent'?");

    // No: left as it is.
    let answer = |yes: Vec<String>| Options {
        extras: Some(yes),
        ..Options::default()
    };
    let res = run_opts(&mut r, &cfg, "1 modify iter:", answer(vec![])).await;
    assert_eq!(message(&res), "Task not modified.");
    assert_eq!(all(&mut r).await[0].status, "iterative");

    // Yes: a plain task, its place in the series gone.
    run_opts(&mut r, &cfg, "1 modify iter:", answer(vec![items[0].key.clone()])).await;
    let [f] = all(&mut r).await.try_into().unwrap();
    assert_eq!(f.status, "pending");
    assert_eq!((f.iter, f.iter_type, f.iter_count), (None, None, None));
}

#[tokio::test]
async fn clearing_iter_ends_a_series_that_came_without_a_count() {
    // `task` always numbers one, but a task synced from elsewhere may lack `iter_count`: clearing its `iter` still makes
    // it a plain task, not an iterative one with no rule to complete it by.
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(&mut r, &cfg, "add Pay rent iter:monthly iter_type:fixed due:tomorrow").await;
    let uuid = all(&mut r).await[0].uuid;
    let mut ops = taskchampion::Operations::new();
    let mut t = r.get_task(uuid).await.unwrap().unwrap();
    t.set_value("iter_count", None, &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();

    run(&mut r, &cfg, "1 modify iter:").await;
    let f = one(&mut r, uuid).await;
    assert_eq!(f.status, "pending");
    assert_eq!((f.iter, f.iter_type, f.iter_count), (None, None, None));
}

#[tokio::test]
async fn a_copy_starts_a_series_of_its_own() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(&mut r, &cfg, "add Water iter:daily iter_type:fixed due:tomorrow").await;
    run(&mut r, &cfg, "1 done").await;
    run(&mut r, &cfg, "+ITERATIVE duplicate").await;
    let open: Vec<Facts> = all(&mut r)
        .await
        .into_iter()
        .filter(|f| f.status == "iterative")
        .collect();
    assert_eq!(open.len(), 2);
    assert!(open.iter().any(|f| f.iter_count.as_deref() == Some("1")));
}

#[tokio::test]
async fn an_iter_on_a_plain_task_makes_it_iterative() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(&mut r, &cfg, "add Stretch due:tomorrow").await;
    run(&mut r, &cfg, "1 modify iter:weekly iter_type:chained").await;
    let [f] = all(&mut r).await.try_into().unwrap();
    assert_eq!((f.status.as_str(), f.iter_count.as_deref()), ("iterative", Some("1")));
}

#[tokio::test]
async fn export_and_import_carry_it_over() {
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(&mut r, &cfg, "add Water iter:daily iter_type:fixed due:tomorrow").await;
    let CliResult::File { text: json, .. } = run(&mut r, &cfg, "export").await else {
        panic!()
    };
    assert!(json.contains("\"iter_count\":1"), "{json}");
    assert!(json.contains("\"status\":\"iterative\""), "{json}");

    let mut other = Replica::new(LiveStorage::new());
    let mut undo = UndoStack::default();
    let out = tc_core::import::import(&mut other, &cfg, Clock::utc(now()), &json, true, &mut undo, None).await;
    assert_eq!(out.unwrap().added, 1);
    let [f] = all(&mut other).await.try_into().unwrap();
    assert_eq!((f.status.as_str(), f.iter_count.as_deref()), ("iterative", Some("1")));
}

/// The next task is the one TaskChampion makes when it completes the task itself (natively, in the machine's zone):
/// the same uuid, the same properties, the same dates. Mid-January, so that no change of clocks falls between them.
#[tokio::test]
async fn the_next_task_is_the_one_taskchampion_would_make() {
    use taskchampion::chrono::{Local, TimeZone};
    use taskchampion::{Operations, Status};
    let due = Local.with_ymd_and_hms(2027, 1, 12, 9, 0, 0).single().unwrap();
    let uuid = Uuid::from_u128(0x2d16_a16d_0000_4000_8000_0000_0000_0001);
    let make = |r: &mut R| {
        let mut r = std::mem::replace(r, Replica::new(LiveStorage::new()));
        async move {
            let mut ops = Operations::new();
            let mut t = r.create_task(uuid, &mut ops).await.unwrap();
            for (k, v) in [
                ("description", "Water"),
                ("project", "Home"),
                ("tag_garden", ""),
                ("iter", "daily"),
                ("iter_type", "fixed"),
                ("iter_count", "1"),
                ("status", "iterative"),
                ("entry", "1767225600"),
            ] {
                t.set_value(k, Some(v.into()), &mut ops).unwrap();
            }
            t.set_due(Some(due.to_utc()), &mut ops).unwrap();
            r.commit_operations(ops).await.unwrap();
            r
        }
    };
    let mut theirs = make(&mut Replica::new(LiveStorage::new())).await;
    let mut ops = Operations::new();
    let mut t = theirs.get_task(uuid).await.unwrap().unwrap();
    t.set_status(Status::Completed, &mut ops).unwrap();
    theirs.commit_operations(ops).await.unwrap();

    let mut ours = make(&mut Replica::new(LiveStorage::new())).await;
    let clock = Clock {
        tz_offset: due.offset().local_minus_utc(),
        ..Clock::utc(now())
    };
    let mut undo = UndoStack::default();
    let line = split_words(&format!("{uuid} done"));
    execute(&mut ours, &cfg(), clock, &line, Options::default(), &mut undo).await;

    let next = Uuid::new_v5(&tc_core::iter::NAMESPACE, uuid.as_bytes());
    let props = |r: &mut R, u: Uuid| {
        let mut r = std::mem::replace(r, Replica::new(LiveStorage::new()));
        async move {
            let t = r.get_task_data(u).await.unwrap().expect("made");
            let mut m: Vec<(String, String)> = t
                .iter()
                .filter(|(k, _)| !matches!(k.as_str(), "modified" | "entry" | "end"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            m.sort();
            (m, r)
        }
    };
    let (want, mut theirs) = props(&mut theirs, next).await;
    let (got, mut ours) = props(&mut ours, next).await;
    assert_eq!(got, want);
    let (want, _) = props(&mut theirs, uuid).await;
    let (got, _) = props(&mut ours, uuid).await;
    assert_eq!(got, want);
}

/// A host that names its zone gets the next date across a change of the clocks, as `task` does in the machine's zone:
/// midnight in New York stays midnight after the clocks go forward. With the offset alone, the hour moves.
#[tokio::test]
async fn a_request_naming_its_zone_keeps_the_wall_clock_time() {
    use tc_core::request;
    use tc_core::warm::Warm;
    // 2027-03-10 00:00 EST (UTC-5); the clocks go forward on 2027-03-14.
    let due = 1_804_654_800;
    let next_due = |zone: Option<&str>| {
        let zone = zone.map(|z| format!(",\"zone\":\"{z}\"")).unwrap_or_default();
        async move {
            let storage = LiveStorage::new();
            let warm = Warm::new(storage.generation());
            let mut r = Replica::new(storage);
            let cfg = cfg();
            let mut undo = UndoStack::default();
            for line in [
                format!("add Rent iter:monthly iter_type:fixed due:{due}"),
                "1 done".to_owned(),
            ] {
                let req = format!(r#"{{"line":"{line}","tz":-18000,"now":{}{zone}}}"#, now());
                let answer = request::run(&mut r, &cfg, &warm, &mut undo, |_| None, &req).await;
                assert!(!answer.contains("\"error\""), "{answer}");
            }
            all(&mut r)
                .await
                .into_iter()
                .find(|f| f.status == "iterative")
                .unwrap()
                .due
                .unwrap()
        }
    };
    // 2027-04-10 00:00 EDT (UTC-4).
    assert_eq!(next_due(Some("America/New_York")).await, 1_807_329_600);
    assert_eq!(next_due(None).await, 1_807_329_600 + 3600);
}

#[tokio::test]
async fn an_iterative_task_made_recurring_in_one_modify_is_a_template() {
    // What a form switching Repeat from iterative to recurring runs, as `task` 3.6.0 takes it: the series ended and the
    // task the template of a recurring one, with its first instance.
    let mut r = Replica::new(LiveStorage::new());
    let cfg = cfg();
    run(&mut r, &cfg, "add Water plants iter:daily iter_type:fixed due:tomorrow").await;
    let res = run(&mut r, &cfg, "1 modify iter: iter_type: recur:weekly").await;
    assert!(!message(&res).starts_with("ERROR"), "{}", message(&res));
    run(&mut r, &cfg, "count").await; // the first command after makes the first instance
    let tasks = all(&mut r).await;
    let mut statuses: Vec<_> = tasks.iter().map(|f| f.status.as_str()).collect();
    statuses.sort_unstable();
    assert_eq!(statuses, ["pending", "recurring"], "{tasks:?}");
    assert!(tasks
        .iter()
        .all(|f| f.iter.is_none() && f.iter_type.is_none() && f.iter_count.is_none()));
}

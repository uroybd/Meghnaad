//! Real command lines against a real taskchampion `Replica` (in-memory storage).

use taskchampion::Replica;
use tc_core::cli::{execute, load_facts, Ask, CliResult, ConfirmItem, Options, UndoStack};
use tc_core::dates::{Clock, DAY};
use tc_core::filter::split_words;
use tc_core::taskrc::{parse, Config};
use tc_core::LiveStorage;

type R = Replica<LiveStorage>;

fn replica() -> R {
    Replica::new(LiveStorage::new())
}

thread_local! {
    // One undo stack per test thread (tests run on their own threads), standing in for the
    // Worker's per-isolate session.
    static UNDO: std::cell::RefCell<UndoStack> = std::cell::RefCell::new(UndoStack::default());
}

fn clock() -> Clock {
    // Real time (taskchampion stamps `modified`/`end` with the wall clock), advancing one
    // second per command so tasks added back to back get distinct, ordered `entry` times.
    use std::sync::atomic::{AtomicI64, Ordering};
    static TICK: AtomicI64 = AtomicI64::new(0);
    Clock::utc(taskchampion::chrono::Utc::now().timestamp() + TICK.fetch_add(1, Ordering::SeqCst))
}

async fn run(r: &mut R, cfg: &Config, line: &str) -> (CliResult, bool) {
    run_opts(r, cfg, line, Options::default()).await
}

async fn run_opts(r: &mut R, cfg: &Config, line: &str, o: Options) -> (CliResult, bool) {
    let mut undo = UNDO.with(|u| u.borrow().clone());
    let d = execute(r, cfg, clock(), &split_words(line), o, &mut undo).await;
    UNDO.with(|u| *u.borrow_mut() = undo);
    (d.result, d.wrote)
}

/// Run a command and say yes to every question it asks, as a user pressing "yes" and "select all"
/// would. (The questions can come in up to three rounds.)
async fn run_yes(r: &mut R, cfg: &Config, line: &str) -> (CliResult, bool) {
    let mut o = Options::default();
    for _ in 0..5 {
        let (res, wrote) = run_opts(r, cfg, line, o.clone()).await;
        let CliResult::Confirm { ask, items, .. } = &res else {
            return (res, wrote);
        };
        let keys: Vec<String> = items.iter().map(|i| i.key.clone()).collect();
        match ask {
            Ask::Plain => o.confirmed = true,
            Ask::Permission => o.approved = Some(keys),
            Ask::Extras => o.extras = Some(keys),
        }
    }
    panic!("still being asked after five rounds: {line}");
}

/// The questions in a `Confirm`, or a panic.
fn asked(res: &CliResult) -> (Ask, &[ConfirmItem]) {
    let CliResult::Confirm { ask, items, .. } = res else {
        panic!("not a question: {res:?}")
    };
    (*ask, items)
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

async fn descs(r: &mut R) -> Vec<String> {
    let mut v: Vec<_> = load_facts(r)
        .await
        .unwrap()
        .into_iter()
        .map(|f| f.description)
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn add_then_report_shows_it_with_fields() {
    let mut r = replica();
    let cfg = Config::default();
    let (res, wrote) = run(
        &mut r,
        &cfg,
        "add Buy milk project:Home priority:H +errand due:tomorrow",
    )
    .await;
    assert!(wrote);
    assert_eq!(message(&res), "Created task 1.");

    let (res, wrote) = run(&mut r, &cfg, "list").await;
    assert!(!wrote);
    let CliResult::Report(o) = res else { panic!("{res:?}") };
    assert_eq!(o.rows.len(), 1);
    let row = &o.rows[0];
    assert_eq!(row.facts.description, "Buy milk");
    assert_eq!(row.facts.project.as_deref(), Some("Home"));
    assert_eq!(row.facts.priority.as_deref(), Some("H"));
    assert!(row.facts.tags.contains("errand"));
    assert!(row.facts.due.is_some());
    assert_eq!(row.id, Some(1));
    // 6.0 (H) + 1.0 (project) + 0.8 (1 tag) + due ramp, so comfortably above 8.
    assert!(row.urgency > 8.0, "{}", row.urgency);
    assert_eq!(o.columns[0].name, "id");
}

#[tokio::test]
async fn done_modify_start_stop_delete_by_id() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add first").await;
    run(&mut r, &cfg, "add second").await;

    let (res, _) = run(&mut r, &cfg, "1 modify project:Work +x priority:M").await;
    assert_eq!(message(&res), "Modified 1 task.");
    let f = load_facts(&mut r).await.unwrap();
    let first = f.iter().find(|x| x.description == "first").unwrap();
    assert_eq!(first.project.as_deref(), Some("Work"));
    assert!(first.tags.contains("x"));

    run(&mut r, &cfg, "1 start").await;
    assert!(load_facts(&mut r)
        .await
        .unwrap()
        .iter()
        .find(|x| x.description == "first")
        .unwrap()
        .start
        .is_some());
    run(&mut r, &cfg, "1 stop").await;
    assert!(load_facts(&mut r)
        .await
        .unwrap()
        .iter()
        .find(|x| x.description == "first")
        .unwrap()
        .start
        .is_none());

    let (res, _) = run(&mut r, &cfg, "1 done").await;
    assert_eq!(message(&res), "Completed 1 task.");
    let f = load_facts(&mut r).await.unwrap();
    let first = f.iter().find(|x| x.description == "first").unwrap();
    assert_eq!(first.status, "completed");
    assert!(first.end.is_some(), "completing must set `end` like the CLI does");

    // `second` is now id 1 (ids renumber over pending tasks only).
    let (res, wrote) = run(&mut r, &cfg, "1 delete").await;
    assert!(!wrote);
    assert_eq!(message(&res), "CONFIRM: Delete task 1 'second'?"); // `confirmation` is on by default
    let (res, _) = run_yes(&mut r, &cfg, "1 delete").await;
    assert_eq!(message(&res), "Deleted 1 task.");
    assert_eq!(
        load_facts(&mut r)
            .await
            .unwrap()
            .iter()
            .find(|x| x.description == "second")
            .unwrap()
            .status,
        "deleted"
    );
}

#[tokio::test]
async fn modify_clears_and_edits_description() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add buy milk and milk project:Home due:tomorrow").await;
    run(&mut r, &cfg, "1 modify /milk/oat/g project: due:").await;
    let f = &load_facts(&mut r).await.unwrap()[0];
    assert_eq!(f.description, "buy oat and oat");
    assert_eq!(f.project, None);
    assert_eq!(f.due, None);
    run(&mut r, &cfg, "1 append now").await;
    run(&mut r, &cfg, "1 prepend please").await;
    assert_eq!(descs(&mut r).await, ["please buy oat and oat now"]);
}

#[tokio::test]
async fn annotate_and_denotate() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add task").await;
    run(&mut r, &cfg, "1 annotate first note").await;
    run(&mut r, &cfg, "1 annotate second note").await;
    assert_eq!(load_facts(&mut r).await.unwrap()[0].annotations.len(), 2);
    let (res, _) = run(&mut r, &cfg, "1 denotate first").await;
    assert!(message(&res).starts_with("Updated"));
    let f = &load_facts(&mut r).await.unwrap()[0];
    assert_eq!(f.annotations.len(), 1);
    assert_eq!(f.annotations[0].text, "second note");
    let (res, wrote) = run(&mut r, &cfg, "1 denotate nothing-like-this").await;
    assert!(message(&res).contains("no annotation matches") && !wrote);
}

#[tokio::test]
async fn multi_task_writes_need_confirmation() {
    let mut r = replica();
    let cfg = Config::default();
    for t in ["a", "b", "c"] {
        run(&mut r, &cfg, &format!("add {t} project:P")).await;
    }
    let (res, wrote) = run(&mut r, &cfg, "project:P done").await;
    assert!(
        !wrote && message(&res).contains("complete 3 tasks"),
        "{}",
        message(&res)
    );
    assert!(load_facts(&mut r).await.unwrap().iter().all(|f| f.status == "pending"));

    let (res, wrote) = run_yes(&mut r, &cfg, "project:P done").await;
    assert!(wrote);
    assert_eq!(message(&res), "Completed 3 tasks.");
}

#[tokio::test]
async fn a_write_with_no_filter_asks_first_and_then_changes_everything() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add a").await;
    run(&mut r, &cfg, "add b").await;
    run(&mut r, &cfg, "1 done").await;
    for line in ["done", "delete", "modify +x"] {
        let (res, wrote) = run(&mut r, &cfg, line).await;
        assert!(!wrote, "{line}");
        assert!(
            message(&res).starts_with(
                "CONFIRM: This command has no filter, and will modify all (including completed and deleted) tasks."
            ),
            "{line}: {}",
            message(&res)
        );
    }
    // Yes: every task is changed, finished ones included.
    let (res, wrote) = run_yes(&mut r, &cfg, "modify +x").await;
    assert!(wrote, "{}", message(&res));
    assert!(load_facts(&mut r).await.unwrap().iter().all(|f| f.tags.contains("x")));
}

#[tokio::test]
async fn allow_empty_filter_off_refuses_and_confirmation_off_prevents_it() {
    let mut r = replica();
    run(&mut r, &Config::default(), "add a").await;
    let no = parse("allow.empty.filter=0\n").config;
    for line in ["done", "delete", "modify +x"] {
        let (res, wrote) = run_yes(&mut r, &no, line).await; // even a "yes" can't get past it
        assert!(!wrote);
        assert_eq!(
            message(&res),
            "ERROR: You did not specify a filter, and with the 'allow.empty.filter' value, no action is taken.",
            "{line}"
        );
    }
    // Allowed, but with `confirmation` off there is nobody to ask: Taskwarrior stops, too.
    let quiet = parse("confirmation=off\n").config;
    let (res, wrote) = run(&mut r, &quiet, "modify +x").await;
    assert!(!wrote);
    assert_eq!(message(&res), "ERROR: Command prevented from running.");
    // With a filter, none of this applies.
    let (_, wrote) = run(&mut r, &no, "+nope modify +x").await;
    assert!(!wrote);
    let (_, wrote) = run(&mut r, &no, "1 modify +x").await;
    assert!(wrote);
}

#[tokio::test]
async fn an_active_context_counts_as_a_filter() {
    let mut r = replica();
    let cfg = parse("context.work.read=+w\ncontext=work\nallow.empty.filter=0\n").config;
    run(&mut r, &Config::default(), "add a +w").await;
    let (res, wrote) = run(&mut r, &cfg, "modify project:P").await;
    assert!(wrote, "{}", message(&res));
}

#[tokio::test]
async fn deleting_asks_unless_confirmation_is_off() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add one").await;
    run(&mut r, &cfg, "add two").await;
    // One task or two (below `bulk`): deleting asks, other changes don't.
    for line in ["1 delete", "1,2 delete"] {
        let (res, wrote) = run(&mut r, &cfg, line).await;
        assert!(
            !wrote && message(&res).starts_with("CONFIRM:"),
            "{line}: {}",
            message(&res)
        );
    }
    let (_, wrote) = run(&mut r, &cfg, "1,2 modify +t").await;
    assert!(wrote, "two tasks are fewer than `bulk`");
    let (_, wrote) = run(&mut r, &cfg, "1 annotate hello").await;
    assert!(wrote);

    let off = parse("confirmation=off\n").config;
    let (res, wrote) = run(&mut r, &off, "1 delete").await;
    assert!(wrote, "{}", message(&res));
    assert_eq!(message(&res), "Deleted 1 task.");
}

#[tokio::test]
async fn bulk_sets_how_many_tasks_a_change_may_touch_unasked() {
    let mut r = replica();
    let cfg = Config::default();
    for t in ["a", "b", "c", "d"] {
        run(&mut r, &cfg, &format!("add {t} project:P")).await;
    }
    // The default is 3: a change to three tasks or more asks, whatever the command.
    let (res, wrote) = run(&mut r, &cfg, "1-3 modify +t").await;
    assert!(
        !wrote && message(&res).contains("This will modify 3 tasks"),
        "{}",
        message(&res)
    );
    let (_, wrote) = run(&mut r, &cfg, "1-2 modify +t").await;
    assert!(wrote);

    let five = parse("bulk=5\n").config;
    let (_, wrote) = run(&mut r, &five, "project:P modify +u").await;
    assert!(wrote, "four tasks are fewer than 5");
    let two = parse("bulk=2\n").config;
    let (res, wrote) = run(&mut r, &two, "1-2 done").await;
    assert!(!wrote && message(&res).starts_with("CONFIRM:"), "{}", message(&res));

    // 0 means the count never asks; with `confirmation` off nothing does.
    let never = parse("bulk=0\n").config;
    let (_, wrote) = run(&mut r, &never, "project:P modify +v").await;
    assert!(wrote);
    let (res, wrote) = run(&mut r, &parse("bulk=0\nconfirmation=off\n").config, "project:P delete").await;
    assert!(wrote, "{}", message(&res));
    // But `bulk` still asks when `confirmation` is off.
    let mut r2 = replica();
    for t in ["a", "b", "c"] {
        run(&mut r2, &cfg, &format!("add {t}")).await;
    }
    let (res, wrote) = run(&mut r2, &parse("confirmation=off\n").config, "1-3 delete").await;
    assert!(!wrote && message(&res).starts_with("CONFIRM:"), "{}", message(&res));
}

#[tokio::test]
async fn bulk_asks_about_each_task_and_goes_ahead_with_the_ones_approved() {
    let mut r = replica();
    let cfg = Config::default();
    for t in ["a", "b", "c", "d"] {
        run(&mut r, &cfg, &format!("add {t}")).await;
    }
    let (res, wrote) = run(&mut r, &cfg, "1-4 modify +t").await;
    assert!(!wrote);
    let (ask, items) = asked(&res);
    assert_eq!(ask, Ask::Permission);
    let names: Vec<(&str, &str)> = items
        .iter()
        .map(|i| (i.description.as_str(), i.question.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("a", "Modify task 1 'a'?"),
            ("b", "Modify task 2 'b'?"),
            ("c", "Modify task 3 'c'?"),
            ("d", "Modify task 4 'd'?")
        ]
    );

    // "yes" for a and c, "no" for b and d.
    let yes = vec![items[0].key.clone(), items[2].key.clone()];
    let o = Options {
        approved: Some(yes),
        ..Options::default()
    };
    let (res, wrote) = run_opts(&mut r, &cfg, "1-4 modify +t", o).await;
    assert!(wrote);
    assert_eq!(message(&res), "Modified 2 tasks. Skipped 2 tasks.");
    let tagged: Vec<String> = {
        let mut v: Vec<_> = load_facts(&mut r)
            .await
            .unwrap()
            .into_iter()
            .filter(|f| f.tags.contains("t"))
            .map(|f| f.description)
            .collect();
        v.sort();
        v
    };
    assert_eq!(tagged, ["a", "c"]);
}

#[tokio::test]
async fn approving_none_changes_nothing_and_says_so_like_taskwarrior() {
    let mut r = replica();
    let cfg = Config::default();
    for t in ["a", "b", "c"] {
        run(&mut r, &cfg, &format!("add {t}")).await;
    }
    let o = Options {
        approved: Some(vec![]),
        ..Options::default()
    };
    let (res, wrote) = run_opts(&mut r, &cfg, "1-3 delete", o).await;
    assert!(!wrote);
    assert_eq!(message(&res), "Task not deleted.\nTask not deleted.\nTask not deleted.");
    assert!(load_facts(&mut r).await.unwrap().iter().all(|f| f.status == "pending"));
}

#[tokio::test]
async fn only_the_tasks_a_command_would_change_are_asked_about() {
    let mut r = replica();
    let cfg = Config::default();
    for t in ["a", "b", "c", "d"] {
        run(&mut r, &cfg, &format!("add {t}")).await;
    }
    run(&mut r, &cfg, "1 done").await; // `a` is finished
                                       // Four tasks are selected (past `bulk`), but `done` has nothing to do for the finished one.
    let (res, _) = run(
        &mut r,
        &cfg,
        "description:a or description:b or description:c or description:d done",
    )
    .await;
    let (_, items) = asked(&res);
    let asked_about: Vec<&str> = items.iter().map(|i| i.description.as_str()).collect();
    assert_eq!(asked_about, ["b", "c", "d"]);
    // A modification that changes nothing is not asked about either.
    run(&mut r, &cfg, "2,3,4 modify +same").await;
    let (res, wrote) = run(&mut r, &cfg, "+same modify +same").await;
    assert!(wrote && !matches!(res, CliResult::Confirm { .. }), "{res:?}");
}

#[tokio::test]
async fn deleting_one_task_is_a_single_question_with_taskwarriors_wording() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add Pay rent").await;
    let (res, _) = run(&mut r, &cfg, "1 delete").await;
    let (ask, items) = asked(&res);
    assert_eq!(
        (ask, items.len(), message(&res).as_str()),
        (Ask::Permission, 1, "CONFIRM: Delete task 1 'Pay rent'?")
    );
}

#[tokio::test]
async fn undo_asks_unless_confirmation_is_off() {
    let mut r = replica();
    let cfg = Config::default();
    UNDO.with(|u| *u.borrow_mut() = UndoStack::default());
    // Nothing to undo: no question needed.
    assert!(message(&run(&mut r, &cfg, "undo").await.0).contains("Nothing to undo"));

    run(&mut r, &cfg, "add keep me").await;
    run(&mut r, &cfg, "1 done").await;
    let (res, wrote) = run(&mut r, &cfg, "undo").await;
    assert!(!wrote);
    assert!(
        message(&res).starts_with("CONFIRM: The undo command is not reversible."),
        "{}",
        message(&res)
    );
    assert_eq!(
        load_facts(&mut r).await.unwrap()[0].status,
        "completed",
        "nothing was undone yet"
    );

    // Off: straight away.
    let (res, wrote) = run(&mut r, &parse("confirmation=0\n").config, "undo").await;
    assert!(wrote, "{}", message(&res));
    assert_eq!(load_facts(&mut r).await.unwrap()[0].status, "pending");
}

mod chain_repair {
    use super::*;

    /// 3 <- 2 <- 1: task 1 waits on 2, which waits on 3.
    async fn chain(r: &mut R, cfg: &Config) {
        run(r, cfg, "add three").await;
        run(r, cfg, "add two depends:1").await;
        run(r, cfg, "add one depends:2").await;
    }

    fn deps_of(f: &[tc_core::model::Facts], d: &str) -> Vec<String> {
        let by = |u: &uuid::Uuid| f.iter().find(|x| &x.uuid == u).unwrap().description.clone();
        let mut v: Vec<String> = f
            .iter()
            .find(|x| x.description == d)
            .unwrap()
            .depends
            .iter()
            .map(by)
            .collect();
        v.sort();
        v
    }

    #[tokio::test]
    async fn finishing_the_middle_of_a_chain_offers_to_repair_it() {
        let mut r = replica();
        let cfg = Config::default();
        chain(&mut r, &cfg).await;
        let two = load_facts(&mut r)
            .await
            .unwrap()
            .iter()
            .find(|f| f.description == "two")
            .unwrap()
            .uuid
            .to_string();

        let (res, wrote) = run(&mut r, &cfg, &format!("{two} done")).await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert_eq!(ask, Ask::Extras);
        assert_eq!(items.len(), 1);
        assert_eq!(
            (items[0].question.as_str(), items[0].description.as_str()),
            ("Would you like the dependency chain fixed?", "two")
        );

        // Yes: `one` stops waiting on `two` and waits on `three` instead.
        let o = Options {
            extras: Some(vec![items[0].key.clone()]),
            ..Options::default()
        };
        let (res, wrote) = run_opts(&mut r, &cfg, &format!("{two} done"), o).await;
        assert!(wrote, "{}", message_of(&res));
        assert!(
            message_of(&res).contains("Repaired the dependencies of 1 task"),
            "{}",
            message_of(&res)
        );
        let f = load_facts(&mut r).await.unwrap();
        assert_eq!(deps_of(&f, "one"), ["three"]);
        assert_eq!(f.iter().find(|x| x.description == "two").unwrap().status, "completed");
    }

    fn message_of(res: &CliResult) -> String {
        super::message(res)
    }

    #[tokio::test]
    async fn saying_no_completes_the_task_and_leaves_the_chain() {
        let mut r = replica();
        let cfg = Config::default();
        chain(&mut r, &cfg).await;
        let o = Options {
            extras: Some(vec![]),
            ..Options::default()
        };
        let (_, wrote) = run_opts(&mut r, &cfg, "2 done", o).await;
        assert!(wrote);
        let f = load_facts(&mut r).await.unwrap();
        assert_eq!(deps_of(&f, "one"), ["two"], "untouched");
        assert_eq!(f.iter().find(|x| x.description == "two").unwrap().status, "completed");
    }

    #[tokio::test]
    async fn dependency_confirmation_off_repairs_without_asking() {
        let mut r = replica();
        let cfg = parse("dependency.confirmation=off\n").config;
        chain(&mut r, &cfg).await;
        let (res, wrote) = run(&mut r, &cfg, "2 delete").await; // deleting asks, but the repair doesn't
        assert!(
            !wrote && message_of(&res).starts_with("CONFIRM: Delete task"),
            "{}",
            message_of(&res)
        );
        let (res, wrote) = run_yes(&mut r, &cfg, "2 delete").await;
        assert!(wrote, "{}", message_of(&res));
        assert_eq!(deps_of(&load_facts(&mut r).await.unwrap(), "one"), ["three"]);
    }

    #[tokio::test]
    async fn only_the_middle_of_a_chain_is_a_broken_chain() {
        let mut r = replica();
        let cfg = Config::default();
        chain(&mut r, &cfg).await;
        // `three` waits on nothing; `one` has nothing waiting on it: neither leaves a gap.
        let (res, wrote) = run(&mut r, &cfg, "description:one done").await;
        assert!(wrote, "{}", message_of(&res));
        let (res, wrote) = run(&mut r, &cfg, "description:three done").await;
        assert!(wrote, "{}", message_of(&res));
    }

    #[tokio::test]
    async fn the_chain_is_checked_task_by_task_in_the_order_they_are_finished() {
        let cfg = Config::default();
        // `two` (lower id) is finished before `one` (which waits on it): that breaks the chain.
        let mut r = replica();
        chain(&mut r, &cfg).await;
        let (res, wrote) = run(&mut r, &cfg, "description:one or description:two done").await;
        assert!(!wrote);
        assert_eq!(asked(&res).0, Ask::Extras, "{}", message_of(&res));

        // `one` first (it has the lower id here), so by the time `two` goes nothing waits on it.
        let mut r = replica();
        run(&mut r, &cfg, "add one").await;
        run(&mut r, &cfg, "add two").await;
        run(&mut r, &cfg, "add three").await;
        run(&mut r, &cfg, "1 modify depends:2").await;
        run(&mut r, &cfg, "2 modify depends:3").await;
        let (res, wrote) = run(&mut r, &cfg, "1,2 done").await;
        assert!(wrote, "{}", message_of(&res));
    }
}

#[tokio::test]
async fn undo_reverses_the_last_change() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add keep me").await;
    run(&mut r, &cfg, "1 done").await;
    assert_eq!(load_facts(&mut r).await.unwrap()[0].status, "completed");
    let (res, wrote) = run_yes(&mut r, &cfg, "undo").await;
    assert!(wrote, "{}", message(&res));
    assert_eq!(load_facts(&mut r).await.unwrap()[0].status, "pending");
}

#[tokio::test]
async fn dependencies_block_and_unblock() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add blocker").await;
    run(&mut r, &cfg, "add blocked depends:1").await;
    let f = load_facts(&mut r).await.unwrap();
    let blocked = f.iter().find(|x| x.description == "blocked").unwrap();
    assert!(blocked.blocked);
    assert!(f.iter().find(|x| x.description == "blocker").unwrap().blocking);
    // `unblocked` report hides it; completing the blocker frees it.
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "unblocked").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
    run(&mut r, &cfg, "1 done").await;
    assert!(
        !load_facts(&mut r)
            .await
            .unwrap()
            .iter()
            .find(|x| x.description == "blocked")
            .unwrap()
            .blocked
    );
}

/// Urgency of the task called `name` in the `next` report run with the given settings.
async fn urgency_of(r: &mut R, cfg: &Config, line: &str, name: &str) -> f64 {
    let (CliResult::Report(o), _) = run(r, cfg, line).await else {
        panic!("not a report")
    };
    o.rows
        .iter()
        .find(|x| x.facts.description == name)
        .unwrap_or_else(|| panic!("no {name}"))
        .urgency
}

#[tokio::test]
async fn urgency_inherit_is_off_by_default_and_follows_the_setting() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add blocker").await;
    run(&mut r, &cfg, "add blocked depends:1 priority:H due:-8d").await;

    // Taskwarrior's default: no inheritance. The blocker only has its own +8 for blocking.
    let own = urgency_of(&mut r, &cfg, "next", "blocker").await;
    assert!((own - 8.0).abs() < 1e-9, "{own}");
    let blocked = urgency_of(&mut r, &cfg, "next", "blocked").await;
    assert!(own < blocked);

    // Switched on in the taskrc: the blocker takes the blocked task's urgency, plus 0.01.
    let on = parse("urgency.inherit=1\n").config;
    let inherited = urgency_of(&mut r, &on, "next", "blocker").await;
    assert!((inherited - (blocked + 0.01)).abs() < 1e-9, "{inherited} vs {blocked}");
    assert!((urgency_of(&mut r, &on, "next", "blocked").await - blocked).abs() < 1e-9);

    // ...or for one command, as `task rc.urgency.inherit:1 next` does.
    let once = urgency_of(&mut r, &cfg, "rc.urgency.inherit:1 next", "blocker").await;
    assert!((once - inherited).abs() < 1e-9);
    // The `urgency` filter sees the inherited score too.
    let (CliResult::Report(o), _) = run(&mut r, &on, "next urgency.over:12.5").await else {
        panic!()
    };
    assert_eq!(o.rows.iter().filter(|x| x.facts.description == "blocker").count(), 1);
}

#[tokio::test]
async fn project_urgency_reaches_sub_projects_but_not_lookalikes() {
    let mut r = replica();
    let cfg = parse("urgency.user.project.Work.coefficient=5\n").config;
    run(&mut r, &cfg, "add a project:Work").await;
    run(&mut r, &cfg, "add b project:Work.Reports").await;
    run(&mut r, &cfg, "add c project:Workshop").await;
    // project term 1.0, plus 5 where the coefficient applies
    assert!((urgency_of(&mut r, &cfg, "next", "a").await - 6.0).abs() < 1e-9);
    assert!((urgency_of(&mut r, &cfg, "next", "b").await - 6.0).abs() < 1e-9);
    assert!((urgency_of(&mut r, &cfg, "next", "c").await - 1.0).abs() < 1e-9);
}

#[tokio::test]
async fn count_skips_recurring_templates_like_taskwarrior() {
    let mut r = replica();
    let cfg = parse("recurrence=on\nrecurrence.limit=2\n").config;
    run(&mut r, &cfg, "add plain").await;
    run(&mut r, &cfg, "add Water plants recur:weekly due:tomorrow").await;
    // The first command that runs after the add also generates the two instances.
    let (res, _) = run(&mut r, &cfg, "count").await;
    let all = load_facts(&mut r).await.unwrap();
    let templates = all.iter().filter(|f| f.status == "recurring").count();
    assert_eq!(templates, 1);
    assert_eq!(all.len(), 4, "plain + template + two instances");
    // Everything but the template.
    assert_eq!(message(&res), "3");
}

/// (rows shown, tasks matched) for a report command.
async fn shown(r: &mut R, cfg: &Config, line: &str) -> (usize, usize) {
    let (CliResult::Report(o), _) = run(r, cfg, line).await else {
        panic!("{line}: not a report")
    };
    (o.rows.len(), o.matched)
}

#[tokio::test]
async fn the_limit_setting_cuts_reports_short_unless_the_command_says_otherwise() {
    let mut r = replica();
    let plain = Config::default();
    for i in 0..6 {
        run(&mut r, &plain, &format!("add task {i}")).await;
    }
    // Unset: every task, as in Taskwarrior.
    assert_eq!(shown(&mut r, &plain, "list").await, (6, 6));

    let cfg = parse("limit=2\n").config;
    assert_eq!(
        shown(&mut r, &cfg, "list").await,
        (2, 6),
        "the report stops at 2 and still says how many matched"
    );
    assert_eq!(shown(&mut r, &cfg, "all").await, (2, 6));
    // The same through `rc.limit:` on the command line, with nothing in the taskrc.
    assert_eq!(shown(&mut r, &plain, "rc.limit:3 list").await, (3, 6));
    // `limit:` in the command beats the setting, both ways.
    assert_eq!(shown(&mut r, &cfg, "list limit:4").await, (4, 6));
    assert_eq!(shown(&mut r, &cfg, "list limit:none").await, (6, 6));
    assert_eq!(shown(&mut r, &cfg, "list limit:0").await, (6, 6));
    assert_eq!(
        shown(&mut r, &cfg, "rc.limit:5 list").await,
        (5, 6),
        "rc beats the taskrc"
    );
    // `page` has no meaning in a browser, so it shows everything.
    assert_eq!(shown(&mut r, &parse("limit=page\n").config, "list").await, (6, 6));
    // The built-in `next` carries its own `limit:page`, which wins, as in Taskwarrior.
    assert_eq!(shown(&mut r, &cfg, "next").await, (6, 6));
    // A report definition with its own limit wins over the setting, too.
    let own = parse("limit=2\nreport.top.columns=id,description\nreport.top.filter=status:pending limit:5\n").config;
    assert_eq!(shown(&mut r, &own, "top").await, (5, 6));
}

#[tokio::test]
async fn the_limit_setting_never_hides_tasks_from_export_count_or_info() {
    let mut r = replica();
    let cfg = parse("limit=2\n").config;
    for i in 0..5 {
        run(&mut r, &cfg, &format!("add task {i}")).await;
    }
    let (res, _) = run(&mut r, &cfg, "count").await;
    assert_eq!(message(&res), "5");
    let (CliResult::Json { value }, _) = run(&mut r, &cfg, "_rows").await else {
        panic!("not json")
    };
    assert_eq!(value.as_array().unwrap().len(), 5);
    let (CliResult::Json { value }, _) = run(&mut r, &cfg, "status:pending _rows").await else {
        panic!("not json")
    };
    assert_eq!(
        value.as_array().unwrap().len(),
        5,
        "what the Projects view and reminders read"
    );
}

async fn only_task(r: &mut R, description: &str) -> tc_core::model::Facts {
    load_facts(r)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.description == description)
        .unwrap_or_else(|| panic!("no {description}"))
}

#[tokio::test]
async fn new_tasks_get_the_default_project_due_and_scheduled() {
    let mut r = replica();
    let cfg = parse("default.project=Inbox\ndefault.due=3d\ndefault.scheduled=2030-01-01\n").config;
    let before = taskchampion::chrono::Utc::now().timestamp();
    run(&mut r, &cfg, "add plain").await;
    // The test clock is real time plus a counter shared by every test running at once, so allow an
    // hour of drift: still nowhere near a wrong reading of "3d".
    let after = taskchampion::chrono::Utc::now().timestamp() + 3600;

    let t = only_task(&mut r, "plain").await;
    assert_eq!(t.project.as_deref(), Some("Inbox"));
    // A duration is "from now", like typing `due:3d`.
    let due = t.due.expect("a default due date");
    assert!((before + 3 * 86_400..=after + 3 * 86_400).contains(&due), "{due}");
    assert!(
        t.scheduled.is_some_and(|s| s > before + 86_400 * 365),
        "{:?}",
        t.scheduled
    );

    // What the user gives wins, field by field.
    run(&mut r, &cfg, "add explicit project:Work due:2031-05-05").await;
    let t = only_task(&mut r, "explicit").await;
    assert_eq!(t.project.as_deref(), Some("Work"));
    assert!(
        t.due.is_some_and(|d| d > before + 86_400 * 365 * 4),
        "the typed due date was kept"
    );
    assert!(t.scheduled.is_some(), "but the scheduled default still applies");

    // Taskwarrior fills them in when a task is added, not when one is changed.
    run(&mut r, &Config::default(), "add later").await;
    let (res, _) = run(&mut r, &cfg, "later modify +x").await;
    let t = only_task(&mut r, "later").await;
    assert!(
        t.project.is_none() && t.due.is_none() && t.scheduled.is_none(),
        "{}",
        message(&res)
    );
}

#[tokio::test]
async fn a_default_due_date_does_not_stand_in_for_a_recurring_tasks_own() {
    let mut r = replica();
    let cfg = parse("default.due=3d\n").config;
    let (res, wrote) = run(&mut r, &cfg, "add Water plants recur:weekly").await;
    assert!(
        !wrote && message(&res).contains("must also have a 'due' date"),
        "{}",
        message(&res)
    );
}

#[tokio::test]
async fn an_unusable_default_is_ignored_and_adding_still_works() {
    let p = parse("default.due=whenever\ndefault.project=\n");
    assert!(
        p.warnings.iter().any(|w| w.starts_with("default.due:")),
        "{:?}",
        p.warnings
    );
    assert!(!p.config.settings.contains_key("default.due") && !p.config.settings.contains_key("default.project"));
    // Set on a command line instead of a taskrc, the same rules apply.
    let mut r = replica();
    run(
        &mut r,
        &Config::default(),
        "rc.default.project:Inbox rc.default.due:nonsense add via rc",
    )
    .await;
    let t = only_task(&mut r, "via rc").await;
    assert_eq!(t.project.as_deref(), Some("Inbox"));
    assert!(t.due.is_none());
}

async fn names_in(r: &mut R, cfg: &Config, line: &str) -> Vec<String> {
    let (CliResult::Report(o), _) = run(r, cfg, line).await else {
        panic!("{line}: not a report")
    };
    o.rows.into_iter().map(|x| x.facts.description).collect()
}

#[tokio::test]
async fn blocked_and_blocking_reports_split_a_dependency_chain() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add the blocker").await;
    run(&mut r, &cfg, "add the blocked depends:1").await;
    run(&mut r, &cfg, "add unrelated").await;
    // Which id the blocker has depends on entry-time ties; find it by name.
    let blocker_id = {
        let (CliResult::Report(o), _) = run(&mut r, &cfg, "all").await else {
            panic!()
        };
        o.rows
            .iter()
            .find(|x| x.facts.description == "the blocker")
            .unwrap()
            .id
            .unwrap()
    };
    let _ = blocker_id;

    assert_eq!(names_in(&mut r, &cfg, "blocked").await, ["the blocked"]);
    assert_eq!(names_in(&mut r, &cfg, "blocking").await, ["the blocker"]);
    let unblocked = names_in(&mut r, &cfg, "unblocked").await;
    assert!(unblocked.contains(&"the blocker".to_string()) && !unblocked.contains(&"the blocked".to_string()));
}

#[tokio::test]
async fn long_and_ls_are_taskwarriors_reports() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add first").await;
    run(&mut r, &cfg, "add second +a project:P").await;
    run(&mut r, &cfg, "add waiting one wait:3d").await;

    for name in ["long", "ls"] {
        // Pending and not waiting, like `list`.
        let rows = names_in(&mut r, &cfg, name).await;
        assert_eq!(rows.len(), 2, "{name}: {rows:?}");
        assert!(!rows.contains(&"waiting one".to_string()), "{name}");
    }
    let (CliResult::Report(long), _) = run(&mut r, &cfg, "long").await else {
        panic!()
    };
    let labels: Vec<&str> = long.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "ID",
            "A",
            "Created",
            "Mod",
            "Deps",
            "P",
            "Project",
            "Tags",
            "Recur",
            "Wait",
            "Sched",
            "Due",
            "Until",
            "Description"
        ]
    );
    assert_eq!(long.sort.as_deref(), Some("modified-"));
    let (CliResult::Report(ls), _) = run(&mut r, &cfg, "ls").await else {
        panic!()
    };
    let labels: Vec<&str> = ls.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "ID",
            "A",
            "D",
            "Project",
            "Tags",
            "R",
            "Wait",
            "S",
            "Due",
            "Until",
            "Description"
        ]
    );
    assert_eq!(ls.sort.as_deref(), Some("start-,description+"));

    // A taskrc still overrides one attribute of them, like any built-in.
    let cfg = parse("report.ls.filter=+a\n").config;
    assert_eq!(names_in(&mut r, &cfg, "ls").await, ["second"]);
}

#[tokio::test]
async fn a_contexts_own_settings_apply_only_while_it_is_active() {
    let mut r = replica();
    let plain = Config::default();
    for i in 0..6 {
        run(
            &mut r,
            &plain,
            &format!("add task {i} +{}", if i < 2 { "a" } else { "b" }),
        )
        .await;
    }
    let rc = "context.work.rc.limit=2\ncontext.home.rc.limit=4\n\
              context.home.rc.report.list.filter=+b\n";

    // No context active: the settings of a context do nothing.
    assert_eq!(shown(&mut r, &parse(rc).config, "list").await, (6, 6));

    let cfg = parse(&format!("context=work\n{rc}")).config;
    assert_eq!(shown(&mut r, &cfg, "list").await, (2, 6));
    // Switching context for one command switches its settings too.
    assert_eq!(
        shown(&mut r, &cfg, "rc.context:home list").await,
        (4, 4),
        "limit 4, and list is filtered to +b"
    );
    // They win over a command-line `rc.` override, which is how Taskwarrior looks settings up.
    assert_eq!(shown(&mut r, &cfg, "rc.limit:5 list").await, (2, 6));
    // `limit:` on the command line is a filter word, not a setting, so it still wins.
    assert_eq!(shown(&mut r, &cfg, "list limit:5").await, (5, 6));
    // The other context's settings are not in force.
    assert_eq!(shown(&mut r, &cfg, "all").await, (2, 6));
}

#[tokio::test]
async fn a_contexts_default_command_is_what_a_bare_command_runs() {
    let mut r = replica();
    run(&mut r, &Config::default(), "add something").await;
    let report_of = |res: CliResult| match res {
        CliResult::Report(o) => o.report,
        other => panic!("not a report: {other:?}"),
    };
    let cfg = parse("context=work\ncontext.work.rc.default.command=minimal\n").config;
    assert_eq!(report_of(run(&mut r, &cfg, "").await.0), "minimal");
    // Without the context, or with another one, it is the usual default.
    assert_eq!(report_of(run(&mut r, &Config::default(), "").await.0), "next");
    assert_eq!(report_of(run(&mut r, &cfg, "rc.context:other").await.0), "next");
}

#[tokio::test]
async fn text_in_filters_and_substitutions_is_a_regular_expression() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add Buy milk and eggs").await;
    run(&mut r, &cfg, "add Call Sam").await;
    run(&mut r, &cfg, "add Water the plants").await;
    let (res, _) = run(&mut r, &cfg, "/Call/ annotate renew the passport").await;
    assert!(message(&res).contains("Annotated"), "{}", message(&res));
    let count = |res: CliResult| message(&res);

    assert_eq!(count(run(&mut r, &cfg, "count m.lk").await.0), "1", "a wildcard");
    assert_eq!(count(run(&mut r, &cfg, "count /^(Buy|Water)/").await.0), "2");
    assert_eq!(
        count(run(&mut r, &cfg, "count passport").await.0),
        "1",
        "found in an annotation"
    );
    assert_eq!(count(run(&mut r, &cfg, "count desc.has:pass.ort").await.0), "1");
    assert_eq!(count(run(&mut r, &cfg, "count desc.hasnt:pass.ort").await.0), "2");
    // "the" is a whole word in one description and in another task's annotation.
    assert_eq!(count(run(&mut r, &cfg, "count desc.word:the").await.0), "2");
    assert_eq!(count(run(&mut r, &cfg, "count desc.word:th").await.0), "0");
    // Command-line `rc.regex:off` goes back to plain text for that command.
    assert_eq!(count(run(&mut r, &cfg, "rc.regex:off count m.lk").await.0), "0");
    assert_eq!(count(run(&mut r, &cfg, "rc.regex:off count milk").await.0), "1");

    // Mistakes are errors that say what is wrong, not silent empty results.
    let (res, wrote) = run(&mut r, &cfg, "count /(/").await;
    assert!(
        !wrote && message(&res).contains("not a valid regular expression"),
        "{}",
        message(&res)
    );
    let (res, _) = run(&mut r, &cfg, "count /a(?=b)/").await;
    assert!(message(&res).contains("lookahead"), "{}", message(&res));

    // Substitution: a pattern to find, literal text to put.
    let (res, _) = run(&mut r, &cfg, "/Buy/ modify '/m.lk and/oat milk,/'").await;
    assert!(message(&res).contains("Modified"), "{}", message(&res));
    let descs = descs(&mut r).await;
    assert!(descs.contains(&"Buy oat milk, eggs".to_owned()), "{descs:?}");
}

#[tokio::test]
async fn summary_shows_progress_per_project() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add dishes project:Home.Kitchen").await;
    run(&mut r, &cfg, "add bins project:Home").await;
    run(&mut r, &cfg, "add report project:Work").await;
    run(&mut r, &cfg, "add loose end").await;
    let tpl = |res: CliResult| match res {
        CliResult::Summary(s) => s,
        other => panic!("not a summary: {other:?}"),
    };
    let s = tpl(run(&mut r, &cfg, "summary").await.0);
    let labels: Vec<(&str, usize, usize)> = s
        .rows
        .iter()
        .map(|x| (x.label.as_str(), x.depth, x.remaining))
        .collect();
    assert_eq!(
        labels,
        [("(none)", 0, 1), ("Home", 0, 2), ("Kitchen", 1, 1), ("Work", 0, 1)]
    );

    // Finish one: it shows as progress, and finished projects only appear with the setting.
    let (res, _) = run(&mut r, &cfg, "project:Work done").await;
    assert!(message(&res).contains("Completed"), "{}", message(&res));
    let s = tpl(run(&mut r, &cfg, "summary").await.0);
    assert!(!s.rows.iter().any(|x| x.project == "Work"), "nothing left to do there");
    let all = parse("summary.all.projects=1\n").config;
    let w = tpl(run(&mut r, &all, "summary").await.0)
        .rows
        .into_iter()
        .find(|x| x.project == "Work")
        .unwrap();
    assert_eq!(
        (w.complete.as_str(), w.bar, w.completed, w.remaining),
        ("100%", 30, 1, 0)
    );

    // A filter narrows it, an abbreviation works, and an empty result says so.
    let h = tpl(run(&mut r, &cfg, "project:Home summary").await.0);
    assert_eq!(
        h.rows.iter().map(|x| x.project.as_str()).collect::<Vec<_>>(),
        ["Home", "Home.Kitchen"]
    );
    assert!(matches!(run(&mut r, &cfg, "summ").await.0, CliResult::Summary(_)));
    let (res, _) = run(&mut r, &cfg, "project:Nowhere summary").await;
    assert_eq!(message(&res), "No projects.");
}

#[tokio::test]
async fn calendar_lays_out_months_and_takes_the_arguments_taskwarrior_does() {
    let mut r = replica();
    let cfg = Config::default();
    let cal = |res: CliResult| match res {
        CliResult::Calendar(c) => *c,
        other => panic!("not a calendar: {other:?}"),
    };
    let months = |c: &tc_core::calendar::CalendarOut| c.months.iter().map(|m| (m.year, m.month)).collect::<Vec<_>>();

    let c = cal(run(&mut r, &cfg, "calendar").await.0);
    assert_eq!(c.months.len(), 3);
    assert_eq!(cal(run(&mut r, &cfg, "calendar y").await.0).months.len(), 12);
    assert_eq!(months(&cal(run(&mut r, &cfg, "calendar 3 2031").await.0))[0], (2031, 3));
    assert_eq!(
        months(&cal(run(&mut r, &cfg, "cale march 2031").await.0))[0],
        (2031, 3),
        "abbreviated command (`cal` is ambiguous with `calc`, as in Taskwarrior), named month"
    );
    let (res, wrote) = run(&mut r, &cfg, "calendar 13 2031").await;
    assert!(
        !wrote && message(&res).contains("not a valid month"),
        "{}",
        message(&res)
    );
    let (res, _) = run(&mut r, &cfg, "calendar whenever").await;
    assert!(
        message(&res).contains("Could not recognize argument 'whenever'"),
        "{}",
        message(&res)
    );
    // `rc.` overrides are settings, not arguments.
    assert_eq!(
        cal(run(&mut r, &cfg, "rc.calendar.monthsperline:2 calendar").await.0)
            .months
            .len(),
        2
    );
}

#[tokio::test]
async fn weeks_start_on_sunday_unless_told_otherwise_like_taskwarrior() {
    let mut r = replica();
    let cal = |res: CliResult| match res {
        CliResult::Calendar(c) => *c,
        other => panic!("not a calendar: {other:?}"),
    };
    // Nothing set: Sunday, which is what a real `task calendar` shows without a `weekstart`.
    let c = cal(run(&mut r, &Config::default(), "calendar").await.0);
    assert_eq!(c.weekdays, ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]);
    // In the taskrc, on a command line, and refused when it isn't one of the two.
    let monday = parse("weekstart=Monday\n").config;
    assert_eq!(cal(run(&mut r, &monday, "calendar").await.0).weekdays[0], "Mo");
    assert_eq!(
        cal(run(&mut r, &Config::default(), "rc.weekstart:monday calendar").await.0).weekdays[0],
        "Mo"
    );
    assert_eq!(
        cal(run(&mut r, &monday, "rc.weekstart:sunday calendar").await.0).weekdays[0],
        "Su"
    );
    let p = parse("weekstart=friday\n");
    assert!(!p.config.settings.contains_key("weekstart"));
    assert!(
        p.warnings.iter().any(|w| w.starts_with("weekstart:")),
        "{:?}",
        p.warnings
    );
}

#[tokio::test]
async fn burndown_charts_take_a_filter_and_the_cumulative_setting() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add old work project:Work entry:-5d").await;
    run(&mut r, &cfg, "add home thing project:Home entry:-3d").await;
    let (res, _) = run(&mut r, &cfg, "project:Work done").await;
    assert!(message(&res).contains("Completed"), "{}", message(&res));
    let chart = |res: CliResult| match res {
        CliResult::Burndown(b) => *b,
        other => panic!("not a burndown: {other:?}"),
    };
    let last = |b: &tc_core::burndown::BurndownOut| b.bars.last().map(|x| (x.pending, x.started, x.done)).unwrap();

    let all = chart(run(&mut r, &cfg, "burndown.daily").await.0);
    assert_eq!(all.title, "Daily Burndown");
    assert_eq!(all.bars.len(), 30);
    assert_eq!(last(&all), (1, 0, 1), "one open, one finished");
    // A filter narrows the chart, and so does the rest of the command-line grammar.
    assert_eq!(
        last(&chart(run(&mut r, &cfg, "project:Home burndown.daily").await.0)),
        (1, 0, 0)
    );
    assert_eq!(
        last(&chart(run(&mut r, &cfg, "burndown.daily project:Work").await.0)),
        (0, 0, 1)
    );
    // The other periods have their own commands.
    assert_eq!(chart(run(&mut r, &cfg, "burndown.weekly").await.0).bars.len(), 26);
    assert_eq!(chart(run(&mut r, &cfg, "burndown.monthly").await.0).bars.len(), 24);
    assert_eq!(chart(run(&mut r, &cfg, "burndown.annual").await.0).bars.len(), 10);
    // `burndown.cumulative` is on by default; off, done shows only on the day it happened.
    let flat = parse("burndown.cumulative=off\n").config;
    assert_eq!(
        last(&chart(run(&mut r, &flat, "burndown.daily").await.0)),
        (1, 0, 1),
        "it was finished today"
    );
    let from_yesterday = chart(run(&mut r, &flat, "burndown.daily").await.0);
    assert_eq!(from_yesterday.bars[28].done, 0);
}

#[tokio::test]
async fn a_calendar_filter_narrows_which_tasks_colour_the_days() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add work thing project:Work due:eom").await;
    run(&mut r, &cfg, "add home thing project:Home due:eom").await;
    let cal = |res: CliResult| match res {
        CliResult::Calendar(c) => *c,
        other => panic!("not a calendar: {other:?}"),
    };
    let due_days = |c: &tc_core::calendar::CalendarOut| {
        c.months
            .iter()
            .flat_map(|m| m.weeks.iter().flat_map(|w| w.days.iter().flatten()))
            .filter(|d| d.due.is_some())
            .count()
    };
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar").await.0)), 1);
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar project:Nowhere").await.0)), 0);
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar project:Work").await.0)), 1);
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar +NOSUCHTAG").await.0)), 0);
    // Filters and months go together, in any order.
    assert_eq!(
        cal(run(&mut r, &cfg, "calendar project:Work y").await.0).months.len(),
        12
    );
    // The details report is narrowed the same way.
    let full = parse("calendar.details=full\n").config;
    let rows = |res: CliResult| {
        cal(res)
            .details
            .unwrap()
            .rows
            .iter()
            .map(|x| x.facts.description.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        rows(run(&mut r, &full, "calendar project:Work").await.0),
        ["work thing"]
    );
    // A word that is neither a month argument nor filter-shaped is still a mistake.
    let (res, _) = run(&mut r, &cfg, "calendar Work").await;
    assert!(
        message(&res).contains("Could not recognize argument 'Work'"),
        "{}",
        message(&res)
    );
}

#[tokio::test]
async fn calendar_colours_what_is_due_and_lists_it_in_full_mode() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add this month due:eom").await;
    run(&mut r, &cfg, "add far away due:2031-06-15").await;
    run(&mut r, &cfg, "add hidden due:eom +nocal").await;
    let cal = |res: CliResult| match res {
        CliResult::Calendar(c) => *c,
        other => panic!("not a calendar: {other:?}"),
    };
    let c = cal(run(&mut r, &cfg, "calendar").await.0);
    let due_days = |c: &tc_core::calendar::CalendarOut| {
        c.months
            .iter()
            .flat_map(|m| m.weeks.iter().flat_map(|w| w.days.iter().flatten()))
            .filter(|d| d.due.is_some())
            .count()
    };
    assert_eq!(
        due_days(&c),
        1,
        "this month's, once: `nocal` is left off and 2031 is out of range"
    );
    assert!(c.details.is_none(), "sparse is the default");

    let full = parse("calendar.details=full\n").config;
    let c = cal(run(&mut r, &full, "calendar").await.0);
    let d = c.details.expect("a report of what is due");
    assert_eq!(d.report, "list");
    assert_eq!(
        d.rows.iter().map(|x| x.facts.description.as_str()).collect::<Vec<_>>(),
        ["this month"]
    );
    let far = cal(run(&mut r, &full, "calendar 6 2031").await.0);
    assert_eq!(
        far.details
            .unwrap()
            .rows
            .iter()
            .map(|x| x.facts.description.as_str())
            .collect::<Vec<_>>(),
        ["far away"]
    );

    // Its report can be another one, and it has to exist.
    let long = parse("calendar.details=full\ncalendar.details.report=long\n").config;
    assert_eq!(
        cal(run(&mut r, &long, "calendar").await.0).details.unwrap().report,
        "long"
    );
    let bad = parse("calendar.details=full\ncalendar.details.report=nosuch\n").config;
    let (res, _) = run(&mut r, &bad, "calendar").await;
    assert!(message(&res).contains("calendar.details.report"), "{}", message(&res));
}

#[tokio::test]
async fn udas_and_custom_reports_from_taskrc() {
    let cfg = parse(
        "uda.estimate.type=string\nuda.estimate.label=Size\nuda.estimate.values=big,small\n\
         uda.points.type=numeric\nuda.points.default=1\n\
         report.sized.description=By size\n\
         report.sized.columns=id,estimate,points,description\n\
         report.sized.labels=ID,Size,Pts,Task\n\
         report.sized.sort=estimate-\nreport.sized.filter=status:pending\n",
    )
    .config;
    let mut r = replica();
    run(&mut r, &cfg, "add small thing estimate:small").await;
    run(&mut r, &cfg, "add big thing estimate:big points:8").await;
    let (res, _) = run(&mut r, &cfg, "add bad estimate:huge").await;
    assert!(message(&res).contains("use one of: big, small"), "{}", message(&res));

    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized").await else {
        panic!()
    };
    let order: Vec<&str> = o.rows.iter().map(|r| r.facts.description.as_str()).collect();
    assert_eq!(order, ["big thing", "small thing"]);
    assert_eq!(o.rows[0].facts.extra["points"], "8");
    assert_eq!(o.rows[1].facts.extra["points"], "1", "uda default applied on add");
    assert_eq!(o.description.as_deref(), Some("By size"));

    // Filtering on UDAs from the command line.
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized estimate:big").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized points.after:5").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
    // And modifying them.
    run(&mut r, &cfg, "estimate:small modify estimate:big").await;
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized estimate:big").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 2);
}

#[tokio::test]
async fn context_filters_reads_and_tags_new_tasks() {
    let cfg = parse("context.work.read=+work\ncontext.work.write=+work\ncontext=work\n").config;
    let mut r = replica();
    run(&mut r, &cfg, "add in context").await; // gets +work from the write rule
    let none = Config::default();
    run(&mut r, &none, "add outside context").await;
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "list").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
    assert_eq!(o.rows[0].facts.description, "in context");
    let (CliResult::Report(o), _) = run(&mut r, &none, "list").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 2);
}

#[tokio::test]
async fn read_commands() {
    let cfg = Config::default();
    let mut r = replica();
    run(&mut r, &cfg, "add a project:Home +x").await;
    run(&mut r, &cfg, "add b project:Home +x +y").await;
    run(&mut r, &cfg, "add c").await;

    assert_eq!(message(&run(&mut r, &cfg, "count").await.0), "3");
    assert_eq!(message(&run(&mut r, &cfg, "project:Home count").await.0), "2");
    assert_eq!(message(&run(&mut r, &cfg, "+x +y count").await.0), "1");
    assert_eq!(message(&run(&mut r, &cfg, "ids").await.0), "1 2 3");

    let (CliResult::Table(t), _) = run(&mut r, &cfg, "projects").await else {
        panic!()
    };
    assert_eq!(t.rows, [vec!["(none)", "1"], vec!["Home", "2"]]);
    let (CliResult::Table(t), _) = run(&mut r, &cfg, "tags").await else {
        panic!()
    };
    assert_eq!(t.rows, [vec!["x", "2"], vec!["y", "1"]]);

    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "2 info").await else {
        panic!()
    };
    assert_eq!(tasks[0].facts.description, "b");
    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "2").await else {
        panic!()
    };
    assert_eq!(tasks.len(), 1, "a bare id means info");

    let (CliResult::Json { value }, _) = run(&mut r, &cfg, "project:Home _rows").await else {
        panic!()
    };
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert!(value[0]["uuid"].is_string() && value[0]["urgency"].is_number());

    let (res, _) = run(&mut r, &cfg, "help").await;
    assert!(message(&res).contains("Usage"));
    assert!(message(&run(&mut r, &cfg, "sync").await.0).contains("automatic"));
}

#[tokio::test]
async fn bad_input_gives_errors_not_changes() {
    let cfg = Config::default();
    let mut r = replica();
    run(&mut r, &cfg, "add a").await;
    for (line, needle) in [
        ("add", "description"),
        ("add x due:garbage", "valid date"),
        ("add x priority:Z", "priority"),
        ("project:Home add x", "not a filter"),
        ("list due:garbage", "valid date"),
        ("1 modify depends:99", "no task with id 99"),
        ("1 modify recur:weekly", "due date"),
        ("1 modify status:completed", "can't be set"),
        ("99 done", "No matches"),
    ] {
        let (res, wrote) = run(&mut r, &cfg, line).await;
        assert!(!wrote, "{line}");
        assert!(message(&res).contains(needle), "{line}: {}", message(&res));
    }
    assert_eq!(descs(&mut r).await, ["a"]);
}

#[tokio::test]
async fn overdue_and_due_virtual_tags_work_through_the_cli() {
    let cfg = Config::default();
    let mut r = replica();
    run(&mut r, &cfg, "add late due:-3d").await;
    run(&mut r, &cfg, "add soon due:2d").await;
    run(&mut r, &cfg, "add far due:30d").await;
    run(&mut r, &cfg, "add never").await;
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "overdue").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
    assert_eq!(o.rows[0].facts.description, "late");
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "list +DUE").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 1);
    assert_eq!(o.rows[0].facts.description, "soon");
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "next due.before:7d").await else {
        panic!()
    };
    assert_eq!(o.rows.len(), 2);
    // `next` sorts by urgency: the overdue one first.
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "next").await else {
        panic!()
    };
    assert_eq!(o.rows[0].facts.description, "late");
    let _ = DAY;
}

#[tokio::test]
async fn a_table_row_carries_waiting_and_only_info_lists_every_virtual_tag() {
    let cfg = Config::default();
    let mut r = replica();
    run(&mut r, &cfg, "add hidden wait:5d").await;
    run(&mut r, &cfg, "add shown due:-1d").await;
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "all").await else {
        panic!()
    };
    for row in &o.rows {
        assert!(row.virtual_tags.is_empty(), "a table row lists no virtual tags");
        assert_eq!(row.waiting, row.facts.description == "hidden");
    }
    let json = serde_json::to_string(&o.rows).unwrap();
    assert!(!json.contains("virtual_tags"), "{json}");
    assert_eq!(json.matches("\"waiting\":true").count(), 1, "{json}");

    // Filters ask each task about a virtual tag themselves, so they don't depend on the row's list.
    for (filter, wanted) in [
        ("+WAITING", "hidden"),
        ("+OVERDUE", "shown"),
        ("+PENDING +READY", "shown"),
        ("-WAITING", "shown"),
    ] {
        let (CliResult::Report(o), _) = run(&mut r, &cfg, &format!("{filter} all")).await else {
            panic!()
        };
        let got: Vec<&str> = o.rows.iter().map(|x| x.facts.description.as_str()).collect();
        assert_eq!(got, [wanted], "{filter}");
    }
    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "description:shown info").await else {
        panic!()
    };
    assert!(tasks[0].virtual_tags.contains(&"OVERDUE") && tasks[0].virtual_tags.contains(&"PENDING"));
    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "description:hidden info").await else {
        panic!()
    };
    assert!(tasks[0].waiting && tasks[0].virtual_tags.contains(&"WAITING"));
}

/// Checked against `task +READY` on the same five tasks: a waiting task is not ready, nor one scheduled ahead.
#[tokio::test]
async fn ready_leaves_out_waiting_and_scheduled_ahead() {
    let cfg = Config::default();
    let mut r = replica();
    for line in [
        "add schedpast scheduled:-1d",
        "add schedfuture scheduled:2d",
        "add waitsched wait:3d",
        "add plain",
        "add waitpast wait:-1d",
    ] {
        run(&mut r, &cfg, line).await;
    }
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "+READY all").await else {
        panic!()
    };
    let mut got: Vec<&str> = o.rows.iter().map(|x| x.facts.description.as_str()).collect();
    got.sort();
    assert_eq!(got, ["plain", "schedpast", "waitpast"]);
}

/// The commands the bulk bar sends: the whole uuids of the selected tasks side by side, then the verb, with the
/// click as the answer to every "are you sure?".
#[tokio::test]
async fn bulk_commands_from_the_bar_change_exactly_the_selected_tasks() {
    let cfg = Config::default();
    let mut r = replica();
    for d in ["a", "b", "c", "d", "e"] {
        run(&mut r, &cfg, &format!("add {d} due:tomorrow +old")).await;
    }
    let (CliResult::Text { lines }, _) = run(&mut r, &cfg, "uuids").await else {
        panic!()
    };
    async fn uuid_of(r: &mut R, name: &str) -> String {
        let (CliResult::Text { lines }, _) = run(r, &Config::default(), &format!("description:{name} uuids")).await
        else {
            panic!()
        };
        lines[0].clone()
    }
    let (a, b, c, d) = (
        uuid_of(&mut r, "a").await,
        uuid_of(&mut r, "b").await,
        uuid_of(&mut r, "c").await,
        uuid_of(&mut r, "d").await,
    );
    assert_eq!(lines.len(), 5);
    let yes = |uuids: &[&String]| Options {
        approved: Some(uuids.iter().map(|u| (*u).clone()).collect()),
        confirmed: true,
        ..Options::default()
    };

    // Modify four tasks: set a project, add and remove tags, clear the due date.
    let line = format!("{a} {b} {c} {d} modify project:Home +x +y -old due:");
    let (res, wrote) = run_opts(&mut r, &cfg, &line, yes(&[&a, &b, &c, &d])).await;
    assert!(wrote, "{res:?}");
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "all").await else {
        panic!()
    };
    for row in &o.rows {
        let touched = row.facts.description != "e";
        assert_eq!(
            row.facts.project.as_deref(),
            touched.then_some("Home"),
            "{}",
            row.facts.description
        );
        assert_eq!(row.facts.due.is_none(), touched, "{}", row.facts.description);
        let tags: Vec<&str> = row.facts.tags.iter().map(String::as_str).collect();
        assert_eq!(tags == ["x", "y"], touched, "{}: {tags:?}", row.facts.description);
        assert_eq!(tags.contains(&"old"), !touched, "{}", row.facts.description);
    }

    // Complete two, delete two.
    let (_, wrote) = run_opts(&mut r, &cfg, &format!("{a} {b} done"), yes(&[&a, &b])).await;
    assert!(wrote);
    let (_, wrote) = run_opts(&mut r, &cfg, &format!("{c} {d} delete"), yes(&[&c, &d])).await;
    assert!(wrote);
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "all").await else {
        panic!()
    };
    let st = |name: &str| {
        o.rows
            .iter()
            .find(|x| x.facts.description == name)
            .unwrap()
            .facts
            .status
            .clone()
    };
    assert_eq!(
        ["a", "b", "c", "d", "e"].map(st),
        ["completed", "completed", "deleted", "deleted", "pending"]
    );
}

mod with_sync {
    use super::*;
    use taskchampion::Server;
    use tc_core::{CloudServer, MemStore};

    async fn server(store: &MemStore) -> Box<dyn Server> {
        Box::new(CloudServer::new(store.clone(), b"secret").await.unwrap())
    }

    async fn sync(r: &mut R, store: &MemStore) {
        r.sync(&mut server(store).await, true).await.unwrap();
    }

    #[tokio::test]
    async fn undo_works_after_the_change_was_synced_and_syncs_back() {
        let store = MemStore::new();
        let cfg = Config::default();
        let (mut a, mut b) = (replica(), replica());
        run(&mut a, &cfg, "add finish report").await;
        run(&mut a, &cfg, "1 done").await;
        sync(&mut a, &store).await; // the Worker does this after every write

        let (res, wrote) = run_yes(&mut a, &cfg, "undo").await;
        assert!(wrote, "{}", message(&res));
        sync(&mut a, &store).await;

        sync(&mut b, &store).await;
        let f = load_facts(&mut b).await.unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].status, "pending", "the undo must propagate to other replicas");
        assert_eq!(f[0].end, None);
    }

    /// Like `run`, but with an explicit undo stack, so two replicas don't share one.
    async fn run_own(r: &mut R, cfg: &Config, line: &str, undo: &mut UndoStack) -> (CliResult, bool) {
        let yes = Options {
            confirmed: true,
            ..Options::default()
        }; // `undo` asks first
        let d = execute(r, cfg, clock(), &split_words(line), yes, undo).await;
        (d.result, d.wrote)
    }

    #[tokio::test]
    async fn undo_refuses_when_another_replica_changed_the_task() {
        let store = MemStore::new();
        let cfg = Config::default();
        let (mut a, mut b) = (replica(), replica());
        let (mut ua, mut ub) = (UndoStack::default(), UndoStack::default());
        run_own(&mut a, &cfg, "add shared", &mut ua).await;
        sync(&mut a, &store).await;
        sync(&mut b, &store).await;

        run_own(&mut a, &cfg, "1 modify project:Mine", &mut ua).await;
        sync(&mut a, &store).await;
        // Someone else edits the same property afterwards.
        run_own(&mut b, &cfg, "1 modify project:Theirs", &mut ub).await;
        sync(&mut b, &store).await;
        sync(&mut a, &store).await;

        let (res, wrote) = run_own(&mut a, &cfg, "undo", &mut ua).await;
        assert!(!wrote);
        assert!(message(&res).contains("can't undo"), "{}", message(&res));
        let f = &load_facts(&mut a).await.unwrap()[0];
        assert_eq!(f.project.as_deref(), Some("Theirs"), "their change must survive");
        // The refused undo stays on the stack, so nothing is silently lost.
        assert!(!ua.is_empty());
    }

    #[tokio::test]
    async fn repeated_undo_walks_back_through_commands() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add one").await;
        run(&mut r, &cfg, "1 modify +a").await;
        run(&mut r, &cfg, "1 modify +b").await;
        run_yes(&mut r, &cfg, "undo").await;
        let f = &load_facts(&mut r).await.unwrap()[0];
        assert!(f.tags.contains("a") && !f.tags.contains("b"));
        run_yes(&mut r, &cfg, "undo").await;
        assert!(load_facts(&mut r).await.unwrap()[0].tags.is_empty());
        run_yes(&mut r, &cfg, "undo").await; // undoes the add itself
        let f = load_facts(&mut r).await.unwrap();
        assert!(f.is_empty() || f[0].status == "deleted", "{f:?}");
        let (res, wrote) = run_yes(&mut r, &cfg, "undo").await;
        assert!(!wrote && message(&res).contains("Nothing to undo"));
    }
}

#[tokio::test]
async fn orphan_udas_are_shown_but_read_only() {
    use taskchampion::{Operations, Status};
    let cfg = parse("uda.estimate.type=string\n").config;
    let mut r = replica();

    // A task carrying a property the taskrc doesn't define, as if written by another machine.
    let mut ops = Operations::new();
    let uuid = uuid::Uuid::new_v4();
    let mut t = r.create_task(uuid, &mut ops).await.unwrap();
    t.set_status(Status::Pending, &mut ops).unwrap();
    t.set_description("old task".into(), &mut ops).unwrap();
    t.set_value("legacy", Some("kept".into()), &mut ops).unwrap();
    t.set_value("estimate", Some("big".into()), &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();

    // Shown: in reports (as an orphan) and in info/export.
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "list").await else {
        panic!()
    };
    assert_eq!(o.rows[0].orphans, ["legacy"]);
    assert_eq!(o.rows[0].facts.extra["legacy"], "kept");
    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "1 info").await else {
        panic!()
    };
    assert_eq!(tasks[0].facts.extra["legacy"], "kept");
    let (res, _) = run(&mut r, &cfg, "+ORPHAN count").await;
    assert_eq!(message(&res), "1");

    // Not editable: a clear error, no change, and the text isn't swallowed into the description.
    let (res, wrote) = run(&mut r, &cfg, "1 modify legacy:changed").await;
    assert!(!wrote && message(&res).contains("read-only"), "{}", message(&res));
    let f = &load_facts(&mut r).await.unwrap()[0];
    assert_eq!(f.extra["legacy"], "kept");
    assert_eq!(f.description, "old task");

    // Defined UDAs on the same task remain editable, and editing doesn't disturb the orphan.
    let (res, wrote) = run(&mut r, &cfg, "1 modify estimate:small +x").await;
    assert!(wrote, "{}", message(&res));
    let f = &load_facts(&mut r).await.unwrap()[0];
    assert_eq!(f.extra["estimate"], "small");
    assert_eq!(f.extra["legacy"], "kept");
}

#[tokio::test]
async fn responses_say_how_the_line_was_understood() {
    let cfg = parse("report.work.columns=id,description\nreport.work.filter=project:Work\n").config;
    let info = |line: &'static str| {
        let (cfgc, line) = (cfg.clone(), line);
        async move {
            let mut rr = replica();
            let d = execute(
                &mut rr,
                &cfgc,
                clock(),
                &split_words(line),
                Options::default(),
                &mut UndoStack::default(),
            )
            .await;
            d.command
        }
    };
    let c = info("project:Home +a work").await.unwrap();
    assert_eq!(
        (c.name.as_str(), c.report, c.filter.clone()),
        ("work", true, vec!["project:Home".to_string(), "+a".into()])
    );
    let c = info("task 3 modify +x").await.unwrap();
    assert_eq!(
        (c.name.as_str(), c.report, c.filter.clone()),
        ("modify", false, vec!["3".to_string()])
    );
    // Abbreviations resolve to the canonical name; the default report is reported as such.
    assert_eq!(info("3 ann hi").await.unwrap().name, "annotate");
    let c = info("+x").await.unwrap();
    assert_eq!((c.name.as_str(), c.report), ("next", true));
    // An unparseable line has no command info.
    assert!(info("3 de").await.is_none());
}

mod overrides_and_journal {
    use super::*;

    fn notes(f: &tc_core::model::Facts) -> Vec<String> {
        f.annotations.iter().map(|a| a.text.clone()).collect()
    }

    async fn only(r: &mut R) -> tc_core::model::Facts {
        load_facts(r).await.unwrap().remove(0)
    }

    #[tokio::test]
    async fn rc_sort_override_resorts_a_report_and_reports_the_sort_used() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add zebra priority:L due:3d").await;
        run(&mut r, &cfg, "add apple priority:H due:9d").await;
        run(&mut r, &cfg, "add mango priority:M due:1d").await;

        let order = |res: CliResult| match res {
            CliResult::Report(o) => (
                o.sort.clone(),
                o.rows.iter().map(|x| x.facts.description.clone()).collect::<Vec<_>>(),
            ),
            other => panic!("{other:?}"),
        };
        // The `list` report's own sort is start-,due+,project+,urgency-.
        let (sort, rows) = order(run(&mut r, &cfg, "list").await.0);
        assert_eq!(sort.as_deref(), Some("start-,due+,project+,urgency-"));
        assert_eq!(rows, ["mango", "zebra", "apple"]);

        let (sort, rows) = order(run(&mut r, &cfg, "rc.report.list.sort:description+ list").await.0);
        assert_eq!(sort.as_deref(), Some("description+"));
        assert_eq!(rows, ["apple", "mango", "zebra"]);
        let (_, rows) = order(run(&mut r, &cfg, "list rc.report.list.sort=description-").await.0);
        assert_eq!(rows, ["zebra", "mango", "apple"]);
        let (_, rows) = order(run(&mut r, &cfg, "rc.report.list.sort:priority- list").await.0);
        assert_eq!(rows, ["apple", "mango", "zebra"]);

        // It applies to one command only.
        let (_, rows) = order(run(&mut r, &cfg, "list").await.0);
        assert_eq!(rows, ["mango", "zebra", "apple"]);
    }

    #[tokio::test]
    async fn rc_overrides_are_validated() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add a").await;
        let (res, _) = run(&mut r, &cfg, "rc.report.list.sort:nonsense+ list").await;
        assert!(
            message(&res).contains("'nonsense' column is not a valid sort field"),
            "{}",
            message(&res)
        );
        let (res, _) = run(&mut r, &cfg, "rc.report.list.sort:due list").await;
        assert!(message(&res).contains("must end in + or -"), "{}", message(&res));
        let (res, _) = run(&mut r, &cfg, "rc.report.nope.sort:due+ list").await;
        assert!(message(&res).contains("not a report"), "{}", message(&res));
        // Credentials can't be injected through the command line either.
        for line in [
            "rc.sync.encryption_secret:x list",
            "rc.sync.aws.bucket=b list",
            "rc.my.api_token:t list",
        ] {
            let (res, _) = run(&mut r, &cfg, line).await;
            assert!(
                message(&res).contains("can't be set from a command line"),
                "{line}: {}",
                message(&res)
            );
        }
        // Harmless settings the real task accepts are fine.
        for line in ["rc.verbose:nothing list", "rc.confirmation=no list"] {
            assert!(
                matches!(run(&mut r, &cfg, line).await.0, CliResult::Report(_)),
                "{line}"
            );
        }
    }

    #[tokio::test]
    async fn rc_can_override_filters_columns_and_context() {
        let cfg = parse("context.w.read=+work\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add one +work").await;
        run(&mut r, &cfg, "add two").await;
        let rows = |res: CliResult| match res {
            CliResult::Report(o) => o.rows.len(),
            o => panic!("{o:?}"),
        };
        assert_eq!(rows(run(&mut r, &cfg, "list").await.0), 2);
        assert_eq!(rows(run(&mut r, &cfg, "rc.context:w list").await.0), 1);
        assert_eq!(rows(run(&mut r, &cfg, "rc.report.list.filter:+work list").await.0), 1);
        let CliResult::Report(o) = run(
            &mut r,
            &cfg,
            "rc.report.list.columns:id,description rc.report.list.labels:N,T list",
        )
        .await
        .0
        else {
            panic!()
        };
        assert_eq!(
            o.columns.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
            ["N", "T"]
        );
    }

    #[tokio::test]
    async fn journal_records_start_and_stop() {
        let cfg = parse("journal.time=on\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add track me").await;
        run(&mut r, &cfg, "1 start").await;
        assert_eq!(notes(&only(&mut r).await), ["Started task"]);
        assert!(only(&mut r).await.start.is_some());

        // Starting an active task is a no-op: no second journal entry.
        let (res, wrote) = run(&mut r, &cfg, "1 start").await;
        assert!(!wrote && message(&res).contains("already active"), "{}", message(&res));
        assert_eq!(notes(&only(&mut r).await), ["Started task"]);

        run(&mut r, &cfg, "1 stop").await;
        let f = only(&mut r).await;
        assert_eq!(notes(&f), ["Started task", "Stopped task"]);
        assert!(f.start.is_none());

        let (res, wrote) = run(&mut r, &cfg, "1 stop").await;
        assert!(!wrote && message(&res).contains("not active"), "{}", message(&res));
        assert_eq!(notes(&only(&mut r).await).len(), 2);
    }

    #[tokio::test]
    async fn done_on_an_active_task_stops_it_and_journals() {
        let cfg = parse("journal.time=1\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add finish me").await;
        run(&mut r, &cfg, "1 start").await;
        run(&mut r, &cfg, "1 done").await;
        let f = only(&mut r).await;
        assert_eq!(f.status, "completed");
        assert!(f.start.is_none(), "completing must stop the task");
        assert_eq!(notes(&f), ["Started task", "Stopped task"]);
    }

    #[tokio::test]
    async fn done_on_an_idle_task_adds_no_journal_entry() {
        let cfg = parse("journal.time=on\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add quick").await;
        run(&mut r, &cfg, "1 done").await;
        assert!(notes(&only(&mut r).await).is_empty());
    }

    #[tokio::test]
    async fn journal_text_is_configurable_and_off_by_default() {
        let cfg =
            parse("journal.time=on\njournal.time.start.annotation=Clock in\njournal.time.stop.annotation=Clock out\n")
                .config;
        let mut r = replica();
        run(&mut r, &cfg, "add custom").await;
        run(&mut r, &cfg, "1 start").await;
        run(&mut r, &cfg, "1 stop").await;
        assert_eq!(notes(&only(&mut r).await), ["Clock in", "Clock out"]);

        let off = Config::default();
        let mut r = replica();
        run(&mut r, &off, "add plain").await;
        run(&mut r, &off, "1 start").await;
        run(&mut r, &off, "1 stop").await;
        assert!(
            notes(&only(&mut r).await).is_empty(),
            "journal.time is off unless enabled"
        );
    }

    /// `(kind, property)` of every change `_history` lists.
    fn history_of(res: CliResult) -> Vec<(String, String)> {
        let CliResult::Json { value } = res else {
            panic!("{res:?}")
        };
        value
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|e| e["changes"].as_array().unwrap())
            .map(|c| {
                (
                    c["kind"].as_str().unwrap().to_owned(),
                    c["prop"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn history_lists_what_changed_unless_journal_info_is_off() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add Alpha project:home +x priority:H").await;
        run(&mut r, &cfg, "1 modify project:work -x +y").await;
        run(&mut r, &cfg, "1 annotate a note").await;
        run(&mut r, &cfg, "1 start").await;
        run(&mut r, &cfg, "1 done").await;
        let id = only(&mut r).await.uuid.to_string();

        let h = history_of(run(&mut r, &cfg, &format!("_history {id}")).await.0);
        let has = |k: &str, p: &str| h.iter().any(|(kk, pp)| kk == k && pp == p);
        assert!(
            has("set", "description") && has("set", "project") && has("set", "priority"),
            "{h:?}"
        );
        assert!(
            has("tag_added", "x") && has("tag_added", "y") && has("tag_deleted", "x"),
            "{h:?}"
        );
        assert!(has("changed", "project"), "{h:?}");
        assert!(
            h.iter().any(|(k, p)| k == "note_added" && p.starts_with("annotation_")),
            "{h:?}"
        );
        assert!(
            has("set", "start") && has("deleted", "start") && has("changed", "status"),
            "{h:?}"
        );
        assert!(
            !h.iter().any(|(_, p)| p == "modified"),
            "the modification time is never listed"
        );

        // A Taskwarrior config says `journal.info=off` or `0`; the web replica follows.
        for off in ["journal.info=off\n", "journal.info=0\n"] {
            let h = history_of(run(&mut r, &parse(off).config, &format!("_history {id}")).await.0);
            assert!(h.is_empty(), "{off}: {h:?}");
        }
        // The command line can switch it too, like any setting.
        let h = history_of(run(&mut r, &cfg, &format!("rc.journal.info:off _history {id}")).await.0);
        assert!(h.is_empty());
    }

    #[tokio::test]
    async fn the_history_stays_out_of_reports_exports_and_info_and_is_asked_for_alone() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add Alpha").await;
        run(&mut r, &cfg, "1 modify +x").await;
        let (res, _) = run(&mut r, &cfg, "_rows").await;
        let CliResult::Json { value } = res else {
            panic!("{res:?}")
        };
        assert!(value[0].get("history").is_none(), "{value}");
        let id = value[0]["uuid"].as_str().unwrap().to_owned();
        let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "1 info").await else {
            panic!()
        };
        assert!(!serde_json::to_string(&tasks).unwrap().contains("history"));
        assert!(!history_of(run(&mut r, &cfg, &format!("_history {id}")).await.0).is_empty());
        // Only a whole uuid of one task; anything else says so.
        for bad in [
            "_history",
            "_history 1",
            "_history nope",
            &format!("_history {id} {id}"),
        ] {
            let (res, wrote) = run(&mut r, &cfg, bad).await;
            assert!(matches!(res, CliResult::Error { .. }) && !wrote, "{bad}: {res:?}");
        }
        // A task that is not there has no history.
        let none = run(&mut r, &cfg, &format!("_history {}", uuid::Uuid::from_u128(77)))
            .await
            .0;
        assert!(history_of(none).is_empty());
        // Not a command `commands` lists: it is the app's own, like `_rows`.
        let (CliResult::Table(t), _) = run(&mut r, &cfg, "commands").await else {
            panic!()
        };
        assert!(!t.rows.iter().any(|row| row[0] == "_history" || row[0] == "_rows"));
    }

    #[tokio::test]
    async fn start_reopens_a_completed_task() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add again").await;
        run(&mut r, &cfg, "1 done").await;
        assert_eq!(only(&mut r).await.status, "completed");
        let id = only(&mut r).await.uuid.to_string();
        let (res, wrote) = run(&mut r, &cfg, &format!("{id} start")).await;
        assert!(wrote, "{}", message(&res));
        let f = only(&mut r).await;
        assert_eq!(f.status, "pending");
        assert!(f.start.is_some());
    }

    #[tokio::test]
    async fn active_time_shows_up_on_rows_when_journalling() {
        let on = parse("journal.time=on\n").config;
        let mut r = replica();
        run(&mut r, &on, "add timed").await;
        run(&mut r, &on, "1 start").await;
        let CliResult::Info { tasks } = run(&mut r, &on, "1 info").await.0 else {
            panic!()
        };
        assert!(
            tasks[0].active_seconds.is_some(),
            "an active task reports its tracked time"
        );
        let CliResult::Info { tasks } = run(&mut r, &Config::default(), "1 info").await.0 else {
            panic!()
        };
        assert!(tasks[0].active_seconds.is_none(), "no journal, no tracked time");
    }
}

#[tokio::test]
async fn info_rows_carry_the_sessions_when_journalling() {
    let cfg = parse("journal.time=on\n").config;
    let mut r = replica();
    run(&mut r, &cfg, "add track").await;
    run(&mut r, &cfg, "1 start").await;
    run(&mut r, &cfg, "1 stop").await;
    run(&mut r, &cfg, "1 start").await;
    let CliResult::Info { tasks } = run(&mut r, &cfg, "1 info").await.0 else {
        panic!()
    };
    let s = &tasks[0].sessions;
    assert_eq!(s.len(), 2, "{s:?}");
    assert!(
        s[0].end.is_some() && s[1].end.is_none(),
        "the second session is still running: {s:?}"
    );
    assert!(s[0].start <= s[0].end.unwrap());
    // Off by default: no sessions without journal.time.
    let CliResult::Info { tasks } = run(&mut r, &Config::default(), "1 info").await.0 else {
        panic!()
    };
    assert!(tasks[0].sessions.is_empty());
}

mod recurrence {
    use super::*;
    use tc_core::model::Facts;

    async fn all(r: &mut R) -> Vec<Facts> {
        load_facts(r).await.unwrap()
    }

    fn template(v: &[Facts]) -> &Facts {
        let mut t = v.iter().filter(|f| f.status == "recurring");
        let one = t.next().expect("a template");
        assert!(t.next().is_none(), "exactly one template");
        one
    }

    fn instances(v: &[Facts]) -> Vec<&Facts> {
        let mut c: Vec<_> = v.iter().filter(|f| f.parent.is_some()).collect();
        c.sort_by_key(|f| f.imask);
        c
    }

    #[tokio::test]
    async fn recurrence_is_on_unless_the_taskrc_turns_it_off() {
        // Nothing configured, as for a user who never imported a taskrc: Taskwarrior's default.
        // Due in the future, so exactly one instance is kept ready (`recurrence.limit` is 1).
        let mut r = replica();
        run(&mut r, &Config::default(), "add Water plants recur:daily due:tomorrow").await;
        run(&mut r, &Config::default(), "list").await;
        let v = all(&mut r).await;
        assert_eq!(v.len(), 2, "a template and its first instance: {v:?}");
        assert_eq!(instances(&v).len(), 1);
        // Turned off, none are created.
        let off = parse("recurrence=off\n").config;
        let mut r = replica();
        run(&mut r, &off, "add Water plants recur:daily due:tomorrow").await;
        run(&mut r, &off, "list").await;
        assert_eq!(all(&mut r).await.len(), 1);
    }

    #[tokio::test]
    async fn adding_with_recur_makes_a_template_that_generates_nothing_while_recurrence_is_off() {
        let cfg = parse("recurrence=off\n").config;
        let mut r = replica();
        let (res, _) = run(&mut r, &cfg, "add Water plants recur:daily due:yesterday").await;
        assert_eq!(message(&res), "Created task 1.");
        run(&mut r, &cfg, "list").await;
        let v = all(&mut r).await;
        assert_eq!(v.len(), 1, "no instances with recurrence=off");
        assert_eq!(template(&v).recur.as_deref(), Some("daily"));
    }

    #[tokio::test]
    async fn recur_needs_a_due_date_and_a_valid_period() {
        let cfg = Config::default();
        let mut r = replica();
        let (res, wrote) = run(&mut r, &cfg, "add Nope recur:weekly").await;
        assert!(!wrote);
        assert!(message(&res).contains("due"), "{}", message(&res));
        let (res, _) = run(&mut r, &cfg, "add Nope due:tomorrow recur:fortnightish").await;
        assert!(message(&res).starts_with("ERROR"), "{}", message(&res));
        assert!(all(&mut r).await.is_empty());
    }

    #[tokio::test]
    async fn with_recurrence_on_due_instances_appear_and_track_their_parents_mask() {
        // `recurrence.limit` counts upcoming instances, finished or not, so ask for two.
        let cfg = parse("recurrence=on\nrecurrence.limit=2\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add Water plants recur:daily due:3d").await;
        run(&mut r, &cfg, "list").await;
        let v = all(&mut r).await;
        let kids = instances(&v);
        assert_eq!(kids.len(), 2, "{kids:?}");
        let parent = template(&v);
        assert_eq!(kids[0].parent, Some(parent.uuid));
        assert_eq!(kids[0].description, "Water plants");
        assert_eq!(kids[0].status, "pending");
        assert_eq!(kids[0].recur.as_deref(), Some("daily"));
        assert_eq!(kids[0].due, parent.due);
        assert_eq!(kids[1].due, parent.due.map(|d| d + DAY));
        assert_eq!(parent.mask.as_deref(), Some("--"));

        // Completing an instance is recorded in the parent's mask; the other stays pending.
        // By full uuid: both instances share an entry time, so their numeric ids aren't ordered by index,
        // and a short prefix with no letter in it (1 time in 40) would be read as a task number.
        let first = kids[0].uuid.to_string();
        let (res, _) = run(&mut r, &cfg, &format!("{first} done")).await;
        assert!(message(&res).contains("Completed"), "{}", message(&res));
        let v = all(&mut r).await;
        assert_eq!(template(&v).mask.as_deref(), Some("+-"));
        let kids = instances(&v);
        assert_eq!(kids.len(), 2, "completing doesn't create or lose instances: {kids:?}");
        assert_eq!(kids[0].status, "completed");
        assert_eq!(kids[1].status, "pending");
    }

    #[tokio::test]
    async fn the_template_cannot_be_completed_or_started_but_can_be_edited() {
        // `confirmation=no`: edit only the task named, so no question is asked about its instances.
        let cfg = parse("recurrence.confirmation=no\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add Pay rent recur:monthly due:1d").await;
        // Numeric ids tie between the template and its first instance, so use the uuid.
        let tpl = template(&all(&mut r).await).uuid.to_string();
        let tpl = tpl.as_str();
        let (res, _) = run(&mut r, &cfg, &format!("{tpl} done")).await;
        assert!(message(&res).contains("not pending"), "{}", message(&res));
        assert_eq!(template(&all(&mut r).await).status, "recurring");
        let (res, _) = run(&mut r, &cfg, &format!("{tpl} modify priority:H")).await;
        assert!(!message(&res).starts_with("ERROR"), "{}", message(&res));
        assert_eq!(template(&all(&mut r).await).priority.as_deref(), Some("H"));
    }

    #[tokio::test]
    async fn editing_the_template_updates_its_pending_instances_but_not_their_dates() {
        let cfg = parse("recurrence=on\nrecurrence.confirmation=yes\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add Water plants recur:daily due:3d").await;
        run(&mut r, &cfg, "list").await;
        let before = instances(&all(&mut r).await)[0].due;
        run(&mut r, &cfg, "1 modify Feed plants priority:H +green").await;
        let v = all(&mut r).await;
        let kid = instances(&v)[0];
        assert_eq!(kid.description, "Feed plants");
        assert_eq!(kid.priority.as_deref(), Some("H"));
        assert!(kid.tags.contains("green"));
        assert_eq!(kid.due, before, "dates stay per instance");
    }

    #[tokio::test]
    async fn deleting_the_template_asks_then_removes_its_pending_instances_too() {
        let cfg = parse("recurrence=on\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add Water plants recur:daily due:3d").await;
        run(&mut r, &cfg, "list").await;
        let (res, wrote) = run(&mut r, &cfg, "1 delete").await;
        assert!(!wrote);
        assert!(message(&res).starts_with("CONFIRM"), "{}", message(&res));
        let (res, wrote) = run(&mut r, &cfg, "1 delete").await;
        assert!(!wrote, "still asking: {}", message(&res));
        let (_, wrote) = run_yes(&mut r, &cfg, "1 delete").await;
        assert!(wrote);
        let v = all(&mut r).await;
        assert!(
            v.iter().all(|f| f.status == "deleted"),
            "{:?}",
            v.iter().map(|f| &f.status).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn an_until_date_in_the_past_expires_instances_and_the_template() {
        let cfg = parse("recurrence=on\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add Old habit recur:daily due:3d until:yesterday").await;
        // Pass 1 creates and expires the instance; the series is retired once none is pending.
        for _ in 0..3 {
            run(&mut r, &cfg, "list").await;
        }
        let v = all(&mut r).await;
        assert!(
            v.iter().all(|f| f.status == "deleted"),
            "{:?}",
            v.iter().map(|f| &f.status).collect::<Vec<_>>()
        );
    }

    // ---- recurrence.confirmation

    /// A template with two open instances, `recurrence.confirmation` set to `mode`.
    async fn series(mode: &str) -> (R, Config) {
        let cfg = parse(&format!(
            "recurrence=on\nrecurrence.limit=2\nrecurrence.confirmation={mode}\n"
        ))
        .config;
        let mut r = replica();
        run(&mut r, &cfg, "add Water plants recur:daily due:3d").await;
        run(&mut r, &cfg, "list").await;
        (r, cfg)
    }

    async fn descriptions(r: &mut R) -> Vec<(String, String)> {
        let mut v: Vec<_> = all(r).await.into_iter().map(|f| (f.status, f.description)).collect();
        v.sort();
        v
    }

    /// Say yes (`true`) to the question just asked in `res`, or no (`false`).
    fn answer(res: &CliResult, yes: bool) -> Options {
        let (ask, items) = asked(res);
        assert_eq!(ask, Ask::Extras);
        Options {
            extras: Some(if yes {
                items.iter().map(|i| i.key.clone()).collect()
            } else {
                vec![]
            }),
            ..Options::default()
        }
    }

    #[tokio::test]
    async fn by_default_editing_a_recurring_task_asks_and_changes_nothing_until_answered() {
        let (mut r, cfg) = series("prompt").await;
        let before = descriptions(&mut r).await;
        let (res, wrote) = run(&mut r, &cfg, "1 modify Feed plants").await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert!(
            ask == Ask::Extras && items[0].question.contains("pending recurrences"),
            "{res:?}"
        );
        assert_eq!(descriptions(&mut r).await, before, "asking must not write");
        // Unset means the same as `prompt`.
        let cfg = parse("recurrence=on\nrecurrence.limit=2\n").config;
        let (res, _) = run(&mut r, &cfg, "1 modify Feed plants").await;
        assert_eq!(asked(&res).0, Ask::Extras, "{res:?}");
    }

    #[tokio::test]
    async fn answering_yes_changes_the_whole_series_and_no_changes_only_the_task() {
        let (mut r, cfg) = series("prompt").await;
        let (asked_res, _) = run(&mut r, &cfg, "1 modify Feed plants").await;
        let (_, wrote) = run_opts(&mut r, &cfg, "1 modify Feed plants", answer(&asked_res, true)).await;
        assert!(wrote);
        assert!(
            all(&mut r).await.iter().all(|f| f.description == "Feed plants"),
            "{:?}",
            descriptions(&mut r).await
        );

        let (mut r, cfg) = series("prompt").await;
        let (asked_res, _) = run(&mut r, &cfg, "1 modify Feed plants").await;
        let (_, wrote) = run_opts(&mut r, &cfg, "1 modify Feed plants", answer(&asked_res, false)).await;
        assert!(wrote);
        let v = all(&mut r).await;
        assert_eq!(
            template(&v).description,
            "Feed plants",
            "the edited task itself always changes"
        );
        assert!(
            instances(&v).iter().all(|f| f.description == "Water plants"),
            "{:?}",
            descriptions(&mut r).await
        );
    }

    #[tokio::test]
    async fn yes_propagates_without_asking_and_no_never_does() {
        let (mut r, cfg) = series("yes").await;
        let (res, wrote) = run(&mut r, &cfg, "1 modify +green").await;
        assert!(wrote, "{res:?}");
        assert!(all(&mut r).await.iter().all(|f| f.tags.contains("green")));

        for off in ["no", "off", "false", "whatever"] {
            let (mut r, cfg) = series(off).await;
            let (res, wrote) = run(&mut r, &cfg, "1 modify +green").await;
            assert!(wrote && !matches!(res, CliResult::Confirm { .. }), "{off}: {res:?}");
            let v = all(&mut r).await;
            assert!(template(&v).tags.contains("green"));
            assert!(instances(&v).iter().all(|f| !f.tags.contains("green")), "{off}");
        }
    }

    #[tokio::test]
    async fn editing_an_instance_reaches_its_siblings_and_the_template() {
        let (mut r, cfg) = series("yes").await;
        let v = all(&mut r).await;
        let first = instances(&v)[0].uuid.to_string();
        let (_, wrote) = run(&mut r, &cfg, &format!("{first} modify priority:H project:Garden")).await;
        assert!(wrote);
        let v = all(&mut r).await;
        assert!(
            v.iter()
                .all(|f| f.priority.as_deref() == Some("H") && f.project.as_deref() == Some("Garden")),
            "{v:?}"
        );

        // Dates stay per instance: moving one instance does not move the others.
        let due_before: Vec<_> = instances(&v).iter().map(|f| f.due).collect();
        let first = instances(&v)[0].uuid.to_string();
        let (res, _) = run(&mut r, &cfg, &format!("{first} modify due:10d")).await;
        assert!(!message(&res).starts_with("ERROR"), "{}", message(&res));
        let v = all(&mut r).await;
        let due_after: Vec<_> = instances(&v).iter().map(|f| f.due).collect();
        assert_ne!(due_before[0], due_after[0]);
        assert_eq!(due_before[1], due_after[1], "the sibling's date is untouched");
    }

    #[tokio::test]
    async fn ordinary_tasks_are_never_asked_about() {
        let cfg = parse("recurrence.confirmation=prompt\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add plain").await;
        let (res, wrote) = run(&mut r, &cfg, "1 modify +x").await;
        assert!(wrote && !matches!(res, CliResult::Confirm { .. }), "{res:?}");
    }
}

mod aliases_and_abbreviations {
    use super::*;

    /// A typed line: aliases stand for their words.
    async fn typed(r: &mut R, cfg: &Config, line: &str) -> (CliResult, bool) {
        run_opts(
            r,
            cfg,
            line,
            Options {
                typed: true,
                ..Options::default()
            },
        )
        .await
    }

    fn is_version(res: &CliResult) -> bool {
        matches!(res, CliResult::Text { lines } if lines.iter().any(|l| l.contains("tc-core")))
    }

    #[tokio::test]
    async fn taskwarriors_own_aliases_work_without_any_taskrc() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add old thing").await;
        // `rm` is `delete`, which asks first.
        let (res, wrote) = typed(&mut r, &cfg, "1 rm").await;
        assert!(
            !wrote && message(&res).starts_with("CONFIRM: Delete task 1"),
            "{}",
            message(&res)
        );
        // `burndown` is `burndown.weekly`.
        let (res, _) = typed(&mut r, &cfg, "burndown").await;
        let CliResult::Burndown(b) = res else { panic!("{res:?}") };
        assert_eq!(b.title, "Weekly Burndown");
    }

    #[tokio::test]
    async fn the_taskrc_adds_changes_and_empties_aliases() {
        let mut r = replica();
        run(&mut r, &Config::default(), "add Pay rent project:home +bill").await;
        run(&mut r, &Config::default(), "add Walk dog").await;
        let cfg = parse("alias.bills=project:home +bill list\nalias.rm=done\nalias.hush=\n").config;
        // One alias, several words: a filter and a report.
        let (res, _) = typed(&mut r, &cfg, "bills").await;
        let CliResult::Report(o) = res else { panic!("{res:?}") };
        assert_eq!(
            o.rows.iter().map(|x| x.facts.description.as_str()).collect::<Vec<_>>(),
            ["Pay rent"]
        );
        // The taskrc's `rm` replaces the built-in one.
        let (res, wrote) = typed(&mut r, &cfg, "description:Walk rm").await;
        assert!(wrote, "{}", message(&res));
        assert_eq!(message(&res), "Completed 1 task.");
        // An empty alias makes the word vanish.
        let (res, _) = typed(&mut r, &cfg, "hush list").await;
        assert!(matches!(res, CliResult::Report(_)), "{res:?}");
    }

    #[tokio::test]
    async fn an_alias_can_use_another_and_a_loop_ends() {
        let mut r = replica();
        run(&mut r, &Config::default(), "add Pay rent").await;
        let cfg = parse("alias.one=two\nalias.two=count\nalias.ping=pong\nalias.pong=ping\n").config;
        let (res, _) = typed(&mut r, &cfg, "one").await;
        assert_eq!(message(&res), "1");
        // A cycle gives up after Taskwarrior's ten rounds instead of hanging.
        let _ = typed(&mut r, &cfg, "ping").await;
    }

    #[tokio::test]
    async fn aliases_are_for_typed_lines_not_for_the_gui_or_after_a_double_dash() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add keep").await;
        // Pre-split arguments (the GUI) are taken as they are: this note is the word `rm`.
        let (res, wrote) = run(&mut r, &cfg, "1 annotate rm").await;
        assert!(wrote, "{}", message(&res));
        let f = load_facts(&mut r).await.unwrap();
        assert_eq!(f[0].annotations[0].text, "rm");
        // After `--` a typed line keeps the word too.
        let args: Vec<String> = ["1", "annotate", "--", "rm"].iter().map(|s| s.to_string()).collect();
        assert_eq!(tc_core::cli::expand_aliases(&args, &cfg), args);
        let args: Vec<String> = ["rm", "1"].iter().map(|s| s.to_string()).collect();
        assert_eq!(tc_core::cli::expand_aliases(&args, &cfg), ["delete", "1"]);
    }

    #[tokio::test]
    async fn abbreviations_must_reach_abbreviation_minimum() {
        let mut r = replica();
        run(&mut r, &Config::default(), "add Pay rent project:home").await;
        let at = |n: u32| parse(&format!("abbreviation.minimum={n}\n")).config;
        // Checked against task 3.5.0: `ve` is `version` at 2 but not at 3, `ver` not at 4.
        for (min, word, command) in [
            (2, "ve", true),
            (3, "ve", false),
            (3, "ver", true),
            (4, "ver", false),
            (4, "vers", true),
            (1, "ve", true),
        ] {
            let (res, _) = run(&mut r, &at(min), word).await;
            assert_eq!(is_version(&res), command, "min {min}: {word}: {res:?}");
        }
        // Attribute names follow it too: `pro:` is `project:` unless the minimum is above 3.
        for (min, matches) in [(2, 1), (3, 1), (4, 0)] {
            let (res, _) = run(&mut r, &at(min), "pro:home count").await;
            assert_eq!(
                message(&res),
                if matches == 1 { "1" } else { "0" },
                "min {min}: {res:?}"
            );
        }
        let (res, _) = run(&mut r, &at(4), "proj:home count").await;
        assert_eq!(message(&res), "1");
    }
}

mod project_and_tag_lists {
    use super::*;

    /// The data the real `task` 3.5.0 was asked about: nested projects, finished and deleted tasks.
    async fn dataset() -> R {
        let mut r = replica();
        let cfg = Config::default();
        for line in [
            "add a project:home.travel +x +y",
            "add b project:home +x",
            "add c project:work.docs +z",
            "add d",
            "add e project:old +gone",
            "add f project:home.travel +done",
            "add g project:trash +trashed",
            "add w project:later wait:2099-01-01 +waiting",
        ] {
            run(&mut r, &cfg, line).await;
        }
        run(&mut r, &cfg, "description:e done").await;
        run(&mut r, &cfg, "description:f done").await;
        run_yes(&mut r, &cfg, "description:g delete").await;
        r
    }

    fn table(res: &CliResult) -> (Vec<(String, String)>, Vec<String>) {
        let CliResult::Table(t) = res else { panic!("{res:?}") };
        (
            t.rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect(),
            t.footer.clone(),
        )
    }
    fn rows(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
    }

    #[tokio::test]
    async fn projects_count_a_projects_children_and_indent_them_like_taskwarrior() {
        let mut r = dataset().await;
        let (res, _) = run(&mut r, &Config::default(), "projects").await;
        let (got, footer) = table(&res);
        assert_eq!(
            got,
            rows(&[
                ("(none)", "1"),
                ("home", "2"),
                ("  travel", "1"),
                ("later", "1"),
                ("work", "1"),
                ("  docs", "1")
            ])
        );
        assert_eq!(footer, ["5 projects (5 tasks)"]);
        // A filter narrows it, and the footer follows.
        let (res, _) = run(&mut r, &Config::default(), "project:home projects").await;
        let (got, footer) = table(&res);
        assert_eq!(got, rows(&[("home", "2"), ("  travel", "1")]));
        assert_eq!(footer, ["2 projects (2 tasks)"]);
    }

    #[tokio::test]
    async fn list_all_projects_adds_finished_tasks_but_never_deleted_ones() {
        let mut r = dataset().await;
        let cfg = parse("list.all.projects=1\n").config;
        let (res, _) = run(&mut r, &cfg, "projects").await;
        let (got, footer) = table(&res);
        assert_eq!(
            got,
            rows(&[
                ("(none)", "1"),
                ("home", "3"),
                ("  travel", "2"),
                ("later", "1"),
                ("old", "1"),
                ("work", "1"),
                ("  docs", "1")
            ])
        );
        assert_eq!(footer, ["6 projects (7 tasks)"]);
        // `rc.list.all.projects:1` on one command does the same.
        let (res, _) = run(&mut r, &Config::default(), "rc.list.all.projects:1 projects").await;
        assert_eq!(table(&res).1, ["6 projects (7 tasks)"]);
    }

    #[tokio::test]
    async fn tags_count_before_the_filter_and_list_all_tags_includes_deleted_tasks() {
        let mut r = dataset().await;
        let (res, _) = run(&mut r, &Config::default(), "tags").await;
        let (got, footer) = table(&res);
        assert_eq!(got, rows(&[("waiting", "1"), ("x", "2"), ("y", "1"), ("z", "1")]));
        assert_eq!(footer, ["4 tags", "(5 tasks)"]);

        let cfg = parse("list.all.tags=on\n").config;
        let (res, _) = run(&mut r, &cfg, "tags").await;
        let (got, footer) = table(&res);
        assert_eq!(
            got,
            rows(&[
                ("done", "1"),
                ("gone", "1"),
                ("trashed", "1"),
                ("waiting", "1"),
                ("x", "2"),
                ("y", "1"),
                ("z", "1")
            ])
        );
        assert_eq!(footer, ["7 tags", "(8 tasks)"], "the deleted task's tag counts here");
    }

    #[tokio::test]
    async fn the_completion_lists_follow_their_own_settings() {
        let mut r = dataset().await;
        let lines = |res: CliResult| match res {
            CliResult::Text { lines } => lines,
            other => panic!("{other:?}"),
        };
        let (res, _) = run(&mut r, &Config::default(), "_projects").await;
        assert_eq!(lines(res), ["home", "home.travel", "later", "work.docs"]);
        // Unlike `projects`, this one also names the projects of deleted tasks.
        let cfg = parse("list.all.projects=1\n").config;
        assert_eq!(
            lines(run(&mut r, &cfg, "_projects").await.0),
            ["home", "home.travel", "later", "old", "trash", "work.docs"]
        );

        let tags = lines(run(&mut r, &Config::default(), "_tags").await.0);
        for t in [
            "ACTIVE",
            "YESTERDAY",
            "next",
            "nocal",
            "nocolor",
            "nonag",
            "waiting",
            "x",
            "y",
            "z",
        ] {
            assert!(tags.contains(&t.to_owned()), "{t} in {tags:?}");
        }
        assert!(
            !tags.contains(&"gone".to_owned()),
            "finished tasks' tags only with complete.all.tags"
        );
        let cfg = parse("complete.all.tags=1\n").config;
        let tags = lines(run(&mut r, &cfg, "_tags").await.0);
        assert!(
            ["done", "gone", "trashed"]
                .iter()
                .all(|t| tags.contains(&(*t).to_owned())),
            "{tags:?}"
        );
        // Sorted as bytes, so the capitals come first (as in the real output).
        assert_eq!(tags[0], "ACTIVE");
    }
}

mod indicator_columns {
    use super::*;

    async fn report_with(cfg: &Config, columns: &str) -> tc_core::run::Output {
        let mut r = replica();
        let plain = Config::default();
        run(&mut r, &plain, "add alpha +t1").await;
        run(&mut r, &plain, "add beta +t1 +t2 depends:1").await;
        run(&mut r, &plain, "add gamma").await;
        run(&mut r, &plain, "add delta depends:3").await;
        run(&mut r, &plain, "description:gamma done").await;
        run(&mut r, &plain, "description:alpha start").await;
        let line = format!("rc.report.list.columns:{columns} rc.report.list.labels: list");
        let (res, _) = run(&mut r, cfg, &line).await;
        let CliResult::Report(o) = res else { panic!("{res:?}") };
        o
    }

    #[tokio::test]
    async fn an_unlabelled_column_gets_taskwarriors_label_for_its_style() {
        // The headers the real `task` 3.5.0 printed for these columns.
        let o = report_with(
            &Config::default(),
            "description,start,start.active,tags,tags.indicator,tags.count,depends,depends.indicator,depends.count",
        )
        .await;
        let labels: Vec<&str> = o.columns.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Description", "Started", "A", "Tags", "T", "Tag", "Depends", "D", "Dep"]
        );
    }

    #[tokio::test]
    async fn the_indicator_settings_shorten_the_label_to_their_own_length() {
        let cfg = parse("tag.indicator=@@\ndependency.indicator=DEP\nactive.indicator=>>\n").config;
        let o = report_with(&cfg, "description,start.active,tags.indicator,depends.indicator").await;
        let labels: Vec<&str> = o.columns.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Description", "A", "Ta", "Dep"]);
        // The real `task` with `dependency.indicator=DEP` printed `Dep`; with `@` as the tag mark, `T`. (A `#` in a
        // taskrc line starts a comment, so it cannot be an indicator there.)
        let cfg = parse("tag.indicator=@\n").config;
        let o = report_with(&cfg, "description,tags.indicator").await;
        assert_eq!(o.columns[1].label, "T");
    }

    #[tokio::test]
    async fn rows_say_how_many_dependencies_are_still_open() {
        let o = report_with(&Config::default(), "description,depends").await;
        let by = |d: &str| o.rows.iter().find(|r| r.facts.description == d).map(|r| r.pending_deps);
        assert_eq!(by("beta"), Some(1));
        assert_eq!(
            by("delta"),
            Some(0),
            "it depends on a finished task, which holds nothing up"
        );
        assert_eq!(by("alpha"), Some(0));
    }
}

mod typed_dates {
    use super::*;

    /// `due:<value>` on a task, under these settings: did it take?
    async fn takes(settings: &str, value: &str) -> bool {
        let mut r = replica();
        run(&mut r, &Config::default(), "add probe").await;
        let cfg = parse(settings).config;
        let (res, wrote) = run(&mut r, &cfg, &format!("1 modify due:{value}")).await;
        wrote && !message(&res).starts_with("ERROR") && load_facts(&mut r).await.unwrap()[0].due.is_some()
    }

    #[tokio::test]
    async fn date_iso_and_dateformat_decide_which_dates_are_understood_like_taskwarrior() {
        // The matrix below is what task 3.5.0 itself accepted (a date or a date and time, typed as `due:`).
        for (settings, input, ok) in [
            ("", "2026-12-25", true),
            ("", "12/25/2026", false),
            ("dateformat=m/d/Y\n", "2026-12-25", true),
            ("dateformat=m/d/Y\n", "12/25/2026", true),
            ("date.iso=0\n", "2026-12-25", true), // the default pattern is Y-M-D
            ("date.iso=0\n", "12/25/2026", false),
            ("date.iso=0\ndateformat=m/d/Y\n", "2026-12-25", false),
            ("date.iso=0\ndateformat=m/d/Y\n", "12/25/2026", true),
            ("date.iso=0\ndateformat=m/d/Y\n", "2026-12-25T10:00", true),
            ("date.iso=0\ndateformat=m/d/Y\n", "tomorrow", true),
        ] {
            assert_eq!(takes(settings, input).await, ok, "{settings:?} due:{input}");
        }
    }
}

mod the_calculator {
    use super::*;

    async fn calc(cfg: &Config, line: &str) -> String {
        let mut r = replica();
        message(&run(&mut r, cfg, line).await.0)
    }

    #[tokio::test]
    async fn calc_is_a_command_and_expressions_chooses_how_it_reads() {
        let cfg = Config::default();
        assert_eq!(calc(&cfg, "calc 1 + 2 * 3").await, "7");
        assert_eq!(calc(&cfg, "calc 2 days + 3 hours").await, "P2DT3H");
        assert_eq!(calc(&cfg, "calc (1 + 2) * 3").await, "9");
        // Postfix: set in the taskrc or for one command.
        let postfix = parse("expressions=postfix\n").config;
        assert_eq!(calc(&postfix, "calc 1 2 + 3 *").await, "9");
        assert_eq!(calc(&cfg, "rc.expressions:postfix calc 1 2 + 3 *").await, "9");
        // Anything but `postfix` is infix.
        assert_eq!(calc(&parse("expressions=infix\n").config, "calc 1 + 2").await, "3");
        // Mistakes say what is wrong, as errors.
        assert_eq!(calc(&cfg, "calc 5 / 0").await, "ERROR: Cannot divide by zero");
        assert_eq!(
            calc(&cfg, "calc (1 + 2").await,
            "ERROR: Mismatched parentheses in expression"
        );
    }

    #[tokio::test]
    async fn calc_knows_the_current_day_in_the_users_zone() {
        // Dates come out in the clock's zone; `today - today` is nothing, `tomorrow - today` a day.
        let cfg = Config::default();
        assert_eq!(calc(&cfg, "calc today - today").await, "PT0S");
        assert_eq!(calc(&cfg, "calc tomorrow - today").await, "P1D");
    }
}

mod the_calculators_references {
    use super::*;

    const UDAS: &str = "uda.est.type=numeric\nuda.when.type=date\nuda.len.type=duration\nuda.note.type=string\n";

    async fn world() -> (R, Config) {
        let cfg = parse(UDAS).config;
        let mut r = replica();
        for line in [
            "add Buy milk project:home.shop +x +y priority:H due:2026-12-25T10:00 est:3 when:2026-03-04T05:06:07 len:PT90M",
            "add other",
            "add later wait:2099-01-01",
            "add blocked depends:1",
        ] {
            run(&mut r, &cfg, line).await;
        }
        run(&mut r, &cfg, "1 annotate first note").await;
        run(&mut r, &cfg, "1 annotate second note").await;
        (r, cfg)
    }

    async fn calc(r: &mut R, cfg: &Config, e: &str) -> String {
        message(&run(r, cfg, &format!("calc {e}")).await.0)
    }

    // Every expected value is what `task calc` 3.5.0 printed for the same data.
    #[tokio::test]
    async fn a_task_by_id_gives_its_attributes_in_their_own_types() {
        let (mut r, cfg) = world().await;
        for (e, want) in [
            ("1.description", "Buy milk"),
            ("1.project", "home.shop"),
            ("1.priority", "H"),
            ("1.status", "pending"),
            ("1.due", "2026-12-25T10:00:00"),
            ("1.est", "3"),
            ("1.tags", "x,y"),
            ("1.id", "1"),
            ("1.end", ""),
            ("1.note", ""),
            ("1.depends", ""),
            ("1.mask", ""),
            ("1.parent", ""),
            ("1.imask", "0"),
            ("1.recur", "PT0S"),
            ("1.when", "2026-03-04T05:06:07"),
            ("1.len", "PT1H30M"),
            ("2.description", "other"),
            ("3.status", "waiting"),
            ("3.wait", "2099-01-01T00:00:00"),
        ] {
            assert_eq!(calc(&mut r, &cfg, e).await, want, "{e}");
        }
        // They are values, so they can be calculated with.
        for (e, want) in [
            ("1.due + 1d", "2026-12-26T10:00:00"),
            ("1.due - 2026-12-01", "P24DT10H"),
            ("1.est * 2", "6"),
            ("\"1.description == 'Buy milk'\"", "true"),
            ("1.len * 2", "PT3H"),
        ] {
            assert_eq!(calc(&mut r, &cfg, e).await, want, "{e}");
        }
        // The urgency is a number.
        assert!(calc(&mut r, &cfg, "1.urgency").await.parse::<f64>().unwrap() > 8.0);
    }

    #[tokio::test]
    async fn a_task_can_be_named_by_uuid_or_the_start_of_one() {
        let (mut r, cfg) = world().await;
        let uuid = calc(&mut r, &cfg, "1.uuid").await;
        assert_eq!(uuid.len(), 36);
        for name in [uuid.clone(), uuid[..8].to_owned(), uuid[..12].to_owned()] {
            assert_eq!(
                calc(&mut r, &cfg, &format!("{name}.project")).await,
                "home.shop",
                "{name}"
            );
        }
        // Seven characters are too few to be a uuid; the id-less word is just a word.
        let short = &uuid[..7];
        assert_eq!(
            calc(&mut r, &cfg, &format!("{short}.project")).await,
            format!("{short}.project")
        );
        assert_eq!(calc(&mut r, &cfg, "4.depends").await, uuid);
    }

    #[tokio::test]
    async fn what_is_not_a_reference_is_a_word_as_in_taskwarrior() {
        let (mut r, cfg) = world().await;
        for e in [
            "1.nope",
            "99.description",
            "due",
            "description",
            "rc.nope",
            "1.uuid.short",
            "1.est.year",
            "1.description.x",
            "1.annotations.5.description",
            "1.annotations.0.description",
            // Attribute names are exact: no abbreviations here.
            "1.desc",
            "1.proj",
            "1.pri",
            "1.dep",
            "1.ann",
            "1.sta",
        ] {
            assert_eq!(calc(&mut r, &cfg, e).await, e, "{e}");
        }
    }

    #[tokio::test]
    async fn parts_of_dates_tags_and_annotations() {
        let (mut r, cfg) = world().await;
        for (e, want) in [
            ("1.due.year", "2026"),
            ("1.due.month", "12"),
            ("1.due.day", "25"),
            ("1.due.hour", "10"),
            ("1.due.minute", "0"),
            ("1.due.second", "0"),
            ("1.due.weekday", "5"),
            ("1.due.julian", "359"),
            ("1.when.year", "2026"),
            ("1.when.hour", "5"),
            ("1.end.year", "1970"),
            ("1.tags.x", "x"),
            ("1.tags.nope", ""),
            ("1.annotations.count", "2"),
            ("1.annotations.1.description", "first note"),
            ("1.annotations.2.description", "second note"),
        ] {
            assert_eq!(calc(&mut r, &cfg, e).await, want, "{e}");
        }
        // The week number follows the first day of the week: 51 from Sunday, 52 from Monday.
        assert_eq!(calc(&mut r, &cfg, "1.due.week").await, "51");
        let monday = parse(&format!("{UDAS}weekstart=monday\n")).config;
        assert_eq!(calc(&mut r, &monday, "1.due.week").await, "52");
        // An annotation's time is a date, with the same parts.
        assert!(calc(&mut r, &cfg, "1.annotations.1.entry").await.starts_with("20"));
        assert!(
            calc(&mut r, &cfg, "1.annotations.1.entry.year")
                .await
                .parse::<i32>()
                .unwrap()
                >= 2026
        );
    }

    #[tokio::test]
    async fn settings_the_program_and_the_system() {
        let (mut r, cfg) = world().await;
        let tuned = parse(&format!("{UDAS}bulk=7\ndateformat=m/d/Y\n")).config;
        for (e, want) in [
            // Defaults when the taskrc is silent, as Taskwarrior has them.
            ("rc.bulk", "3"),
            ("rc.confirmation", "1"),
            ("rc.dateformat", "Y-M-D"),
            ("rc.dateformat.info", "Y-M-D H:N:S"),
            ("rc.weekstart", "sunday"),
            ("rc.abbreviation.minimum", "2"),
            ("rc.expressions", "infix"),
            // Everything the app holds can be asked, UDAs included.
            ("rc.uda.est.type", "numeric"),
            ("rc.uda.when.type", "date"),
            ("tw.version", "3.5.0"),
            ("system.version", "3.5.0"),
            ("tw.program", "task"),
            ("context.program", "task"),
            ("tw.width", "80"),
            ("tw.height", "24"),
            // Changes not yet synced: this replica never syncs (the real client says 1 here too).
            ("tw.syncneeded", "1"),
        ] {
            assert_eq!(calc(&mut r, &cfg, e).await, want, "{e}");
        }
        assert_eq!(calc(&mut r, &tuned, "rc.bulk").await, "7");
        assert_eq!(calc(&mut r, &tuned, "rc.dateformat").await, "m/d/Y");
        // Settings are text, so they combine like text; and a number can be built from one.
        assert_eq!(calc(&mut r, &tuned, "rc.bulk + 1").await, "71");
        // The line as typed.
        assert_eq!(calc(&mut r, &cfg, "tw.args").await, "task calc tw.args");
    }
}

mod changes_after_a_command_word {
    use super::*;

    async fn one(r: &mut R) -> tc_core::model::Facts {
        load_facts(r).await.unwrap().remove(0)
    }
    fn near(got: Option<i64>, ago_secs: i64) -> bool {
        let want = taskchampion::chrono::Utc::now().timestamp() - ago_secs;
        // The test clock runs a second ahead per command issued by any test, so allow for that;
        // the offsets checked here (two hours, a day) are far larger.
        got.is_some_and(|g| (g - want).abs() < 3000)
    }

    // Each case is what the real `task` 3.5.0 did with the same line.
    #[tokio::test]
    async fn done_takes_attributes_tags_and_words_and_an_end_time_given_wins() {
        let (mut r, cfg) = (replica(), Config::default());
        run(&mut r, &cfg, "add task a").await;
        let (res, wrote) = run(&mut r, &cfg, "1 done end:-2h priority:H +x some words").await;
        assert!(wrote, "{}", message(&res));
        let f = one(&mut r).await;
        assert_eq!(f.status, "completed");
        assert!(near(f.end, 2 * 3600), "end {:?}", f.end);
        assert_eq!(f.priority.as_deref(), Some("H"));
        assert!(f.tags.contains("x"));
        assert_eq!(
            f.annotations.iter().map(|a| a.text.as_str()).collect::<Vec<_>>(),
            ["some words"]
        );
    }

    #[tokio::test]
    async fn done_alone_still_ends_now_and_a_bad_value_changes_nothing() {
        let (mut r, cfg) = (replica(), Config::default());
        run(&mut r, &cfg, "add task a").await;
        let (res, wrote) = run(&mut r, &cfg, "1 done due:notadate").await;
        assert!(!wrote && message(&res).starts_with("ERROR"), "{}", message(&res));
        assert_eq!(one(&mut r).await.status, "pending");
        run(&mut r, &cfg, "1 done").await;
        assert!(near(one(&mut r).await.end, 0));
    }

    #[tokio::test]
    async fn start_and_stop_take_changes_too() {
        let (mut r, cfg) = (replica(), Config::default());
        run(&mut r, &cfg, "add task a").await;
        run(&mut r, &cfg, "1 start due:eow +y").await;
        let f = one(&mut r).await;
        assert!(f.start.is_some() && f.due.is_some() && f.tags.contains("y"));
        run(&mut r, &cfg, "1 stop project:P note words").await;
        let f = one(&mut r).await;
        assert!(f.start.is_none());
        assert_eq!(f.project.as_deref(), Some("P"));
        assert_eq!(
            f.annotations.iter().map(|a| a.text.as_str()).collect::<Vec<_>>(),
            ["note words"]
        );
    }

    #[tokio::test]
    async fn delete_takes_an_end_time_and_a_reason() {
        let (mut r, cfg) = (replica(), Config::default());
        run(&mut r, &cfg, "add task a").await;
        let (res, wrote) = run_yes(&mut r, &cfg, "1 delete end:-1d reason words").await;
        assert!(wrote, "{}", message(&res));
        let f = one(&mut r).await;
        assert_eq!(f.status, "deleted");
        assert!(near(f.end, 86_400), "end {:?}", f.end);
        assert_eq!(f.annotations[0].text, "reason words");
    }

    /// A typed line, like the console's.
    async fn typed(r: &mut R, cfg: &Config, line: &str) -> (CliResult, bool) {
        run_opts(
            r,
            cfg,
            line,
            Options {
                typed: true,
                ..Options::default()
            },
        )
        .await
    }

    #[tokio::test]
    async fn annotate_append_and_prepend_take_attributes_beside_their_text_when_typed() {
        let (mut r, cfg) = (replica(), Config::default());
        run(&mut r, &cfg, "add task a").await;
        typed(&mut r, &cfg, "1 annotate hello due:eow").await;
        let f = one(&mut r).await;
        assert!(f.due.is_some());
        assert_eq!(f.annotations[0].text, "hello");
        // Attributes alone are enough: no annotation is made out of nothing.
        typed(&mut r, &cfg, "1 annotate +z").await;
        let f = one(&mut r).await;
        assert!(f.tags.contains("z") && f.annotations.len() == 1);
        typed(&mut r, &cfg, "1 append more +w").await;
        typed(&mut r, &cfg, "1 prepend first").await;
        let f = one(&mut r).await;
        assert_eq!(f.description, "first task a more");
        assert!(f.tags.contains("w"));
        // With nothing at all there is nothing to do.
        for cmd in ["1 annotate", "1 append", "1 prepend"] {
            let (res, wrote) = typed(&mut r, &cfg, cmd).await;
            assert!(
                !wrote && message(&res).contains("needs some text"),
                "{cmd}: {}",
                message(&res)
            );
        }
    }

    #[tokio::test]
    async fn a_note_from_a_button_is_text_whatever_it_looks_like() {
        let (mut r, cfg) = (replica(), Config::default());
        run(&mut r, &cfg, "add task a").await;
        // The GUI sends the note as one argument; `due:tomorrow check` is a note, not a due date.
        run(&mut r, &cfg, "1 annotate due:tomorrow check").await;
        let f = one(&mut r).await;
        assert_eq!(f.due, None);
        assert_eq!(f.annotations[0].text, "due:tomorrow check");
    }
}

mod show_and_config {
    use super::*;
    use tc_core::cli::{Ask, Done};

    async fn done(r: &mut R, cfg: &Config, line: &str, confirmed: bool) -> Done {
        let mut undo = UndoStack::default();
        execute(
            r,
            cfg,
            clock(),
            &split_words(line),
            Options {
                confirmed,
                ..Options::default()
            },
            &mut undo,
        )
        .await
    }

    #[tokio::test]
    async fn show_lists_every_setting_and_highlights_what_was_changed() {
        let mut r = replica();
        let cfg = parse("bulk=7\ndefault.project=Home\n").config;
        let d = done(&mut r, &cfg, "show", false).await;
        assert!(!d.wrote && d.config.is_none());
        let CliResult::Table(t) = d.result else {
            panic!("{:?}", d.result)
        };
        let at = |n: &str| t.rows.iter().position(|row| row[0] == n).unwrap();
        assert_eq!(t.rows[at("bulk")][1], "7");
        assert!(t.highlight.contains(&at("bulk")) && !t.highlight.contains(&at("confirmation")));
        assert_eq!(t.rows[at("bulk") + 1], ["  Default value", "3"]);
        // Narrowed by a word.
        let CliResult::Table(t) = done(&mut r, &cfg, "show default", false).await.result else {
            panic!()
        };
        assert!(t
            .rows
            .iter()
            .all(|row| row[0].trim_start().starts_with("default") || row[0] == "  Default value"));
        // An `rc.` override is a setting for this command, not a word of it, and shows as changed.
        let CliResult::Table(t) = done(&mut r, &Config::default(), "rc.bulk:5 show bulk", false)
            .await
            .result
        else {
            panic!()
        };
        assert_eq!(t.rows[0], ["bulk", "5"]);
        assert_eq!(t.highlight, [0, 1]);
    }

    #[tokio::test]
    async fn config_asks_then_returns_the_new_settings_for_the_caller_to_save() {
        let mut r = replica();
        let cfg = parse("bulk=7\n").config;
        // `confirmation` is on by default: the first run only asks.
        let d = done(&mut r, &cfg, "config bulk 9", false).await;
        assert!(d.config.is_none());
        match &d.result {
            CliResult::Confirm {
                ask: Ask::Plain,
                message,
                ..
            } => {
                assert_eq!(
                    message,
                    "Are you sure you want to change the value of 'bulk' from '7' to '9'?"
                );
            }
            other => panic!("{other:?}"),
        }
        let d = done(&mut r, &cfg, "config bulk 9", true).await;
        assert_eq!(message(&d.result), "Config modified.");
        assert_eq!(d.config.expect("new settings").bulk(), 9);
        // Without the question when `confirmation` is off, and several words are one value.
        let quiet = parse("confirmation=off\nbulk=7\n").config;
        let d = done(&mut r, &quiet, "config default.command next +PENDING", false).await;
        let saved = d.config.expect("new settings");
        assert_eq!(
            saved.settings.get("default.command").map(String::as_str),
            Some("next +PENDING")
        );
        assert_eq!(saved.bulk(), 7, "the rest is kept");
        // Removing puts back the default; removing what isn't there is an error.
        let d = done(&mut r, &quiet, "config bulk", false).await;
        assert_eq!(d.config.expect("new settings").bulk(), 3);
        assert_eq!(
            message(&done(&mut r, &quiet, "config nothing.here", false).await.result),
            "ERROR: No entry named 'nothing.here' found."
        );
        assert_eq!(
            message(&done(&mut r, &quiet, "config", false).await.result),
            "ERROR: Specify the name of a config variable to modify."
        );
    }

    #[tokio::test]
    async fn config_edits_what_is_saved_not_what_one_command_overrides() {
        let mut r = replica();
        let quiet = parse("confirmation=off\nbulk=7\n").config;
        // `rc.limit:5` is for this command only; it must not be written into the saved settings.
        let d = done(&mut r, &quiet, "rc.limit:5 config weekstart monday", false).await;
        let saved = d.config.expect("new settings");
        assert_eq!(saved.settings.get("weekstart").map(String::as_str), Some("monday"));
        assert!(!saved.settings.contains_key("limit"));
    }

    #[tokio::test]
    async fn nothing_sensitive_can_be_set_shown_or_removed() {
        let mut r = replica();
        let quiet = parse("confirmation=off\n").config;
        for line in [
            "config sync.encryption_secret hunter2",
            "config sync.aws.access_key_id AKIAHUNTER2",
            "config taskd.password hunter2",
            "config my.api_token hunter2",
            "config sync.encryption_secret",
        ] {
            let d = done(&mut r, &quiet, line, false).await;
            let m = message(&d.result);
            assert!(
                m.starts_with("ERROR:") && !m.contains("hunter2") && !m.contains("AKIA"),
                "{line}: {m}"
            );
            assert!(d.config.is_none(), "{line} produced a new config");
        }
        // And `show` has nothing of the kind to show.
        for word in ["sync", "secret", "token", "password"] {
            let d = done(&mut r, &quiet, &format!("show {word}"), false).await;
            // Nothing sensitive is listed; harmless names containing the word (`color.sync.added`) may be.
            match &d.result {
                CliResult::Text { .. } => {}
                CliResult::Table(t) => assert!(
                    t.rows.iter().all(|r| !tc_core::taskrc::is_sensitive(&r[0])),
                    "show {word}: {:?}",
                    t.rows
                ),
                other => panic!("show {word}: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn a_setting_the_app_does_not_read_or_a_bad_value_is_not_stored() {
        let mut r = replica();
        let quiet = parse("confirmation=off\n").config;
        let d = done(&mut r, &quiet, "config verbose nothing", false).await;
        assert!(message(&d.result).contains("not a setting this app reads") && d.config.is_none());
        let d = done(&mut r, &quiet, "config weekstart someday", false).await;
        assert!(message(&d.result).starts_with("ERROR:") && d.config.is_none());
    }
}

mod hooks {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tc_core::cli::Done;
    use tc_core::hooks::{Hooked, Hooks, Kind, Reject};
    use tc_core::model::Facts;

    /// A hook set that does what a test asks, and records what it was shown.
    #[derive(Debug, Default)]
    struct Probe {
        tag_new: bool,
        refuse_new: bool,
        refuse_done: bool,
        refuse_launch: bool,
        rename_on_modify: bool,
        /// Tag a task that is an instance of a recurring one, and refuse it when `refuse_instances` is set.
        tag_instances: bool,
        refuse_instances: bool,
        seen: Mutex<Vec<String>>,
        /// Masks a parent was given (a parent whose mask changed reaches `on_modify`).
        masks: Mutex<Vec<String>>,
    }

    impl Hooks for Probe {
        fn on_launch(&self, h: &mut Hooked, command: &str) -> Result<(), Reject> {
            h.say(format!("launch: {command}"));
            if self.refuse_launch {
                return Err("not today".into());
            }
            Ok(())
        }
        fn on_add(&self, h: &mut Hooked, mut task: Facts) -> Result<Facts, Reject> {
            self.seen
                .lock()
                .unwrap()
                .push(format!("add {} [{}]", task.description, task.status));
            if task.parent.is_some() {
                if self.refuse_instances {
                    return Err("no instances".into());
                }
                if self.tag_instances {
                    task.tags.insert("instance".into());
                }
            }
            if self.refuse_new {
                h.warn("no new tasks");
                return Err("add refused".into());
            }
            if self.tag_new {
                task.tags.insert("hooked".into());
                task.project.get_or_insert_with(|| "Inbox".into());
                h.say("tagged");
            }
            Ok(task)
        }
        fn on_modify(&self, h: &mut Hooked, old: &Facts, mut new: Facts) -> Result<Facts, Reject> {
            self.seen
                .lock()
                .unwrap()
                .push(format!("modify {} {}->{}", old.description, old.status, new.status));
            if old.mask != new.mask {
                self.masks.lock().unwrap().push(new.mask.clone().unwrap_or_default());
            }
            if self.refuse_done && new.status == "completed" {
                h.warn("finish it later");
                return Err("done refused".into());
            }
            if self.rename_on_modify {
                new.description = format!("{}!", new.description);
            }
            Ok(new)
        }
        fn on_exit(&self, h: &mut Hooked, changed: &[Facts]) {
            h.say(format!("exit: {} changed", changed.len()));
        }
    }

    async fn go(r: &mut R, cfg: &Config, line: &str, hooks: &Arc<Probe>) -> Done {
        let mut undo = UNDO.with(|u| u.borrow().clone());
        let o = Options {
            hooks: Some(hooks.clone()),
            confirmed: true,
            ..Options::default()
        };
        let d = execute(r, cfg, clock(), &split_words(line), o, &mut undo).await;
        UNDO.with(|u| *u.borrow_mut() = undo);
        d
    }

    fn texts(d: &Done) -> Vec<String> {
        d.feedback.iter().map(|l| l.text.clone()).collect()
    }

    #[tokio::test]
    async fn a_hook_can_change_a_new_task_and_say_something_and_one_undo_takes_it_all_back() {
        let (mut r, cfg) = (replica(), Config::default());
        let p = Arc::new(Probe {
            tag_new: true,
            ..Probe::default()
        });
        let d = go(&mut r, &cfg, "add Buy milk", &p).await;
        assert_eq!(texts(&d), ["launch: add Buy milk", "tagged", "exit: 1 changed"]);
        let all = load_facts(&mut r).await.unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].tags.contains("hooked"));
        assert_eq!(all[0].project.as_deref(), Some("Inbox"));
        assert_eq!(p.seen.lock().unwrap()[0], "add Buy milk [pending]");
        // The hook's edit and the add are one undo step.
        assert!(matches!(run_yes(&mut r, &cfg, "undo").await.0, CliResult::Text { .. }));
        let all = load_facts(&mut r).await.unwrap();
        assert!(all.iter().all(|f| f.status == "deleted"), "{all:?}");
    }

    #[tokio::test]
    async fn a_hook_can_refuse_an_add() {
        let (mut r, cfg) = (replica(), Config::default());
        let p = Arc::new(Probe {
            refuse_new: true,
            ..Probe::default()
        });
        let d = go(&mut r, &cfg, "add Nope", &p).await;
        assert_eq!(message(&d.result), "ERROR: add refused");
        assert!(!d.wrote);
        assert!(descs(&mut r).await.is_empty());
        assert!(d
            .feedback
            .iter()
            .any(|l| l.kind == Kind::Warn && l.text == "no new tasks"));
        // A refused add changed nothing, so the exit hook has nothing to report.
        assert_eq!(d.feedback.last().map(|l| l.text.as_str()), Some("exit: 0 changed"));
    }

    #[tokio::test]
    async fn on_modify_sees_before_and_after_and_can_refuse_or_change() {
        let (mut r, cfg) = (replica(), Config::default());
        let quiet = Arc::new(Probe::default());
        go(&mut r, &cfg, "add Write report", &quiet).await;

        let refuse = Arc::new(Probe {
            refuse_done: true,
            ..Probe::default()
        });
        let d = go(&mut r, &cfg, "1 done", &refuse).await;
        assert_eq!(message(&d.result), "ERROR: done refused");
        assert_eq!(refuse.seen.lock().unwrap()[0], "modify Write report pending->completed");
        assert_eq!(
            load_facts(&mut r).await.unwrap()[0].status,
            "pending",
            "a refused change is not saved"
        );

        let rename = Arc::new(Probe {
            rename_on_modify: true,
            ..Probe::default()
        });
        let d = go(&mut r, &cfg, "1 modify project:Work", &rename).await;
        assert!(d.wrote);
        let f = &load_facts(&mut r).await.unwrap()[0];
        assert_eq!(
            (f.description.as_str(), f.project.as_deref()),
            ("Write report!", Some("Work"))
        );
    }

    #[tokio::test]
    async fn on_launch_can_stop_a_command_before_it_does_anything() {
        let (mut r, cfg) = (replica(), Config::default());
        let p = Arc::new(Probe {
            refuse_launch: true,
            ..Probe::default()
        });
        let d = go(&mut r, &cfg, "add Never", &p).await;
        assert_eq!(message(&d.result), "ERROR: not today");
        assert!(descs(&mut r).await.is_empty());
        assert_eq!(texts(&d), ["launch: add Never"], "nothing else ran");
    }

    #[tokio::test]
    async fn hooks_off_runs_none_and_a_plain_run_prints_nothing() {
        let (mut r, cfg) = (replica(), Config::default());
        let p = Arc::new(Probe {
            tag_new: true,
            refuse_launch: true,
            ..Probe::default()
        });
        for line in ["rc.hooks:off add Free", "rc.hooks:0 add Also free"] {
            let d = go(&mut r, &cfg, line, &p).await;
            assert!(d.wrote && d.feedback.is_empty(), "{line}: {:?}", d.feedback);
        }
        let off = parse("hooks=off\n").config;
        assert!(go(&mut r, &off, "add Third", &p).await.feedback.is_empty());
        assert!(load_facts(&mut r).await.unwrap().iter().all(|f| f.tags.is_empty()));
        // Without any hooks supplied, the shipped placeholders do nothing.
        let mut undo = UndoStack::default();
        let d = execute(
            &mut r,
            &cfg,
            clock(),
            &split_words("add Plain"),
            Options::default(),
            &mut undo,
        )
        .await;
        assert!(d.wrote && d.feedback.is_empty());
    }

    #[tokio::test]
    async fn a_generated_recurring_instance_runs_on_add_and_the_parents_new_mask_runs_on_modify() {
        // Taskwarrior fires both from its database layer, so a generated instance is a new task like any other.
        let (mut r, cfg) = (replica(), Config::default());
        let p = Arc::new(Probe {
            tag_instances: true,
            ..Probe::default()
        });
        go(&mut r, &cfg, "add Water plants recur:daily due:tomorrow", &p).await;
        let d = go(&mut r, &cfg, "next", &p).await; // the housekeeping before this command makes the instance
        let all = load_facts(&mut r).await.unwrap();
        let instances: Vec<_> = all.iter().filter(|f| f.parent.is_some()).collect();
        assert_eq!(instances.len(), 1, "{all:?}");
        assert!(
            instances[0].tags.contains("instance"),
            "the hook's edit is saved: {:?}",
            instances[0]
        );
        assert_eq!(
            p.masks.lock().unwrap().as_slice(),
            ["-"],
            "the parent's mask change reached on_modify"
        );
        assert!(d.wrote);
        // on_exit is told about everything written: the new instance and the parent whose mask changed.
        assert!(texts(&d).iter().any(|t| t == "exit: 2 changed"), "{:?}", texts(&d));
    }

    #[tokio::test]
    async fn a_hook_refusing_a_generated_instance_ends_the_command_and_writes_nothing() {
        let (mut r, cfg) = (replica(), Config::default());
        let quiet = Arc::new(Probe::default());
        go(&mut r, &cfg, "add Water plants recur:daily due:tomorrow", &quiet).await;
        let p = Arc::new(Probe {
            refuse_instances: true,
            ..Probe::default()
        });
        // As in Taskwarrior the whole command stops, with the hook's message.
        let d = go(&mut r, &cfg, "add Other", &p).await;
        assert_eq!(message(&d.result), "ERROR: no instances");
        assert!(!d.wrote);
        let all = load_facts(&mut r).await.unwrap();
        assert!(
            all.iter().all(|f| f.parent.is_none() && f.description != "Other"),
            "{all:?}"
        );
        assert_eq!(all[0].mask, None, "the parent's mask was not touched either");
        // `hooks=off` is the way out, and then the instance is made.
        let d = go(&mut r, &cfg, "rc.hooks:off add Free", &p).await;
        assert!(d.wrote && d.feedback.is_empty());
        let all = load_facts(&mut r).await.unwrap();
        assert!(all.iter().any(|f| f.parent.is_some()) && all.iter().any(|f| f.description == "Free"));
    }
}

mod export_command {
    use super::*;

    fn file(res: &CliResult) -> (&str, &str, usize) {
        let CliResult::File { name, text, count, .. } = res else {
            panic!("not a file: {res:?}")
        };
        (name, text, *count)
    }

    #[tokio::test]
    async fn export_is_taskwarriors_json_in_a_file_with_the_ids_and_urgency_added() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Pay rent/bills project:Home +b due:2026-12-25").await;
        run(&mut r, &cfg, "add Second").await;
        run(&mut r, &cfg, "2 done").await;
        let (res, wrote) = run(&mut r, &cfg, "export").await;
        assert!(!wrote);
        let (name, text, count) = file(&res);
        assert!(name.starts_with("tasks-") && name.ends_with(".json"), "{name}");
        assert_eq!(count, 2);
        let v: serde_json::Value = serde_json::from_str(text).expect("a JSON array");
        let tasks = v.as_array().unwrap();
        // Finished tasks (no id) come first, as in Taskwarrior.
        assert_eq!(tasks[0]["description"], "Second");
        assert_eq!(tasks[0]["id"], 0);
        assert_eq!(tasks[1]["id"], 1);
        assert_eq!(tasks[1]["project"], "Home");
        assert_eq!(
            tasks[1]["uuid"].as_str().map(str::len),
            Some(36),
            "the uuid is in it: {text}"
        );
        assert_eq!(tasks[1]["tags"][0], "b");
        assert_eq!(tasks[1]["due"], "20261225T000000Z");
        assert!(tasks[1]["urgency"].is_number());
        // One task per line, `/` escaped, keys in Taskwarrior's order.
        assert!(text.starts_with("[\n{\"id\":0,\"description\":\"Second\""), "{text}");
        assert!(text.contains("Pay rent\\/bills"), "{text}");
        assert!(text.ends_with("}\n]\n"), "{text}");
    }

    #[tokio::test]
    async fn json_array_off_prints_one_task_per_line_and_nothing_selected_prints_an_empty_array() {
        let mut r = replica();
        let off = parse("json.array=off\n").config;
        run(&mut r, &off, "add A").await;
        run(&mut r, &off, "add B").await;
        let (res, _) = run(&mut r, &off, "export").await;
        let (_, text, _) = file(&res);
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().all(|l| l.starts_with('{') && l.ends_with('}')), "{text}");
        let (res, _) = run(&mut r, &off, "description:nothing export").await;
        assert_eq!(file(&res).1, "");
        let (res, _) = run(&mut r, &Config::default(), "description:nothing export").await;
        assert_eq!(file(&res).1, "[\n]\n");
        // `rc.json.array:off` is the same for one command.
        let (res, _) = run(&mut r, &Config::default(), "rc.json.array:off export").await;
        assert_eq!(file(&res).1.lines().count(), 2);
    }

    #[tokio::test]
    async fn export_takes_a_filter_and_a_report_name() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Keep project:Home").await;
        run(&mut r, &cfg, "add Other project:Work").await;
        run(&mut r, &cfg, "add Gone project:Home").await;
        run(&mut r, &cfg, "3 done").await;
        let (res, _) = run(&mut r, &cfg, "project:Home export").await;
        assert_eq!(file(&res).2, 2);
        // The report's own filter applies: `next` leaves out what is finished.
        let (res, _) = run(&mut r, &cfg, "export next").await;
        let (_, text, count) = file(&res);
        assert_eq!(count, 2, "{text}");
        assert!(!text.contains("Gone"));
        let (res, _) = run(&mut r, &cfg, "project:Home export next").await;
        assert_eq!(file(&res).2, 1);
    }

    #[tokio::test]
    async fn information_is_info_and_so_is_any_abbreviation_of_either() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Look at me").await;
        for line in ["1 info", "1 information", "1 infor", "info 1"] {
            let (res, _) = run(&mut r, &cfg, line).await;
            assert!(matches!(res, CliResult::Info { .. }), "{line}: {res:?}");
        }
    }

    #[tokio::test]
    async fn show_lists_json_array_among_the_settings() {
        let mut r = replica();
        let (res, _) = run(&mut r, &Config::default(), "show json").await;
        let CliResult::Table(t) = res else { panic!("{res:?}") };
        assert_eq!(t.rows[0], ["json.array", "1"]);
    }
}

mod activity_reports {
    use super::*;

    fn table(res: CliResult) -> tc_core::cli::TableOut {
        let CliResult::Table(t) = res else {
            panic!("not a table: {res:?}")
        };
        t
    }

    #[tokio::test]
    async fn history_counts_what_was_added_and_finished_this_month() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add One").await;
        run(&mut r, &cfg, "add Two").await;
        run(&mut r, &cfg, "add Three").await;
        run(&mut r, &cfg, "description:One done").await;
        run_yes(&mut r, &cfg, "description:Two delete").await;
        for cmd in ["history.monthly", "history.annual", "history.daily", "history.weekly"] {
            let t = table(run(&mut r, &cfg, cmd).await.0);
            // The last three columns before Net are Added, Completed, Deleted; one period holds it all.
            let n = t.headers.len();
            assert_eq!(&t.headers[n - 4..], ["Added", "Completed", "Deleted", "Net"], "{cmd}");
            assert_eq!(&t.rows[0][n - 4..], ["3", "1", "1", "1"], "{cmd}: {:?}", t.rows[0]);
            assert_eq!(t.rows.last().unwrap()[n - 5], "Average", "{cmd}");
            assert!(t.right.contains(&(n - 1)), "numbers on the right");
        }
        // With colour on the graph is coloured bars with their counts, and a legend in the same colours.
        let (res, _) = run(&mut r, &cfg, "description:One ghistory.monthly").await;
        let CliResult::Styled { lines, .. } = res else {
            panic!("{res:?}")
        };
        assert!(
            lines[1].iter().any(|s| s.style.is_some() && s.text.trim() == "1"),
            "{lines:?}"
        );
        assert_eq!(
            lines
                .last()
                .unwrap()
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>(),
            "Legend: Added, Completed, Deleted"
        );
        // With colour off it is made of `+`, `X` and `-`, and the legend says what they mean.
        let off = parse("color=off\n").config;
        let (res, _) = run(&mut r, &off, "description:One ghistory.monthly").await;
        let CliResult::Text { lines } = res else {
            panic!("{res:?}")
        };
        assert!(lines[0].starts_with("Year Month") && lines[0].ends_with("Number Added/Completed/Deleted"));
        assert!(lines[1].contains('+') && lines[1].contains('X'), "{lines:?}");
        assert_eq!(lines.last().unwrap(), "Legend: + Added, X Completed, - Deleted");
        // Nothing selected is said in words.
        let (res, _) = run(&mut r, &cfg, "description:nothing history.monthly").await;
        assert_eq!(message(&res), "No tasks.");
    }

    #[tokio::test]
    async fn a_recurring_template_is_not_counted_as_added() {
        let mut r = replica();
        let cfg = parse("recurrence=off\n").config;
        run(&mut r, &cfg, "add Water recur:weekly due:2026-12-25").await;
        run(&mut r, &cfg, "add Plain").await;
        let t = table(run(&mut r, &cfg, "history.annual").await.0);
        assert_eq!(t.rows[0][1], "1", "{:?}", t.rows[0]);
    }

    #[tokio::test]
    async fn the_timesheet_shows_the_last_four_weeks_unless_given_a_filter() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Finished project:Home").await;
        run(&mut r, &cfg, "add Working").await;
        run(&mut r, &cfg, "add Idle").await;
        run(&mut r, &cfg, "description:Finished done").await;
        run(&mut r, &cfg, "description:Working start").await;
        let t = table(run(&mut r, &cfg, "timesheet").await.0);
        assert_eq!(
            t.headers,
            ["Wk", "Date", "Day", "ID", "Action", "Project", "Due", "Task"]
        );
        let actions: Vec<&str> = t.rows.iter().map(|r| r[4].as_str()).collect();
        assert_eq!(actions.iter().filter(|a| **a == "Completed").count(), 1, "{actions:?}");
        assert_eq!(actions.iter().filter(|a| **a == "Started").count(), 1, "{actions:?}");
        assert!(
            t.rows.iter().all(|r| r[7] != "Idle"),
            "a task neither started nor finished is left out: {:?}",
            t.rows
        );
        assert_eq!(t.footer, ["1 completed, 1 started."]);
        // Given a filter, only that filter applies, so the idle task comes in (listed under 1970, as there).
        let t = table(run(&mut r, &cfg, "project:none timesheet").await.0);
        assert!(t.rows.is_empty());
        let t = table(run(&mut r, &cfg, "description:Idle timesheet").await.0);
        assert_eq!(t.rows[0][1], "1970-01-01");
    }

    #[tokio::test]
    async fn report_timesheet_filter_and_context_are_settings() {
        let mut r = replica();
        let cfg = parse("report.timesheet.filter=status:pending\n").config;
        run(&mut r, &cfg, "add Idle").await;
        let t = table(run(&mut r, &cfg, "timesheet").await.0);
        assert_eq!(t.rows.len(), 1, "{:?}", t.rows);
        // `rc.` does the same for one command, and the setting shows in `show`.
        let t = table(
            run(
                &mut r,
                &Config::default(),
                "rc.report.timesheet.filter:status:pending timesheet",
            )
            .await
            .0,
        );
        assert_eq!(t.rows.len(), 1);
        let (res, _) = run(&mut r, &cfg, "show report.timesheet").await;
        let shown = table(res);
        let filter = shown
            .rows
            .iter()
            .position(|r| r[0] == "report.timesheet.filter")
            .expect("listed");
        assert_eq!(shown.rows[filter], ["report.timesheet.filter", "status:pending"]);
        assert!(
            shown.highlight.contains(&filter),
            "changed from the default, so highlighted"
        );
        let context = shown
            .rows
            .iter()
            .find(|r| r[0] == "report.timesheet.context")
            .expect("listed");
        assert_eq!(context[1], "0");
        // The active context applies only if `report.timesheet.context` says so.
        let ctx =
            parse("context.work.read=project:Work\ncontext=work\nreport.timesheet.filter=status:pending\n").config;
        let t = table(run(&mut r, &ctx, "timesheet").await.0);
        assert_eq!(t.rows.len(), 1, "the context is not applied");
        let ctx = parse(
            "context.work.read=project:Work\ncontext=work\nreport.timesheet.filter=status:pending\nreport.timesheet.context=1\n",
        )
        .config;
        let t = table(run(&mut r, &ctx, "timesheet").await.0);
        assert!(t.rows.is_empty(), "now it is");
    }
}

mod report_defaults {
    use super::*;
    use std::collections::BTreeMap;

    /// `task show report.` on a fresh Taskwarrior 3.5.0, name to value.
    fn real() -> BTreeMap<String, String> {
        serde_json::from_str(include_str!("data/report_defaults_real.json")).unwrap()
    }

    #[tokio::test]
    async fn every_built_in_report_has_taskwarriors_own_defaults() {
        let mut r = replica();
        let (res, _) = run(&mut r, &Config::default(), "show report.").await;
        let CliResult::Table(t) = res else { panic!("{res:?}") };
        let ours: BTreeMap<String, String> = t
            .rows
            .iter()
            .filter(|row| !row[0].starts_with(' '))
            .map(|row| (row[0].clone(), row[1].clone()))
            .collect();
        let real = real();
        let mut diffs = Vec::new();
        for (k, v) in &real {
            match ours.get(k) {
                None => diffs.push(format!("missing  {k} = {v}")),
                Some(o) if o != v => diffs.push(format!("differs  {k}\n    real: {v}\n    ours: {o}")),
                _ => {}
            }
        }
        for k in ours.keys().filter(|k| !real.contains_key(*k)) {
            diffs.push(format!("extra    {k} = {}", ours[k]));
        }
        assert!(
            diffs.is_empty(),
            "{} differences from the real defaults:\n{}",
            diffs.len(),
            diffs.join("\n")
        );
    }

    #[tokio::test]
    async fn every_built_in_report_runs_with_its_default_columns() {
        let mut r = replica();
        let cfg = Config::default();
        run(
            &mut r,
            &cfg,
            "add Plain project:Home +a priority:H due:2026-01-01 until:2030-01-01",
        )
        .await;
        run(&mut r, &cfg, "add Waits wait:2030-01-01 scheduled:2029-01-01").await;
        run(&mut r, &cfg, "add Blocked depends:1").await;
        run(&mut r, &cfg, "add Water recur:weekly due:2026-12-25").await;
        run(&mut r, &cfg, "add Done project:Home").await;
        run(&mut r, &cfg, "description:Done done").await;
        run(&mut r, &cfg, "description:Plain annotate a note").await;
        run(&mut r, &cfg, "description:Plain start").await;
        for name in tc_core::report::BUILTIN_NAMES {
            let (res, _) = run(&mut r, &cfg, name).await;
            assert!(matches!(res, CliResult::Report(_)), "{name}: {res:?}");
        }
    }

    #[tokio::test]
    async fn a_sort_key_ending_in_a_slash_breaks_the_table_when_that_columns_value_changes() {
        // The two reports and their output are what the real `task` printed for the same tasks.
        let cfg = parse(
            "uda.outcome.type=string\nuda.outcome.label=Outcome\nuda.outcome.values=pass,fail,skip\n\
             uda.size.type=numeric\n\
             report.g.columns=id,outcome,size,project,description\nreport.g.labels=ID,Outcome,Size,Proj,Desc\n\
             report.g.sort=outcome+/,project-/\nreport.g.filter=status:pending\n\
             report.n.columns=id,outcome,size,description\nreport.n.labels=ID,Outcome,Size,Desc\n\
             report.n.sort=size-/,description+\nreport.n.filter=status:pending\n",
        )
        .config;
        let mut r = replica();
        for t in [
            "a outcome:pass project:X size:1",
            "b outcome:fail project:Y size:2",
            "c project:Y size:2",
            "d outcome:pass project:Y size:1",
            "e outcome:fail project:X",
            "f",
            "g outcome:pass project:X size:3",
        ] {
            run(&mut r, &cfg, &format!("add {t}")).await;
        }
        let shown = |res: CliResult| {
            let CliResult::Report(o) = res else { panic!("{res:?}") };
            (
                o.rows
                    .iter()
                    .map(|r| r.facts.description.clone())
                    .collect::<Vec<_>>()
                    .join(""),
                o.breaks,
            )
        };
        // Both columns break: a gap whenever the outcome or the project changes.
        let (order, breaks) = shown(run(&mut r, &cfg, "g").await.0);
        assert_eq!(order, "bedagcf");
        assert_eq!(breaks, [false, true, true, true, false, true, true]);
        // A numeric column breaks too, and a task without a value comes last.
        let (order, breaks) = shown(run(&mut r, &cfg, "n").await.0);
        assert_eq!(order, "gbcadef");
        assert_eq!(breaks, [false, true, false, true, false, true, false]);
    }

    // ---- colour

    fn rows(res: CliResult) -> Vec<tc_core::run::Row> {
        let CliResult::Report(o) = res else { panic!("{res:?}") };
        o.rows
    }

    fn style_of(rows: &[tc_core::run::Row], description: &str) -> Option<tc_core::color::Resolved> {
        rows.iter()
            .find(|r| r.facts.description == description)
            .unwrap()
            .style
            .map(|s| s.resolved())
    }

    #[tokio::test]
    async fn rows_come_back_coloured_by_the_apps_default_theme() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add plain").await;
        run(&mut r, &cfg, "add working").await;
        run(&mut r, &cfg, "description:working start").await;
        run(&mut r, &cfg, "add late due:2020-01-01").await;
        run(&mut r, &cfg, "add hidden due:2020-01-01 +nocolor").await;
        let list = rows(run(&mut r, &cfg, "all").await.0);
        assert_eq!(style_of(&list, "plain"), None, "no rule applies");
        // active: bold on sage (basic green background)
        let a = style_of(&list, "working").expect("active is coloured");
        assert!(a.bold && a.fg.is_none() && a.bg == Some(2), "{a:?}");
        // overdue: bold coral (red)
        let o = style_of(&list, "late").expect("overdue is coloured");
        assert!(o.bold && o.fg == Some(1), "{o:?}");
        // `nocolor` beats everything
        assert_eq!(style_of(&list, "hidden"), None);
    }

    #[tokio::test]
    async fn the_default_theme_does_not_blend_so_the_first_rule_that_applies_decides() {
        let mut r = replica();
        let cfg = Config::default();
        // Active and overdue at once: active comes first in the precedence order.
        run(&mut r, &cfg, "add both due:2020-01-01").await;
        run(&mut r, &cfg, "description:both start").await;
        let list = rows(run(&mut r, &cfg, "all").await.0);
        let s = style_of(&list, "both").expect("coloured");
        assert!(
            s.bold && s.fg.is_none() && s.bg == Some(2),
            "just the active rule: {s:?}"
        );
        // Taskwarrior's own way is a setting away: the two rules lay one on the other.
        let blend = parse("rule.color.merge=yes\n").config;
        let list = rows(run(&mut r, &blend, "all").await.0);
        let s = style_of(&list, "both").expect("coloured");
        assert!(s.bold && s.fg == Some(1) && s.bg == Some(2), "both rules: {s:?}");
    }

    #[tokio::test]
    async fn due_soon_is_plain_white_and_only_due_today_stands_out_and_priority_colours_no_row() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add soon due:3d priority:H").await;
        run(&mut r, &cfg, "add today due:eod").await;
        run(&mut r, &cfg, "add urgent priority:H").await;
        run(&mut r, &cfg, "add repeats due:40d recur:monthly priority:M").await;
        let list = rows(run(&mut r, &cfg, "all").await.0);
        // Due within the week: white (basic colour 7), not bold, whatever its priority is.
        let s = style_of(&list, "soon").expect("due soon is coloured");
        assert_eq!((s.fg, s.bold), (Some(7), false), "{s:?}");
        // Due today: the one that stands out.
        let t = style_of(&list, "today").expect("due today is coloured");
        assert_eq!((t.fg, t.bold), (Some(3), true), "{t:?}");
        // A priority no longer colours the row (the page colours the Priority cell, from the same setting).
        assert_eq!(style_of(&list, "urgent"), None);
        // Recurring still has its own colour when nothing about its date claims the row.
        assert_eq!(style_of(&list, "repeats").map(|s| s.fg), Some(Some(5)));
        // And the setting the page reads for that cell is still there.
        let colours = tc_core::color::palette_for_page(&cfg);
        for (v, fg) in [("H", 1), ("M", 3), ("L", 2)] {
            assert_eq!(colours[&format!("uda.priority.{v}")].fg, Some(fg), "{v}");
        }
    }

    #[tokio::test]
    async fn the_taskrc_overrides_the_theme_and_color_off_turns_it_all_off() {
        let mut r = replica();
        let cfg = parse("color.active=bold red\ncolor.overdue=\n").config;
        run(&mut r, &cfg, "add working").await;
        run(&mut r, &cfg, "description:working start").await;
        run(&mut r, &cfg, "add late due:2020-01-01").await;
        let list = rows(run(&mut r, &cfg, "all").await.0);
        assert_eq!(
            style_of(&list, "working").unwrap().fg,
            Some(1),
            "a rule the taskrc changed"
        );
        assert_eq!(
            style_of(&list, "late"),
            None,
            "an empty value switches a default rule off"
        );
        let off = parse("color=off\n").config;
        let list = rows(run(&mut r, &off, "all").await.0);
        assert!(list.iter().all(|r| r.style.is_none()));
        // `rc.color:off` is the same for one command.
        let list = rows(run(&mut r, &Config::default(), "rc.color:off all").await.0);
        assert!(list.iter().all(|r| r.style.is_none()));
    }

    #[tokio::test]
    async fn every_other_row_of_a_report_is_shaded_by_color_alternate() {
        let mut r = replica();
        let cfg = parse("color.alternate=on gray2\ncolor.tag.x=bold red\n").config;
        for t in ["a", "b", "c +x", "d", "e +x"] {
            run(&mut r, &cfg, &format!("add {t}")).await;
        }
        let list = rows(run(&mut r, &cfg, "oldest").await.0);
        let bgs: Vec<Option<u8>> = list.iter().map(|r| r.style.and_then(|s| s.resolved().bg)).collect();
        // Rows 2 and 4 (index 1 and 3) are shaded; the rule on row 5 adds to nothing (it is not odd).
        assert_eq!(bgs, [None, Some(234), None, Some(234), None], "{bgs:?}");
        // A rule on a shaded row is laid over the shading.
        let c = list
            .iter()
            .find(|r| r.facts.description == "c")
            .unwrap()
            .style
            .unwrap()
            .resolved();
        assert!(c.bold && c.fg == Some(1) && c.bg.is_none(), "{c:?}");
        let shaded_and_ruled = rows(run(&mut r, &cfg, "newest").await.0);
        let e = shaded_and_ruled
            .iter()
            .find(|r| r.facts.description == "e")
            .unwrap()
            .style
            .unwrap()
            .resolved();
        assert!(e.bold && e.fg == Some(1), "{e:?}");
    }

    #[tokio::test]
    async fn a_bad_colour_is_refused_with_taskwarriors_words() {
        let p = parse("color.active=bold purple\ncolor.overdue=red\n");
        assert_eq!(p.warnings, ["color.active: The color 'purple' is not recognized."]);
        assert!(!p.config.settings.contains_key("color.active"));
        assert!(p.config.settings.contains_key("color.overdue"));
    }

    #[tokio::test]
    async fn show_lists_the_colour_settings_and_marks_the_ones_you_changed() {
        let mut r = replica();
        let cfg = parse("color.active=bold red\n").config;
        let (res, _) = run(&mut r, &cfg, "show color.act").await;
        let CliResult::Table(t) = res else { panic!("{res:?}") };
        assert_eq!(t.rows[0], ["color.active", "bold red"]);
        assert!(t.highlight.contains(&0));
        assert_eq!(t.rows[1], ["  Default value", "bold on sage"]);
    }

    #[tokio::test]
    async fn the_colors_command_shows_the_palette_a_sample_and_the_legend() {
        let mut r = replica();
        let cfg = parse("color.active=bold red\n").config;
        let text = |res: CliResult| {
            let CliResult::Styled { lines, .. } = res else {
                panic!("{res:?}")
            };
            lines
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let palette = text(run(&mut r, &cfg, "colors").await.0);
        assert!(palette.contains("Basic colors") && palette.contains("Gray ramp gray0 - gray23"));
        // `color` is enough, as `task color` is in Taskwarrior.
        assert_eq!(text(run(&mut r, &cfg, "color").await.0), palette);
        let sample = text(run(&mut r, &cfg, "colors coral on bright sky").await.0);
        assert!(
            sample.contains("Your sample:\n\n  task color coral on bright sky"),
            "{sample}"
        );
        assert_eq!(
            message(&run(&mut r, &cfg, "colors purple").await.0),
            "ERROR: The color 'purple' is not recognized."
        );
        let legend = text(run(&mut r, &cfg, "colors legend").await.0);
        assert!(
            legend.contains("color.active") && legend.contains("bold red"),
            "{legend}"
        );
        // The names are Taskwarrior's, the colours the ones in force (the theme's, with the taskrc's on top).
        assert!(
            legend.contains("color.overdue") && legend.contains("bold coral"),
            "{legend}"
        );
        let off = parse("color=off\n").config;
        assert!(message(&run(&mut r, &off, "colors").await.0).starts_with("Color is currently turned off"));
    }
}

mod duplicate_log_stats_context {
    use super::*;
    use tc_core::cli::Done;
    use tc_core::model::Facts;

    async fn done(r: &mut R, cfg: &Config, line: &str, confirmed: bool) -> Done {
        let mut undo = UndoStack::default();
        let o = Options {
            confirmed,
            typed: true,
            ..Options::default()
        };
        execute(r, cfg, clock(), &split_words(line), o, &mut undo).await
    }

    async fn find(r: &mut R, description: &str) -> Vec<Facts> {
        let mut v = load_facts(r).await.unwrap();
        v.retain(|f| f.description == description);
        v
    }

    fn quiet() -> Config {
        parse("confirmation=off\n").config
    }

    #[tokio::test]
    async fn duplicate_copies_a_task_but_not_its_identity_start_end_or_entry() {
        let mut r = replica();
        let cfg = Config::default();
        run(
            &mut r,
            &cfg,
            "add Buy milk project:Home +errand due:tomorrow priority:H",
        )
        .await;
        run(&mut r, &cfg, "1 annotate a note").await;
        run(&mut r, &cfg, "1 start").await;
        let (res, wrote) = run(&mut r, &cfg, "1 duplicate").await;
        assert!(wrote);
        assert_eq!(message(&res), "Duplicated 1 task.");
        let CliResult::Changed { tasks, .. } = &res else {
            panic!("{res:?}")
        };
        assert_eq!(tasks.len(), 1, "the copy is what is reported");

        let both = find(&mut r, "Buy milk").await;
        assert_eq!(both.len(), 2);
        let (old, new) = if both[0].start.is_some() {
            (&both[0], &both[1])
        } else {
            (&both[1], &both[0])
        };
        assert_ne!(old.uuid, new.uuid);
        assert_eq!(tasks[0].uuid, new.uuid);
        assert!(
            old.start.is_some() && new.start.is_none(),
            "a copy has not been started"
        );
        assert_eq!(new.status, "pending");
        assert_eq!(new.project.as_deref(), Some("Home"));
        assert_eq!(new.priority.as_deref(), Some("H"));
        assert!(new.tags.contains("errand"));
        assert_eq!(new.due, old.due);
        assert_eq!(new.annotations.len(), 1);
        assert_eq!(new.annotations[0].text, "a note");
        assert!(new.entry.is_some() && new.entry >= old.entry);
    }

    #[tokio::test]
    async fn what_follows_duplicate_is_applied_to_the_copy_and_its_words_become_a_note() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Buy milk project:Home +errand").await;
        let d = done(&mut r, &cfg, "1 duplicate Another +x project:Work", false).await;
        assert_eq!(message(&d.result), "Duplicated 1 task.");
        let v = find(&mut r, "Buy milk").await;
        assert_eq!(v.len(), 2, "the words are not a new description");
        let copy = v
            .iter()
            .find(|f| f.project.as_deref() == Some("Work"))
            .expect("the copy");
        assert!(copy.tags.contains("x") && copy.tags.contains("errand"));
        assert_eq!(copy.annotations.len(), 1);
        assert_eq!(copy.annotations[0].text, "Another");
        let original = v
            .iter()
            .find(|f| f.project.as_deref() == Some("Home"))
            .expect("the original");
        assert!(original.annotations.is_empty() && !original.tags.contains("x"));
        // A change that can't be made stops it before anything is copied.
        let d = done(&mut r, &cfg, "1 duplicate due:nonsense", false).await;
        assert!(message(&d.result).starts_with("ERROR:"), "{}", message(&d.result));
        assert_eq!(find(&mut r, "Buy milk").await.len(), 2);
    }

    #[tokio::test]
    async fn a_finished_task_is_copied_as_a_pending_one() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Done thing").await;
        run(&mut r, &cfg, "1 done").await;
        run(&mut r, &cfg, "status:completed duplicate").await;
        let v = find(&mut r, "Done thing").await;
        assert_eq!(v.len(), 2);
        let copy = v.iter().find(|f| f.status == "pending").expect("a pending copy");
        assert_eq!(copy.end, None);
    }

    #[tokio::test]
    async fn copying_a_recurring_instance_gives_a_plain_task_and_a_template_gives_a_template() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Water plants recur:daily due:tomorrow").await;
        run(&mut r, &cfg, "count").await;
        let all = load_facts(&mut r).await.unwrap();
        let instance = all.iter().find(|f| f.parent.is_some()).expect("an instance").clone();
        let template = all
            .iter()
            .find(|f| f.status == "recurring")
            .expect("a template")
            .clone();

        let d = done(&mut r, &cfg, &format!("{} duplicate", instance.uuid), false).await;
        let m = message(&d.result);
        assert!(m.contains("was a recurring task.  The duplicated task is not."), "{m}");
        let CliResult::Changed { tasks, .. } = &d.result else {
            panic!("{m}")
        };
        let copy = load_facts(&mut r)
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.uuid == tasks[0].uuid)
            .unwrap();
        assert!(copy.parent.is_none() && copy.recur.is_none() && copy.imask.is_none());
        assert_eq!(copy.status, "pending");

        let d = done(&mut r, &cfg, &format!("{} duplicate", template.uuid), false).await;
        let m = message(&d.result);
        assert!(
            m.contains("was a parent recurring task.  The duplicated task is too."),
            "{m}"
        );
        let CliResult::Changed { tasks, .. } = &d.result else {
            panic!("{m}")
        };
        let copy = load_facts(&mut r)
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.uuid == tasks[0].uuid)
            .unwrap();
        assert_eq!(copy.status, "recurring");
        assert_eq!(copy.recur.as_deref(), Some("daily"));
        assert_eq!(copy.mask, None, "it has made no instances of its own yet");
    }

    #[tokio::test]
    async fn duplicate_asks_like_any_other_change() {
        let mut r = replica();
        let cfg = parse("bulk=2\n").config;
        for t in ["a", "b", "c"] {
            run(&mut r, &cfg, &format!("add {t}")).await;
        }
        // At `bulk` tasks it asks about each one.
        let (res, wrote) = run(&mut r, &cfg, "+PENDING duplicate").await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert_eq!(ask, Ask::Permission);
        assert!(
            items[0].question.starts_with("Duplicate task "),
            "{}",
            items[0].question
        );
        // No filter at all is the safety net's business.
        let (res, wrote) = run(&mut r, &cfg, "duplicate").await;
        assert!(
            !wrote && message(&res).starts_with("CONFIRM: This command has no filter"),
            "{}",
            message(&res)
        );
        let off = parse("confirmation=off\n").config;
        assert_eq!(
            message(&run(&mut r, &off, "duplicate").await.0),
            "ERROR: Command prevented from running."
        );
        let (res, wrote) = run_yes(&mut r, &cfg, "+PENDING duplicate").await;
        assert!(wrote);
        assert_eq!(message(&res), "Duplicated 3 tasks.");
        assert_eq!(load_facts(&mut r).await.unwrap().len(), 6);
    }

    #[tokio::test]
    async fn log_adds_a_task_that_is_already_completed() {
        let mut r = replica();
        let cfg = Config::default();
        let (res, wrote) = run(&mut r, &cfg, "log Did a thing project:Home +x").await;
        assert!(wrote);
        let m = message(&res);
        assert!(m.starts_with("Logged task ") && m.ends_with('.'), "{m}");
        let f = &find(&mut r, "Did a thing").await[0];
        assert!(m.contains(&f.uuid.to_string()));
        assert_eq!(f.status, "completed");
        assert_eq!(f.project.as_deref(), Some("Home"));
        assert!(f.tags.contains("x"));
        assert!(f.entry.is_some() && f.end == f.entry, "it ended when it was entered");
        // Nothing is left to do.
        assert_eq!(message(&run(&mut r, &cfg, "count +PENDING").await.0), "0");
        // A date given for the end is kept.
        run(&mut r, &cfg, "log Earlier end:yesterday").await;
        let e = &find(&mut r, "Earlier").await[0];
        assert!(e.end < e.entry);
    }

    #[tokio::test]
    async fn log_refuses_what_a_finished_task_cannot_be() {
        let mut r = replica();
        let cfg = Config::default();
        for (line, expected) in [
            (
                "log Repeat due:today recur:daily",
                "ERROR: You cannot log recurring tasks.",
            ),
            ("log Hold wait:tomorrow", "ERROR: You cannot log waiting tasks."),
            (
                "project:Home log Thing",
                "ERROR: log takes a description and modifications, not a filter",
            ),
        ] {
            let (res, wrote) = run(&mut r, &cfg, line).await;
            assert!(!wrote, "{line}");
            assert_eq!(message(&res), expected, "{line}");
        }
        let (res, _) = run(&mut r, &cfg, "log").await;
        assert!(message(&res).starts_with("ERROR:"), "{}", message(&res));
        assert!(load_facts(&mut r).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn log_follows_the_active_contexts_write_rule() {
        let mut r = replica();
        let cfg = parse("context=work\ncontext.work.read=+work\ncontext.work.write=+work\n").config;
        run(&mut r, &cfg, "log Reviewed").await;
        assert!(find(&mut r, "Reviewed").await[0].tags.contains("work"));
    }

    #[tokio::test]
    async fn stats_counts_the_database() {
        let mut r = replica();
        let cfg = quiet();
        run(&mut r, &cfg, "add one project:Home +a").await;
        run(&mut r, &cfg, "add two +b").await;
        run(&mut r, &cfg, "add three wait:tomorrow").await;
        run(&mut r, &cfg, "add four").await;
        run(&mut r, &cfg, "4 delete").await;
        run(&mut r, &cfg, "1 done").await;
        run(&mut r, &cfg, "2 annotate hello").await;
        let (res, wrote) = run(&mut r, &cfg, "stats").await;
        assert!(!wrote);
        let CliResult::Table(t) = res else { panic!("{res:?}") };
        assert_eq!(t.headers, ["Category", "Data"]);
        let v = |n: &str| {
            t.rows
                .iter()
                .find(|row| row[0] == n)
                .map(|row| row[1].clone())
                .unwrap_or_default()
        };
        assert_eq!(
            ["Pending", "Waiting", "Recurring", "Completed", "Deleted", "Total"].map(v),
            ["1", "1", "0", "1", "1", "4"]
        );
        assert_eq!([v("Annotations"), v("Unique tags"), v("Projects")], ["1", "2", "1"]);
        assert_eq!(v("Tasks tagged"), "50%");
        assert!(v("Average desc length").ends_with(" characters"));
        // It takes a filter.
        let (res, _) = run(&mut r, &cfg, "project:Home stats").await;
        let CliResult::Table(t) = res else { panic!() };
        assert_eq!(t.rows.iter().find(|row| row[0] == "Total").unwrap()[1], "1");
    }

    #[tokio::test]
    async fn context_defines_lists_shows_switches_and_deletes() {
        let mut r = replica();
        let cfg = quiet();
        assert_eq!(
            message(&done(&mut r, &cfg, "context", false).await.result),
            "ERROR: No contexts defined."
        );
        assert_eq!(
            message(&done(&mut r, &cfg, "context show", false).await.result),
            "No context is currently applied."
        );

        run(&mut r, &cfg, "add a +errand").await;
        let d = done(&mut r, &cfg, "context define work +errand", false).await;
        assert_eq!(
            message(&d.result),
            "Context 'work' defined (read, write). Use 'task context work' to activate."
        );
        assert!(!d.wrote, "settings are saved by the caller, not as tasks");
        let cfg = d.config.expect("new settings");
        assert_eq!(cfg.contexts["work"].read.as_deref(), Some("+errand"));
        assert_eq!(cfg.contexts["work"].write.as_deref(), Some("+errand"));
        assert_eq!(cfg.active_context, None);

        // The list has a read row and a write row for each, and says which is active.
        let d = done(&mut r, &cfg, "context list", false).await;
        let CliResult::Table(t) = d.result else { panic!() };
        assert_eq!(t.headers, ["Name", "Type", "Definition", "Active"]);
        assert_eq!(
            t.rows,
            [["work", "read", "+errand", "no"], ["", "write", "+errand", "no"]]
        );
        let d = done(&mut r, &cfg, "context", false).await;
        let CliResult::Table(t) = d.result else { panic!() };
        assert_eq!(t.footer, ["Use 'task context none' to unset the current context."]);

        let d = done(&mut r, &cfg, "context work", false).await;
        assert_eq!(
            message(&d.result),
            "Context 'work' set. Use 'task context none' to remove."
        );
        let cfg = d.config.expect("new settings");
        assert_eq!(cfg.active_context.as_deref(), Some("work"));
        assert_eq!(
            message(&done(&mut r, &cfg, "context show", false).await.result),
            "Context 'work' with \n\n* read filter: '+errand'\n* write filter: '+errand'\n\nis currently applied."
        );
        // It is in force: only tagged tasks show, and new ones get the tag.
        run(&mut r, &cfg, "add b").await;
        assert_eq!(shown(&mut r, &cfg, "list").await, (2, 2));
        assert!(find(&mut r, "b").await[0].tags.contains("errand"));
        run(&mut r, &Config::default(), "add c").await;
        assert_eq!(shown(&mut r, &cfg, "list").await, (2, 2));
        assert_eq!(shown(&mut r, &Config::default(), "list").await, (3, 3));

        assert_eq!(
            message(&done(&mut r, &cfg, "context nope", false).await.result),
            "ERROR: Context 'nope' not found."
        );
        let d = done(&mut r, &cfg, "context none", false).await;
        assert_eq!(message(&d.result), "Context unset.");
        let off = d.config.expect("new settings");
        assert_eq!(off.active_context, None);
        assert_eq!(
            message(&done(&mut r, &off, "context none", false).await.result),
            "ERROR: Context not unset."
        );

        // Deleting the active one unsets it too.
        let d = done(&mut r, &cfg, "context delete work", false).await;
        assert_eq!(message(&d.result), "Context 'work' deleted.");
        let gone = d.config.expect("new settings");
        assert!(gone.contexts.is_empty() && gone.active_context.is_none());
        assert_eq!(
            message(&done(&mut r, &gone, "context delete work", false).await.result),
            "ERROR: Context 'work' not found."
        );
        assert_eq!(
            message(&done(&mut r, &gone, "context delete", false).await.result),
            "ERROR: Context name needs to be specified."
        );
    }

    #[tokio::test]
    async fn a_filter_that_cannot_be_a_write_rule_defines_a_read_only_context() {
        let mut r = replica();
        for (filter, why) in [
            ("project:Home or project:Work", "contains the 'OR' operator"),
            (
                "due.before:tomorrow",
                "contains an attribute modifier 'due.before:tomorrow'",
            ),
            ("+x -y", "contains tag exclusion '-y'"),
        ] {
            let d = done(&mut r, &quiet(), &format!("context define c {filter}"), false).await;
            let m = message(&d.result);
            assert!(m.contains(&format!("because it {why}.")), "{filter}: {m}");
            assert!(
                m.ends_with("Context 'c' defined (read only). Use 'task context c' to activate."),
                "{m}"
            );
            let cfg = d.config.expect("new settings");
            assert_eq!(cfg.contexts["c"].read.as_deref(), Some(filter));
            assert_eq!(cfg.contexts["c"].write, None);
        }
        // `is` and `equals` are changes too.
        let d = done(&mut r, &quiet(), "context define c project.is:Home", false).await;
        assert!(message(&d.result).contains("(read, write)"));
    }

    #[tokio::test]
    async fn context_refuses_what_it_cannot_define() {
        let mut r = replica();
        let cfg = quiet();
        for (line, expected) in [
            (
                "context define work",
                "ERROR: Both context name and its definition must be provided.",
            ),
            (
                "context define",
                "ERROR: Both context name and its definition must be provided.",
            ),
            (
                "context define list +x",
                "ERROR: The name 'list' is reserved and not allowed to use as a context name.",
            ),
            (
                "context define none +x",
                "ERROR: The name 'none' is reserved and not allowed to use as a context name.",
            ),
            (
                "context define a.b +x",
                "ERROR: 'a.b' can't be a context name: use letters, digits and underscores.",
            ),
        ] {
            let d = done(&mut r, &cfg, line, false).await;
            assert_eq!(message(&d.result), expected, "{line}");
            assert!(d.config.is_none(), "{line}");
        }
        let m = message(
            &done(&mut r, &cfg, "context define work due.nonsense:x", false)
                .await
                .result,
        );
        assert!(m.starts_with("ERROR: Filter validation failed: "), "{m}");
    }

    #[tokio::test]
    async fn context_asks_first_when_confirmation_is_on() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add a +errand").await;
        // A filter that matches no pending task asks; one that matches goes ahead.
        let d = done(&mut r, &cfg, "context define empty project:Nowhere", false).await;
        assert_eq!(
            message(&d.result),
            "CONFIRM: The filter 'project:Nowhere' matches 0 pending tasks. Do you wish to continue?"
        );
        assert!(d.config.is_none());
        let d = done(&mut r, &cfg, "context define empty project:Nowhere", true).await;
        assert!(d.config.is_some());
        assert!(done(&mut r, &cfg, "context define work +errand", false)
            .await
            .config
            .is_some());

        let cfg = parse("context.work.read=+errand\n").config;
        let d = done(&mut r, &cfg, "context delete work", false).await;
        assert_eq!(message(&d.result), "CONFIRM: Do you want to delete context 'work'?");
        assert!(d.config.is_none());
        assert!(done(&mut r, &cfg, "context delete work", true).await.config.is_some());
        // Switching never asks.
        assert!(done(&mut r, &cfg, "context work", false).await.config.is_some());
    }

    #[tokio::test]
    async fn a_credential_like_context_name_is_refused_and_not_echoed() {
        let mut r = replica();
        let d = done(&mut r, &quiet(), "context define sync_secret +x", false).await;
        let m = message(&d.result);
        assert!(m.starts_with("ERROR:") && d.config.is_none(), "{m}");
    }
}

mod commands_and_get {
    use super::*;

    async fn text(r: &mut R, cfg: &Config, line: &str) -> String {
        message(&run(r, cfg, line).await.0)
    }

    #[tokio::test]
    async fn commands_lists_every_command_and_report_with_what_it_takes() {
        let mut r = replica();
        let cfg = parse("report.mine.description=My own\nreport.mine.columns=id,description\n").config;
        let (res, wrote) = run(&mut r, &cfg, "commands").await;
        assert!(!wrote);
        let CliResult::Table(t) = res else { panic!("{res:?}") };
        assert_eq!(
            t.headers,
            [
                "Command",
                "Category",
                "R/W",
                "ID",
                "GC",
                "Recur",
                "Context",
                "Filter",
                "Mods",
                "Misc",
                "Description"
            ]
        );
        let row = |n: &str| {
            t.rows
                .iter()
                .find(|r| r[0] == n)
                .unwrap_or_else(|| panic!("{n}"))
                .clone()
        };
        assert_eq!(
            row("add"),
            [
                "add",
                "operation",
                "RW",
                "",
                "",
                "",
                "Ctxt",
                "",
                "Mods",
                "",
                "Adds a new task"
            ]
        );
        assert_eq!(
            row("undo"),
            [
                "undo",
                "operation",
                "RW",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "Reverts the most recent change to a task"
            ]
        );
        // A report is a command that shows tasks; its description is the report's own.
        assert_eq!(
            row("next"),
            [
                "next",
                "report",
                "RO",
                "ID",
                "GC",
                "Recur",
                "Ctxt",
                "Filt",
                "",
                "",
                "Most urgent tasks"
            ]
        );
        assert_eq!(row("mine")[1..3], ["report", "RO"]);
        assert_eq!(row("mine")[10], "My own");
        // The names Taskwarrior uses, in its order (an underscore sorts first); nothing this app lacks.
        let names: Vec<&str> = t.rows.iter().map(|r| r[0].as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        assert_eq!(names[0], "_get");
        for gone in ["edit", "import-v2", "info", "_rows", "sync"] {
            assert!(!names.contains(&gone), "{gone}");
        }
        for new in [
            "commands",
            "context",
            "duplicate",
            "import",
            "log",
            "purge",
            "stats",
            "synchronize",
        ] {
            assert!(names.contains(&new), "{new}");
        }
        // `sync` still reaches it, as an abbreviation.
        assert!(text(&mut r, &cfg, "sync").await.contains("automatic"));
    }

    #[tokio::test]
    async fn get_prints_the_value_of_each_dom_reference() {
        let mut r = replica();
        let cfg = parse("bulk=7\n").config;
        run(
            &mut r,
            &cfg,
            "add Third project:Home.Kitchen +a +b due:2026-12-25T08:30 priority:H",
        )
        .await;
        run(&mut r, &cfg, "1 annotate hello").await;
        assert_eq!(
            text(
                &mut r,
                &cfg,
                "_get 1.description 1.project 1.due 1.due.year 1.tags rc.bulk tw.version"
            )
            .await,
            "Third Home.Kitchen 2026-12-25T08:30:00 2026 a,b 7 3.5.0"
        );
        assert_eq!(
            text(
                &mut r,
                &cfg,
                "_get 1.priority 1.annotations.1.description 1.annotations.count"
            )
            .await,
            "H hello 1"
        );
        // A reference with nothing behind it is an empty value, not an error, and still takes its place.
        assert_eq!(
            text(&mut r, &cfg, "_get 1.start 1.project 9.description rc.nosuch").await,
            " Home.Kitchen  "
        );
        // A setting's default is its value too, and a command's own `rc.` override is not a reference.
        assert_eq!(
            text(&mut r, &Config::default(), "rc.bulk:5 _get rc.bulk rc.confirmation").await,
            "5 1"
        );
    }

    #[tokio::test]
    async fn get_reads_the_recurrence_attributes_too() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add Water plants due:tomorrow recur:weekly").await;
        run(&mut r, &cfg, "count").await; // the first command makes the first instance
        assert_eq!(text(&mut r, &cfg, "_get 1.mask 1.imask").await, "- 0");
    }

    #[tokio::test]
    async fn get_wants_dom_references_and_nothing_else() {
        let mut r = replica();
        let cfg = Config::default();
        run(&mut r, &cfg, "add one").await;
        for (line, expected) in [
            ("_get", "ERROR: No DOM reference specified."),
            ("_get rc.bulk:5", "ERROR: No DOM reference specified."),
            ("_get foo", "ERROR: 'foo' is not a DOM reference."),
            ("_get 1.description foo", "ERROR: 'foo' is not a DOM reference."),
            ("_get 1.nothing", "ERROR: '1.nothing' is not a DOM reference."),
            ("_get tw.bogus", "ERROR: 'tw.bogus' is not a DOM reference."),
        ] {
            let (res, wrote) = run(&mut r, &cfg, line).await;
            assert!(!wrote, "{line}");
            assert_eq!(message(&res), expected, "{line}");
        }
    }

    #[tokio::test]
    async fn get_never_reveals_a_credential() {
        let mut r = replica();
        // The settings the app holds contain nothing of the kind (they are refused on the way in), so a
        // reference to one is just empty.
        for line in [
            "_get rc.sync.encryption_secret",
            "_get rc.taskd.password",
            "_get rc.sync.aws.secret_access_key",
        ] {
            assert_eq!(text(&mut r, &quiet_cfg(), line).await, "", "{line}");
        }
    }

    fn quiet_cfg() -> Config {
        parse("confirmation=off\nsync.encryption_secret=hunter2\n").config
    }
}

mod import_command {
    use super::*;
    use std::sync::Arc;
    use tc_core::hooks::{Hooked, Hooks, Reject};
    use tc_core::import::{import, ImportOut};
    use tc_core::model::Facts;

    async fn go(r: &mut R, cfg: &Config, text: &str, apply: bool) -> Result<ImportOut, String> {
        let mut undo = UNDO.with(|u| u.borrow().clone());
        let out = import(r, cfg, clock(), text, apply, &mut undo, None).await;
        UNDO.with(|u| *u.borrow_mut() = undo);
        out
    }

    fn counts(o: &ImportOut) -> (usize, usize, usize) {
        (o.added, o.modified, o.skipped)
    }

    async fn export_of(r: &mut R, cfg: &Config) -> serde_json::Value {
        let CliResult::File { text, .. } = run(r, cfg, "export").await.0 else {
            panic!("not a file")
        };
        let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
        // Urgency depends on the time the command ran; ids on the replica.
        for t in v.as_array_mut().unwrap() {
            t.as_object_mut().unwrap().remove("urgency");
        }
        v
    }

    const FILE: &str = r#"[
{"description":"First","uuid":"11111111-1111-4111-8111-111111111111","status":"pending","entry":"20261001T100000Z",
 "project":"Home","tags":["a","b"],"due":"20261225T083000Z","priority":"H",
 "annotations":[{"entry":"20261002T000000Z","description":"note one"}],
 "depends":["22222222-2222-4222-8222-222222222222"],"id":99,"urgency":99},
{"description":"Second","uuid":"22222222-2222-4222-8222-222222222222","status":"completed","entry":"20261001T100000Z"},
{"description":"Later","uuid":"33333333-3333-4333-8333-333333333333","status":"waiting","wait":"20271001T000000Z"}
]"#;

    #[tokio::test]
    async fn a_file_is_added_then_skipped_when_imported_again() {
        let mut r = replica();
        let cfg = Config::default();
        let o = go(&mut r, &cfg, FILE, true).await.unwrap();
        assert_eq!((counts(&o), o.applied), ((3, 0, 0), true));
        assert_eq!(o.lines.iter().map(|l| l.action).collect::<Vec<_>>(), ["add"; 3]);
        let all = load_facts(&mut r).await.unwrap();
        let first = all.iter().find(|f| f.description == "First").unwrap();
        assert_eq!(first.project.as_deref(), Some("Home"));
        assert!(first.tags.contains("a") && first.tags.contains("b"));
        assert_eq!(first.annotations[0].text, "note one");
        assert_eq!(first.depends.len(), 1);
        assert_eq!(first.entry, Some(1_790_848_800), "the file's entry is kept");
        let second = all.iter().find(|f| f.description == "Second").unwrap();
        assert_eq!(second.status, "completed");
        assert!(second.end.is_some(), "a finished task without an end gets one");
        let later = all.iter().find(|f| f.description == "Later").unwrap();
        assert_eq!((later.status.as_str(), later.wait), ("pending", Some(1_822_348_800)));

        // The same file again changes nothing, though `entry` and `end` of two tasks were made up.
        let o = go(&mut r, &cfg, FILE, true).await.unwrap();
        assert_eq!((counts(&o), o.applied), ((0, 0, 3), false));
        assert_eq!(load_facts(&mut r).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn what_export_writes_comes_back_as_it_was() {
        let mut a = replica();
        let cfg = parse("uda.est.type=numeric\nuda.size.type=string\n").config;
        run(
            &mut a,
            &cfg,
            "add Water plants project:Home.Kitchen +a +b due:2026-12-25T08:30 priority:H est:3.5 size:big",
        )
        .await;
        run(&mut a, &cfg, "add Pay rent due:tomorrow +bills").await;
        run(&mut a, &cfg, "1 annotate hello").await;
        run(&mut a, &cfg, "2 start").await;
        run(&mut a, &cfg, "add Done thing").await;
        run(&mut a, &cfg, "3 done").await;
        run(&mut a, &cfg, "add Hold wait:2030-01-01").await;
        let before = export_of(&mut a, &cfg).await;
        let CliResult::File { text, .. } = run(&mut a, &cfg, "export").await.0 else {
            panic!()
        };

        let mut b = replica();
        let o = go(&mut b, &cfg, &text, true).await.unwrap();
        assert_eq!(counts(&o), (4, 0, 0));
        assert_eq!(export_of(&mut b, &cfg).await, before);
        // And once more: nothing to do.
        assert_eq!(counts(&go(&mut b, &cfg, &text, true).await.unwrap()), (0, 0, 4));
    }

    #[tokio::test]
    async fn a_changed_task_takes_the_files_attributes_whole() {
        let mut r = replica();
        let cfg = Config::default();
        go(&mut r, &cfg, FILE, true).await.unwrap();
        let changed = r#"{"description":"First renamed","uuid":"11111111-1111-4111-8111-111111111111",
            "status":"pending","entry":"20261001T100000Z","priority":"L"}"#;
        let o = go(&mut r, &cfg, changed, true).await.unwrap();
        assert_eq!((counts(&o), o.lines[0].action), ((0, 1, 0), "mod"));
        let all = load_facts(&mut r).await.unwrap();
        let f = all.iter().find(|f| f.description == "First renamed").unwrap();
        assert_eq!(f.priority.as_deref(), Some("L"));
        assert!(f.project.is_none() && f.tags.is_empty() && f.annotations.is_empty() && f.depends.is_empty());
        assert!(f.due.is_none(), "what the file leaves out is removed");
        assert!(f.modified.unwrap() >= f.entry.unwrap());
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn an_entry_or_end_made_up_here_never_replaces_a_stored_one() {
        let mut r = replica();
        let cfg = Config::default();
        let id = "11111111-1111-4111-8111-111111111111";
        // A file with no `entry`: the task gets one when it is imported.
        let first = format!(r#"{{"description":"x","uuid":"{id}","priority":"H"}}"#);
        go(&mut r, &cfg, &first, true).await.unwrap();
        let entry = load_facts(&mut r).await.unwrap()[0].entry;
        // The same task again, with one thing changed and still no `entry`, some time later.
        let second = format!(r#"{{"description":"x","uuid":"{id}","priority":"L"}}"#);
        let o = go(&mut r, &cfg, &second, true).await.unwrap();
        assert_eq!(counts(&o), (0, 1, 0));
        let f = &load_facts(&mut r).await.unwrap()[0];
        assert_eq!(f.priority.as_deref(), Some("L"));
        assert_eq!(f.entry, entry, "the creation date is not reset by a later import");
        // The same for a finished task's `end`.
        let done = r#"{"description":"y","uuid":"22222222-2222-4222-8222-222222222222","status":"completed"}"#;
        go(&mut r, &cfg, done, true).await.unwrap();
        let end = load_facts(&mut r)
            .await
            .unwrap()
            .iter()
            .find(|f| f.description == "y")
            .unwrap()
            .end;
        let again = r#"{"description":"y renamed","uuid":"22222222-2222-4222-8222-222222222222","status":"completed"}"#;
        go(&mut r, &cfg, again, true).await.unwrap();
        let f = load_facts(&mut r).await.unwrap();
        assert_eq!(f.iter().find(|f| f.description == "y renamed").unwrap().end, end);
    }

    #[tokio::test]
    async fn a_check_writes_nothing_and_a_bad_task_stops_everything() {
        let mut r = replica();
        let cfg = Config::default();
        let o = go(&mut r, &cfg, FILE, false).await.unwrap();
        assert_eq!((counts(&o), o.applied), ((3, 0, 0), false));
        assert!(load_facts(&mut r).await.unwrap().is_empty());

        let bad = r#"[{"description":"fine"},{"description":"bad","due":"garbage"},{"description":"also fine"}]"#;
        let e = go(&mut r, &cfg, bad, true).await.err().unwrap();
        assert!(e.starts_with("Task 2: due: 'garbage'"), "{e}");
        assert!(load_facts(&mut r).await.unwrap().is_empty(), "nothing was imported");
        let e = go(&mut r, &cfg, "{\"description\":\"a\"}\n{broken", true)
            .await
            .err()
            .unwrap();
        assert!(e.contains("line 2"), "{e}");
    }

    #[tokio::test]
    async fn a_uuid_twice_is_one_task_and_a_warning() {
        let mut r = replica();
        let cfg = Config::default();
        let twice = r#"[{"description":"one","uuid":"11111111-1111-4111-8111-111111111111"},
                        {"description":"two","uuid":"11111111-1111-4111-8111-111111111111"}]"#;
        let o = go(&mut r, &cfg, twice, true).await.unwrap();
        assert_eq!(counts(&o), (1, 0, 0));
        assert_eq!(o.warnings.len(), 2);
        assert!(o.warnings[0].starts_with("Input contains UUID '11111111-1111-4111-8111-111111111111' 2 times."));
        assert_eq!(descs(&mut r).await, ["two"], "the later one wins");
    }

    #[tokio::test]
    async fn defaults_fill_what_a_task_lacks_but_not_what_it_has() {
        let mut r = replica();
        let cfg = parse("default.project=Inbox\nuda.size.type=string\nuda.size.default=medium\n").config;
        let file = r#"[{"description":"bare"},{"description":"owned","project":"Work","size":"big"}]"#;
        go(&mut r, &cfg, file, true).await.unwrap();
        let all = load_facts(&mut r).await.unwrap();
        let get = |d: &str| all.iter().find(|f| f.description == d).unwrap().clone();
        assert_eq!(get("bare").project.as_deref(), Some("Inbox"));
        assert_eq!(get("bare").extra.get("size").map(String::as_str), Some("medium"));
        assert_eq!(get("owned").project.as_deref(), Some("Work"));
        assert_eq!(get("owned").extra.get("size").map(String::as_str), Some("big"));
    }

    #[tokio::test]
    async fn an_import_is_one_undo_step() {
        let mut r = replica();
        let cfg = parse("confirmation=off\n").config;
        go(&mut r, &cfg, FILE, true).await.unwrap();
        assert_eq!(load_facts(&mut r).await.unwrap().len(), 3);
        run(&mut r, &cfg, "undo").await;
        assert!(load_facts(&mut r).await.unwrap().is_empty());
    }

    #[derive(Debug)]
    struct NoNope;
    impl Hooks for NoNope {
        fn on_add(&self, h: &mut Hooked, task: Facts) -> Result<Facts, Reject> {
            if task.description == "nope" {
                h.warn("not today");
                return Err("no nopes".into());
            }
            Ok(task)
        }
    }

    #[tokio::test]
    async fn a_hook_can_refuse_a_task_and_then_nothing_is_imported() {
        let mut r = replica();
        let cfg = Config::default();
        let file = r#"[{"description":"fine"},{"description":"nope"}]"#;
        let e = import(
            &mut r,
            &cfg,
            clock(),
            file,
            true,
            &mut UndoStack::default(),
            Some(Arc::new(NoNope)),
        )
        .await
        .err()
        .unwrap();
        // The reason, then what the hook said before it refused.
        assert_eq!(e, "no nopes\nnot today");
        assert!(load_facts(&mut r).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_import_is_limited_in_size() {
        let mut r = replica();
        let many = format!(
            "[{}]",
            vec![r#"{"description":"x"}"#; tc_core::import::MAX_TASKS + 1].join(",")
        );
        let e = go(&mut r, &Config::default(), &many, false).await.err().unwrap();
        assert!(e.contains("import at most"), "{e}");
    }

    #[tokio::test]
    async fn a_chained_template_is_kept_as_it_came_and_recurs_periodically_like_task_does() {
        let mut r = replica();
        let cfg = Config::default();
        let file = r#"{"description":"Mow","uuid":"11111111-1111-4111-8111-111111111111","status":"recurring",
            "due":"20271001T000000Z","recur":"weekly","rtype":"chained","entry":"20261001T100000Z"}"#;
        go(&mut r, &cfg, file, true).await.unwrap();
        // The next command makes the instances; the template still says what it said.
        run(&mut r, &cfg, "count").await;
        let all = load_facts(&mut r).await.unwrap();
        assert!(
            all.iter().any(|f| f.parent.is_some()),
            "instances are made on the periodic schedule"
        );
        let exported = export_of(&mut r, &cfg).await;
        let template = exported
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["uuid"] == "11111111-1111-4111-8111-111111111111")
            .unwrap();
        assert_eq!(template["rtype"], "chained");
        // And importing the export again changes nothing.
        let CliResult::File { text, .. } = run(&mut r, &cfg, "export").await.0 else {
            panic!()
        };
        assert_eq!(counts(&go(&mut r, &cfg, &text, true).await.unwrap()), (0, 0, all.len()));
    }

    #[tokio::test]
    async fn the_import_command_asks_the_page_for_a_file() {
        let mut r = replica();
        assert!(matches!(
            run(&mut r, &Config::default(), "import").await.0,
            CliResult::Import
        ));
    }
}

mod purge_command {
    use super::*;
    use tc_core::import::import;

    async fn put(r: &mut R, cfg: &Config, json: &str) {
        let mut undo = UndoStack::default();
        import(r, cfg, clock(), json, true, &mut undo, None).await.unwrap();
    }

    fn quiet() -> Config {
        parse("confirmation=off\n").config
    }

    async fn uuids_of(r: &mut R, description: &str) -> Vec<uuid::Uuid> {
        load_facts(r)
            .await
            .unwrap()
            .into_iter()
            .filter(|f| f.description == description)
            .map(|f| f.uuid)
            .collect()
    }

    #[tokio::test]
    async fn only_a_deleted_task_can_be_purged() {
        let mut r = replica();
        let cfg = quiet();
        run(&mut r, &cfg, "add alive").await;
        run(&mut r, &cfg, "add gone").await;
        run(&mut r, &cfg, "description:gone delete").await;
        let (res, wrote) = run(&mut r, &cfg, "description:alive purge").await;
        assert!(!wrote);
        assert_eq!(
            message(&res),
            "Purged 0 tasks.\nNo deleted tasks specified. Maybe you forgot to delete tasks first?"
        );
        assert_eq!(descs(&mut r).await, ["alive", "gone"]);

        let (res, wrote) = run(&mut r, &cfg, "description:gone purge").await;
        assert!(wrote);
        assert_eq!(message(&res), "Purged 1 task.");
        assert_eq!(descs(&mut r).await, ["alive"], "it is gone, not just marked");
        // The filter may come after the word, as in Taskwarrior.
        run(&mut r, &cfg, "add also").await;
        run(&mut r, &cfg, "description:also delete").await;
        assert_eq!(
            message(&run(&mut r, &cfg, "purge status:deleted").await.0),
            "Purged 1 task."
        );
    }

    #[tokio::test]
    async fn it_asks_about_each_task_unless_confirmation_is_off() {
        let mut r = replica();
        let cfg = parse("confirmation=off\n").config;
        for t in ["one", "two"] {
            run(&mut r, &cfg, &format!("add {t}")).await;
        }
        run(&mut r, &cfg, "status:pending delete").await;
        let asking = Config::default();
        let (res, wrote) = run(&mut r, &asking, "status:deleted purge").await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert_eq!(ask, Ask::Permission);
        assert_eq!(items.len(), 2);
        assert!(
            items.iter().all(|i| i.question.starts_with("Permanently remove task ")),
            "{items:?}"
        );
        // Declining all of them purges nothing.
        let o = Options {
            approved: Some(vec![]),
            ..Options::default()
        };
        let (res, wrote) = run_opts(&mut r, &asking, "status:deleted purge", o).await;
        assert!(!wrote, "{}", message(&res));
        assert_eq!(descs(&mut r).await.len(), 2);
        // Choosing one purges that one.
        let o = Options {
            approved: Some(vec![items[0].key.clone()]),
            ..Options::default()
        };
        let (res, _) = run_opts(&mut r, &asking, "status:deleted purge", o).await;
        assert_eq!(message(&res), "Purged 1 task. Skipped 1 task.");
        assert_eq!(descs(&mut r).await.len(), 1);
    }

    #[tokio::test]
    async fn without_a_filter_it_is_stopped_or_asked_like_any_other_change() {
        let mut r = replica();
        run(&mut r, &quiet(), "add gone").await;
        run(&mut r, &quiet(), "status:pending delete").await;
        assert_eq!(
            message(&run(&mut r, &quiet(), "purge").await.0),
            "ERROR: Command prevented from running."
        );
        let (res, wrote) = run(&mut r, &Config::default(), "purge").await;
        assert!(
            !wrote && message(&res).starts_with("CONFIRM: This command has no filter"),
            "{}",
            message(&res)
        );
        assert_eq!(descs(&mut r).await, ["gone"]);
    }

    #[tokio::test]
    async fn what_waited_on_a_purged_task_no_longer_does() {
        let mut r = replica();
        let cfg = quiet();
        run(&mut r, &cfg, "add blocker").await;
        run(&mut r, &cfg, "add keeps waiting").await;
        run(&mut r, &cfg, "add other").await;
        run(&mut r, &cfg, "description:keeps modify depends:1").await;
        run(&mut r, &cfg, "description:other modify depends:1,2").await;
        run(&mut r, &cfg, "description:blocker delete").await;
        run(&mut r, &cfg, "description:blocker purge").await;
        let all = load_facts(&mut r).await.unwrap();
        let keeps = all.iter().find(|f| f.description == "keeps waiting").unwrap();
        let other = all.iter().find(|f| f.description == "other").unwrap();
        assert!(keeps.depends.is_empty(), "nothing points at a task that is gone");
        assert_eq!(other.depends.len(), 1, "its other dependency stays");
        assert_eq!(other.depends[0], keeps.uuid);
    }

    const TEMPLATE: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const CHILD: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    /// A recurring template (deleted) and one instance of it, in the given state.
    async fn series(r: &mut R, cfg: &Config, child_status: &str) {
        let json = format!(
            r#"[{{"description":"Water","uuid":"{TEMPLATE}","status":"deleted","due":"20271001T000000Z","recur":"daily","mask":"-","rtype":"periodic","entry":"20261001T100000Z","end":"20261002T100000Z"}},
               {{"description":"Water","uuid":"{CHILD}","status":"{child_status}","parent":"{TEMPLATE}","imask":0,"due":"20271001T000000Z","entry":"20261001T100000Z"{}}}]"#,
            if child_status == "deleted" {
                r#","end":"20261002T100000Z""#
            } else {
                ""
            }
        );
        put(r, cfg, &json).await;
    }

    #[tokio::test]
    async fn a_template_with_a_live_instance_cannot_be_purged() {
        let mut r = replica();
        let cfg = parse("confirmation=off\nrecurrence=off\n").config;
        series(&mut r, &cfg, "pending").await;
        let (res, wrote) = run(&mut r, &cfg, "status:deleted purge").await;
        assert!(!wrote);
        let m = message(&res);
        assert!(
            m.starts_with("ERROR: Task 'Water' is a recurrence template. Its child task ")
                && m.ends_with(" must be deleted before it can be purged."),
            "{m}"
        );
        assert_eq!(uuids_of(&mut r, "Water").await.len(), 2, "nothing was purged");
    }

    #[tokio::test]
    async fn a_template_takes_its_deleted_instances_as_recurrence_confirmation_says() {
        for (setting, expected) in [("yes", "Purged 2 tasks."), ("no", "ERROR: Purge operation aborted.")] {
            let mut r = replica();
            let cfg = parse(&format!(
                "confirmation=off\nrecurrence=off\nrecurrence.confirmation={setting}\n"
            ))
            .config;
            series(&mut r, &cfg, "deleted").await;
            let (res, _) = run(&mut r, &cfg, "status:deleted purge").await;
            assert_eq!(message(&res), expected, "recurrence.confirmation={setting}");
            let left = uuids_of(&mut r, "Water").await.len();
            assert_eq!(left, if setting == "yes" { 0 } else { 2 });
        }
        // Only the template selected: its instances still go with it.
        let mut r = replica();
        let cfg = parse("confirmation=off\nrecurrence=off\nrecurrence.confirmation=yes\n").config;
        series(&mut r, &cfg, "deleted").await;
        let (res, _) = run(&mut r, &cfg, &format!("{TEMPLATE} purge")).await;
        assert_eq!(message(&res), "Purged 2 tasks.");
        assert!(uuids_of(&mut r, "Water").await.is_empty());
    }

    #[tokio::test]
    async fn by_default_it_asks_before_taking_the_instances() {
        let mut r = replica();
        let cfg = parse("confirmation=off\nrecurrence=off\n").config;
        series(&mut r, &cfg, "deleted").await;
        let (res, wrote) = run(&mut r, &cfg, &format!("{TEMPLATE} purge")).await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert_eq!(ask, Ask::Extras);
        assert_eq!(
            items[0].question,
            "Task 'Water' is a recurrence template. All its 1 deleted children tasks will be purged as well. Continue?"
        );
        // Not answered yes: aborted, nothing purged.
        let o = Options {
            extras: Some(vec![]),
            ..Options::default()
        };
        let (res, _) = run_opts(&mut r, &cfg, &format!("{TEMPLATE} purge"), o).await;
        assert_eq!(message(&res), "ERROR: Purge operation aborted.");
        assert_eq!(uuids_of(&mut r, "Water").await.len(), 2);
        // Answered yes.
        let (res, wrote) = run_yes(&mut r, &cfg, &format!("{TEMPLATE} purge")).await;
        assert!(wrote);
        assert_eq!(message(&res), "Purged 2 tasks.");
    }

    #[tokio::test]
    async fn a_purge_can_be_undone_with_everything_the_task_had() {
        let mut r = replica();
        let cfg = quiet();
        run(&mut r, &cfg, "add Keep project:Home +a due:2026-12-25").await;
        run(&mut r, &cfg, "1 annotate a note").await;
        run(&mut r, &cfg, "description:Keep delete").await;
        let before = load_facts(&mut r).await.unwrap().remove(0);
        run(&mut r, &cfg, "description:Keep purge").await;
        assert!(load_facts(&mut r).await.unwrap().is_empty());
        let (res, wrote) = run(&mut r, &cfg, "undo").await;
        assert!(wrote, "{}", message(&res));
        let back = load_facts(&mut r).await.unwrap();
        assert_eq!(back.len(), 1);
        let t = &back[0];
        assert_eq!((t.uuid, t.status.as_str()), (before.uuid, "deleted"));
        assert_eq!(t.project.as_deref(), Some("Home"));
        assert!(t.tags.contains("a"));
        assert_eq!(t.annotations[0].text, "a note");
        assert_eq!(t.due, before.due);
    }

    #[tokio::test]
    async fn the_commands_table_lists_purge_as_taskwarrior_does() {
        let mut r = replica();
        let CliResult::Table(t) = run(&mut r, &Config::default(), "commands").await.0 else {
            panic!()
        };
        let row = t.rows.iter().find(|row| row[0] == "purge").expect("purge is listed");
        assert_eq!(
            row[1..],
            [
                "operation",
                "RW",
                "",
                "GC",
                "",
                "Ctxt",
                "Filt",
                "",
                "",
                "Removes the specified tasks from the data files. Causes permanent loss of data."
            ]
        );
    }
}

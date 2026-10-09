//! Real command lines against a real taskchampion `Replica` (in-memory storage).

use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::Replica;
use tc_core::cli::{execute, load_facts, Ask, CliResult, ConfirmItem, Options, UndoStack};
use tc_core::dates::{Clock, DAY};
use tc_core::filter::split_words;
use tc_core::taskrc::{parse, Config};

type R = Replica<InMemoryStorage>;

fn replica() -> R {
    Replica::new(InMemoryStorage::new())
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
        let CliResult::Confirm { ask, items, .. } = &res else { return (res, wrote) };
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
    let CliResult::Confirm { ask, items, .. } = res else { panic!("not a question: {res:?}") };
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
    let mut v: Vec<_> = load_facts(r).await.unwrap().into_iter().map(|f| f.description).collect();
    v.sort();
    v
}

#[tokio::test]
async fn add_then_report_shows_it_with_fields() {
    let mut r = replica();
    let cfg = Config::default();
    let (res, wrote) = run(&mut r, &cfg, "add Buy milk project:Home priority:H +errand due:tomorrow").await;
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
    assert!(load_facts(&mut r).await.unwrap().iter().find(|x| x.description == "first").unwrap().start.is_some());
    run(&mut r, &cfg, "1 stop").await;
    assert!(load_facts(&mut r).await.unwrap().iter().find(|x| x.description == "first").unwrap().start.is_none());

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
        load_facts(&mut r).await.unwrap().iter().find(|x| x.description == "second").unwrap().status,
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
    assert!(!wrote && message(&res).contains("complete 3 tasks"), "{}", message(&res));
    assert!(load_facts(&mut r).await.unwrap().iter().all(|f| f.status == "pending"));

    let (res, wrote) =
        run_yes(&mut r, &cfg, "project:P done").await;
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
            message(&res).starts_with("CONFIRM: This command has no filter, and will modify all (including completed and deleted) tasks."),
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
        assert!(!wrote && message(&res).starts_with("CONFIRM:"), "{line}: {}", message(&res));
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
    assert!(!wrote && message(&res).contains("This will modify 3 tasks"), "{}", message(&res));
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
    let names: Vec<(&str, &str)> = items.iter().map(|i| (i.description.as_str(), i.question.as_str())).collect();
    assert_eq!(
        names,
        [("a", "Modify task 1 'a'?"), ("b", "Modify task 2 'b'?"), ("c", "Modify task 3 'c'?"), ("d", "Modify task 4 'd'?")]
    );

    // "yes" for a and c, "no" for b and d.
    let yes = vec![items[0].key.clone(), items[2].key.clone()];
    let o = Options { approved: Some(yes), ..Options::default() };
    let (res, wrote) = run_opts(&mut r, &cfg, "1-4 modify +t", o).await;
    assert!(wrote);
    assert_eq!(message(&res), "Modified 2 tasks. Skipped 2 tasks.");
    let tagged: Vec<String> = {
        let mut v: Vec<_> = load_facts(&mut r).await.unwrap().into_iter().filter(|f| f.tags.contains("t")).map(|f| f.description).collect();
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
    let o = Options { approved: Some(vec![]), ..Options::default() };
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
    let (res, _) = run(&mut r, &cfg, "description:a or description:b or description:c or description:d done").await;
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
    assert_eq!((ask, items.len(), message(&res).as_str()), (Ask::Permission, 1, "CONFIRM: Delete task 1 'Pay rent'?"));
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
    assert!(message(&res).starts_with("CONFIRM: The undo command is not reversible."), "{}", message(&res));
    assert_eq!(load_facts(&mut r).await.unwrap()[0].status, "completed", "nothing was undone yet");

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
        let mut v: Vec<String> = f.iter().find(|x| x.description == d).unwrap().depends.iter().map(by).collect();
        v.sort();
        v
    }

    #[tokio::test]
    async fn finishing_the_middle_of_a_chain_offers_to_repair_it() {
        let mut r = replica();
        let cfg = Config::default();
        chain(&mut r, &cfg).await;
        let two = load_facts(&mut r).await.unwrap().iter().find(|f| f.description == "two").unwrap().uuid.to_string();

        let (res, wrote) = run(&mut r, &cfg, &format!("{two} done")).await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert_eq!(ask, Ask::Extras);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].question.as_str(), items[0].description.as_str()), ("Would you like the dependency chain fixed?", "two"));

        // Yes: `one` stops waiting on `two` and waits on `three` instead.
        let o = Options { extras: Some(vec![items[0].key.clone()]), ..Options::default() };
        let (res, wrote) = run_opts(&mut r, &cfg, &format!("{two} done"), o).await;
        assert!(wrote, "{}", message_of(&res));
        assert!(message_of(&res).contains("Repaired the dependencies of 1 task"), "{}", message_of(&res));
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
        let o = Options { extras: Some(vec![]), ..Options::default() };
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
        assert!(!wrote && message_of(&res).starts_with("CONFIRM: Delete task"), "{}", message_of(&res));
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
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "unblocked").await else { panic!() };
    assert_eq!(o.rows.len(), 1);
    run(&mut r, &cfg, "1 done").await;
    assert!(!load_facts(&mut r).await.unwrap().iter().find(|x| x.description == "blocked").unwrap().blocked);
}

/// Urgency of the task called `name` in the `next` report run with the given settings.
async fn urgency_of(r: &mut R, cfg: &Config, line: &str, name: &str) -> f64 {
    let (CliResult::Report(o), _) = run(r, cfg, line).await else { panic!("not a report") };
    o.rows.iter().find(|x| x.facts.description == name).unwrap_or_else(|| panic!("no {name}")).urgency
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
    let (CliResult::Report(o), _) = run(&mut r, &on, "next urgency.over:12.5").await else { panic!() };
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
    let (CliResult::Report(o), _) = run(r, cfg, line).await else { panic!("{line}: not a report") };
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
    assert_eq!(shown(&mut r, &cfg, "list").await, (2, 6), "the report stops at 2 and still says how many matched");
    assert_eq!(shown(&mut r, &cfg, "all").await, (2, 6));
    // The same through `rc.limit:` on the command line, with nothing in the taskrc.
    assert_eq!(shown(&mut r, &plain, "rc.limit:3 list").await, (3, 6));
    // `limit:` in the command beats the setting, both ways.
    assert_eq!(shown(&mut r, &cfg, "list limit:4").await, (4, 6));
    assert_eq!(shown(&mut r, &cfg, "list limit:none").await, (6, 6));
    assert_eq!(shown(&mut r, &cfg, "list limit:0").await, (6, 6));
    assert_eq!(shown(&mut r, &cfg, "rc.limit:5 list").await, (5, 6), "rc beats the taskrc");
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
    let (CliResult::Json { value }, _) = run(&mut r, &cfg, "export").await else { panic!("not json") };
    assert_eq!(value.as_array().unwrap().len(), 5);
    let (CliResult::Json { value }, _) = run(&mut r, &cfg, "status:pending export").await else { panic!("not json") };
    assert_eq!(value.as_array().unwrap().len(), 5, "what the Projects view and reminders read");
}

async fn only_task(r: &mut R, description: &str) -> tc_core::model::Facts {
    load_facts(r).await.unwrap().into_iter().find(|f| f.description == description).unwrap_or_else(|| panic!("no {description}"))
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
    assert!(t.scheduled.is_some_and(|s| s > before + 86_400 * 365), "{:?}", t.scheduled);

    // What the user gives wins, field by field.
    run(&mut r, &cfg, "add explicit project:Work due:2031-05-05").await;
    let t = only_task(&mut r, "explicit").await;
    assert_eq!(t.project.as_deref(), Some("Work"));
    assert!(t.due.is_some_and(|d| d > before + 86_400 * 365 * 4), "the typed due date was kept");
    assert!(t.scheduled.is_some(), "but the scheduled default still applies");

    // Taskwarrior fills them in when a task is added, not when one is changed.
    run(&mut r, &Config::default(), "add later").await;
    let (res, _) = run(&mut r, &cfg, "later modify +x").await;
    let t = only_task(&mut r, "later").await;
    assert!(t.project.is_none() && t.due.is_none() && t.scheduled.is_none(), "{}", message(&res));
}

#[tokio::test]
async fn a_default_due_date_does_not_stand_in_for_a_recurring_tasks_own() {
    let mut r = replica();
    let cfg = parse("default.due=3d\n").config;
    let (res, wrote) = run(&mut r, &cfg, "add Water plants recur:weekly").await;
    assert!(!wrote && message(&res).contains("must also have a 'due' date"), "{}", message(&res));
}

#[tokio::test]
async fn an_unusable_default_is_ignored_and_adding_still_works() {
    let p = parse("default.due=whenever\ndefault.project=\n");
    assert!(p.warnings.iter().any(|w| w.starts_with("default.due:")), "{:?}", p.warnings);
    assert!(!p.config.settings.contains_key("default.due") && !p.config.settings.contains_key("default.project"));
    // Set on a command line instead of a taskrc, the same rules apply.
    let mut r = replica();
    run(&mut r, &Config::default(), "rc.default.project:Inbox rc.default.due:nonsense add via rc").await;
    let t = only_task(&mut r, "via rc").await;
    assert_eq!(t.project.as_deref(), Some("Inbox"));
    assert!(t.due.is_none());
}

async fn names_in(r: &mut R, cfg: &Config, line: &str) -> Vec<String> {
    let (CliResult::Report(o), _) = run(r, cfg, line).await else { panic!("{line}: not a report") };
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
        let (CliResult::Report(o), _) = run(&mut r, &cfg, "all").await else { panic!() };
        o.rows.iter().find(|x| x.facts.description == "the blocker").unwrap().id.unwrap()
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
    let (CliResult::Report(long), _) = run(&mut r, &cfg, "long").await else { panic!() };
    let labels: Vec<&str> = long.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["ID", "A", "Created", "Mod", "Deps", "P", "Project", "Tags", "Recur", "Wait", "Sched", "Due", "Until", "Description"]);
    assert_eq!(long.sort.as_deref(), Some("modified-"));
    let (CliResult::Report(ls), _) = run(&mut r, &cfg, "ls").await else { panic!() };
    let labels: Vec<&str> = ls.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["ID", "A", "D", "Project", "Tags", "R", "Wait", "S", "Due", "Until", "Description"]);
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
        run(&mut r, &plain, &format!("add task {i} +{}", if i < 2 { "a" } else { "b" })).await;
    }
    let rc = "context.work.rc.limit=2\ncontext.home.rc.limit=4\n\
              context.home.rc.report.list.filter=+b\n";

    // No context active: the settings of a context do nothing.
    assert_eq!(shown(&mut r, &parse(rc).config, "list").await, (6, 6));

    let cfg = parse(&format!("context=work\n{rc}")).config;
    assert_eq!(shown(&mut r, &cfg, "list").await, (2, 6));
    // Switching context for one command switches its settings too.
    assert_eq!(shown(&mut r, &cfg, "rc.context:home list").await, (4, 4), "limit 4, and list is filtered to +b");
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
    assert_eq!(count(run(&mut r, &cfg, "count passport").await.0), "1", "found in an annotation");
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
    assert!(!wrote && message(&res).contains("not a valid regular expression"), "{}", message(&res));
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
    let labels: Vec<(&str, usize, usize)> = s.rows.iter().map(|x| (x.label.as_str(), x.depth, x.remaining)).collect();
    assert_eq!(labels, [("(none)", 0, 1), ("Home", 0, 2), ("Kitchen", 1, 1), ("Work", 0, 1)]);

    // Finish one: it shows as progress, and finished projects only appear with the setting.
    let (res, _) = run(&mut r, &cfg, "project:Work done").await;
    assert!(message(&res).contains("Completed"), "{}", message(&res));
    let s = tpl(run(&mut r, &cfg, "summary").await.0);
    assert!(!s.rows.iter().any(|x| x.project == "Work"), "nothing left to do there");
    let all = parse("summary.all.projects=1\n").config;
    let w = tpl(run(&mut r, &all, "summary").await.0).rows.into_iter().find(|x| x.project == "Work").unwrap();
    assert_eq!((w.complete.as_str(), w.bar, w.completed, w.remaining), ("100%", 30, 1, 0));

    // A filter narrows it, an abbreviation works, and an empty result says so.
    let h = tpl(run(&mut r, &cfg, "project:Home summary").await.0);
    assert_eq!(h.rows.iter().map(|x| x.project.as_str()).collect::<Vec<_>>(), ["Home", "Home.Kitchen"]);
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
    assert_eq!(months(&cal(run(&mut r, &cfg, "cal march 2031").await.0))[0], (2031, 3), "abbreviated command, named month");
    let (res, wrote) = run(&mut r, &cfg, "calendar 13 2031").await;
    assert!(!wrote && message(&res).contains("not a valid month"), "{}", message(&res));
    let (res, _) = run(&mut r, &cfg, "calendar whenever").await;
    assert!(message(&res).contains("Could not recognize argument 'whenever'"), "{}", message(&res));
    // `rc.` overrides are settings, not arguments.
    assert_eq!(cal(run(&mut r, &cfg, "rc.calendar.monthsperline:2 calendar").await.0).months.len(), 2);
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
    assert_eq!(cal(run(&mut r, &Config::default(), "rc.weekstart:monday calendar").await.0).weekdays[0], "Mo");
    assert_eq!(cal(run(&mut r, &monday, "rc.weekstart:sunday calendar").await.0).weekdays[0], "Su");
    let p = parse("weekstart=friday\n");
    assert!(p.config.settings.get("weekstart").is_none());
    assert!(p.warnings.iter().any(|w| w.starts_with("weekstart:")), "{:?}", p.warnings);
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
    assert_eq!(last(&chart(run(&mut r, &cfg, "project:Home burndown.daily").await.0)), (1, 0, 0));
    assert_eq!(last(&chart(run(&mut r, &cfg, "burndown.daily project:Work").await.0)), (0, 0, 1));
    // The other periods have their own commands.
    assert_eq!(chart(run(&mut r, &cfg, "burndown.weekly").await.0).bars.len(), 26);
    assert_eq!(chart(run(&mut r, &cfg, "burndown.monthly").await.0).bars.len(), 24);
    assert_eq!(chart(run(&mut r, &cfg, "burndown.annual").await.0).bars.len(), 10);
    // `burndown.cumulative` is on by default; off, done shows only on the day it happened.
    let flat = parse("burndown.cumulative=off\n").config;
    assert_eq!(last(&chart(run(&mut r, &flat, "burndown.daily").await.0)), (1, 0, 1), "it was finished today");
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
        c.months.iter().flat_map(|m| m.weeks.iter().flat_map(|w| w.days.iter().flatten())).filter(|d| d.due.is_some()).count()
    };
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar").await.0)), 1);
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar project:Nowhere").await.0)), 0);
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar project:Work").await.0)), 1);
    assert_eq!(due_days(&cal(run(&mut r, &cfg, "calendar +NOSUCHTAG").await.0)), 0);
    // Filters and months go together, in any order.
    assert_eq!(cal(run(&mut r, &cfg, "calendar project:Work y").await.0).months.len(), 12);
    // The details report is narrowed the same way.
    let full = parse("calendar.details=full\n").config;
    let rows = |res: CliResult| cal(res).details.unwrap().rows.iter().map(|x| x.facts.description.clone()).collect::<Vec<_>>();
    assert_eq!(rows(run(&mut r, &full, "calendar project:Work").await.0), ["work thing"]);
    // A word that is neither a month argument nor filter-shaped is still a mistake.
    let (res, _) = run(&mut r, &cfg, "calendar Work").await;
    assert!(message(&res).contains("Could not recognize argument 'Work'"), "{}", message(&res));
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
        c.months.iter().flat_map(|m| m.weeks.iter().flat_map(|w| w.days.iter().flatten())).filter(|d| d.due.is_some()).count()
    };
    assert_eq!(due_days(&c), 1, "this month's, once: `nocal` is left off and 2031 is out of range");
    assert!(c.details.is_none(), "sparse is the default");

    let full = parse("calendar.details=full\n").config;
    let c = cal(run(&mut r, &full, "calendar").await.0);
    let d = c.details.expect("a report of what is due");
    assert_eq!(d.report, "list");
    assert_eq!(d.rows.iter().map(|x| x.facts.description.as_str()).collect::<Vec<_>>(), ["this month"]);
    let far = cal(run(&mut r, &full, "calendar 6 2031").await.0);
    assert_eq!(far.details.unwrap().rows.iter().map(|x| x.facts.description.as_str()).collect::<Vec<_>>(), ["far away"]);

    // Its report can be another one, and it has to exist.
    let long = parse("calendar.details=full\ncalendar.details.report=long\n").config;
    assert_eq!(cal(run(&mut r, &long, "calendar").await.0).details.unwrap().report, "long");
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

    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized").await else { panic!() };
    let order: Vec<&str> = o.rows.iter().map(|r| r.facts.description.as_str()).collect();
    assert_eq!(order, ["big thing", "small thing"]);
    assert_eq!(o.rows[0].facts.extra["points"], "8");
    assert_eq!(o.rows[1].facts.extra["points"], "1", "uda default applied on add");
    assert_eq!(o.description.as_deref(), Some("By size"));

    // Filtering on UDAs from the command line.
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized estimate:big").await else { panic!() };
    assert_eq!(o.rows.len(), 1);
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized points.after:5").await else { panic!() };
    assert_eq!(o.rows.len(), 1);
    // And modifying them.
    run(&mut r, &cfg, "estimate:small modify estimate:big").await;
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "sized estimate:big").await else { panic!() };
    assert_eq!(o.rows.len(), 2);
}

#[tokio::test]
async fn context_filters_reads_and_tags_new_tasks() {
    let cfg = parse("context.work.read=+work\ncontext.work.write=+work\ncontext=work\n").config;
    let mut r = replica();
    run(&mut r, &cfg, "add in context").await; // gets +work from the write rule
    let none = Config::default();
    run(&mut r, &none, "add outside context").await;
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "list").await else { panic!() };
    assert_eq!(o.rows.len(), 1);
    assert_eq!(o.rows[0].facts.description, "in context");
    let (CliResult::Report(o), _) = run(&mut r, &none, "list").await else { panic!() };
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

    let (CliResult::Table(t), _) = run(&mut r, &cfg, "projects").await else { panic!() };
    assert_eq!(t.rows, [vec!["(none)", "1"], vec!["Home", "2"]]);
    let (CliResult::Table(t), _) = run(&mut r, &cfg, "tags").await else { panic!() };
    assert_eq!(t.rows, [vec!["x", "2"], vec!["y", "1"]]);

    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "2 info").await else { panic!() };
    assert_eq!(tasks[0].facts.description, "b");
    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "2").await else { panic!() };
    assert_eq!(tasks.len(), 1, "a bare id means info");

    let (CliResult::Json { value }, _) = run(&mut r, &cfg, "project:Home export").await else { panic!() };
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
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "overdue").await else { panic!() };
    assert_eq!(o.rows.len(), 1);
    assert_eq!(o.rows[0].facts.description, "late");
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "list +DUE").await else { panic!() };
    assert_eq!(o.rows.len(), 1);
    assert_eq!(o.rows[0].facts.description, "soon");
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "next due.before:7d").await else { panic!() };
    assert_eq!(o.rows.len(), 2);
    // `next` sorts by urgency: the overdue one first.
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "next").await else { panic!() };
    assert_eq!(o.rows[0].facts.description, "late");
    let _ = DAY;
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
        let yes = Options { confirmed: true, ..Options::default() }; // `undo` asks first
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
    let (CliResult::Report(o), _) = run(&mut r, &cfg, "list").await else { panic!() };
    assert_eq!(o.rows[0].orphans, ["legacy"]);
    assert_eq!(o.rows[0].facts.extra["legacy"], "kept");
    let (CliResult::Info { tasks }, _) = run(&mut r, &cfg, "1 info").await else { panic!() };
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
            let d = execute(&mut rr, &cfgc, clock(), &split_words(line), Options::default(), &mut UndoStack::default()).await;
            d.command
        }
    };
    let c = info("project:Home +a work").await.unwrap();
    assert_eq!((c.name.as_str(), c.report, c.filter.clone()), ("work", true, vec!["project:Home".to_string(), "+a".into()]));
    let c = info("task 3 modify +x").await.unwrap();
    assert_eq!((c.name.as_str(), c.report, c.filter.clone()), ("modify", false, vec!["3".to_string()]));
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
            CliResult::Report(o) => (o.sort.clone(), o.rows.iter().map(|x| x.facts.description.clone()).collect::<Vec<_>>()),
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
        assert!(message(&res).contains("'nonsense' column is not a valid sort field"), "{}", message(&res));
        let (res, _) = run(&mut r, &cfg, "rc.report.list.sort:due list").await;
        assert!(message(&res).contains("must end in + or -"), "{}", message(&res));
        let (res, _) = run(&mut r, &cfg, "rc.report.nope.sort:due+ list").await;
        assert!(message(&res).contains("not a report"), "{}", message(&res));
        // Credentials can't be injected through the command line either.
        for line in ["rc.sync.encryption_secret:x list", "rc.sync.aws.bucket=b list", "rc.my.api_token:t list"] {
            let (res, _) = run(&mut r, &cfg, line).await;
            assert!(message(&res).contains("can't be set from a command line"), "{line}: {}", message(&res));
        }
        // Harmless settings the real task accepts are fine.
        for line in ["rc.verbose:nothing list", "rc.confirmation=no list"] {
            assert!(matches!(run(&mut r, &cfg, line).await.0, CliResult::Report(_)), "{line}");
        }
    }

    #[tokio::test]
    async fn rc_can_override_filters_columns_and_context() {
        let cfg = parse("context.w.read=+work\n").config;
        let mut r = replica();
        run(&mut r, &cfg, "add one +work").await;
        run(&mut r, &cfg, "add two").await;
        let rows = |res: CliResult| match res { CliResult::Report(o) => o.rows.len(), o => panic!("{o:?}") };
        assert_eq!(rows(run(&mut r, &cfg, "list").await.0), 2);
        assert_eq!(rows(run(&mut r, &cfg, "rc.context:w list").await.0), 1);
        assert_eq!(rows(run(&mut r, &cfg, "rc.report.list.filter:+work list").await.0), 1);
        let CliResult::Report(o) = run(&mut r, &cfg, "rc.report.list.columns:id,description rc.report.list.labels:N,T list").await.0 else { panic!() };
        assert_eq!(o.columns.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["N", "T"]);
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
        let cfg = parse("journal.time=on\njournal.time.start.annotation=Clock in\njournal.time.stop.annotation=Clock out\n").config;
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
        assert!(notes(&only(&mut r).await).is_empty(), "journal.time is off unless enabled");
    }

    fn history_of(res: CliResult) -> Vec<(String, String)> {
        let CliResult::Info { tasks } = res else { panic!("{res:?}") };
        tasks[0]
            .history
            .iter()
            .flat_map(|e| e.changes.iter().map(|c| (c.kind.to_owned(), c.prop.clone())))
            .collect()
    }

    #[tokio::test]
    async fn info_lists_what_changed_unless_journal_info_is_off() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add Alpha project:home +x priority:H").await;
        run(&mut r, &cfg, "1 modify project:work -x +y").await;
        run(&mut r, &cfg, "1 annotate a note").await;
        run(&mut r, &cfg, "1 start").await;
        run(&mut r, &cfg, "1 done").await;
        let id = only(&mut r).await.uuid.to_string();

        let h = history_of(run(&mut r, &cfg, &format!("{id} info")).await.0);
        let has = |k: &str, p: &str| h.iter().any(|(kk, pp)| kk == k && pp == p);
        assert!(has("set", "description") && has("set", "project") && has("set", "priority"), "{h:?}");
        assert!(has("tag_added", "x") && has("tag_added", "y") && has("tag_deleted", "x"), "{h:?}");
        assert!(has("changed", "project"), "{h:?}");
        assert!(h.iter().any(|(k, p)| k == "note_added" && p.starts_with("annotation_")), "{h:?}");
        assert!(has("set", "start") && has("deleted", "start") && has("changed", "status"), "{h:?}");
        assert!(!h.iter().any(|(_, p)| p == "modified"), "the modification time is never listed");

        // A Taskwarrior config says `journal.info=off` or `0`; the web replica follows.
        for off in ["journal.info=off\n", "journal.info=0\n"] {
            let h = history_of(run(&mut r, &parse(off).config, &format!("{id} info")).await.0);
            assert!(h.is_empty(), "{off}: {h:?}");
        }
        // The command line can switch it too, like any setting.
        let h = history_of(run(&mut r, &cfg, &format!("rc.journal.info:off {id} info")).await.0);
        assert!(h.is_empty());
    }

    #[tokio::test]
    async fn the_history_stays_out_of_reports_and_exports() {
        let cfg = Config::default();
        let mut r = replica();
        run(&mut r, &cfg, "add Alpha").await;
        let (res, _) = run(&mut r, &cfg, "export").await;
        let CliResult::Json { value } = res else { panic!("{res:?}") };
        assert!(value[0].get("history").is_none(), "{value}");
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
        let CliResult::Info { tasks } = run(&mut r, &on, "1 info").await.0 else { panic!() };
        assert!(tasks[0].active_seconds.is_some(), "an active task reports its tracked time");
        let CliResult::Info { tasks } = run(&mut r, &Config::default(), "1 info").await.0 else { panic!() };
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
    let CliResult::Info { tasks } = run(&mut r, &cfg, "1 info").await.0 else { panic!() };
    let s = &tasks[0].sessions;
    assert_eq!(s.len(), 2, "{s:?}");
    assert!(s[0].end.is_some() && s[1].end.is_none(), "the second session is still running: {s:?}");
    assert!(s[0].start <= s[0].end.unwrap());
    // Off by default: no sessions without journal.time.
    let CliResult::Info { tasks } = run(&mut r, &Config::default(), "1 info").await.0 else { panic!() };
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
        assert!(v.iter().all(|f| f.status == "deleted"), "{:?}", v.iter().map(|f| &f.status).collect::<Vec<_>>());
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
        assert!(v.iter().all(|f| f.status == "deleted"), "{:?}", v.iter().map(|f| &f.status).collect::<Vec<_>>());
    }

    // ---- recurrence.confirmation

    /// A template with two open instances, `recurrence.confirmation` set to `mode`.
    async fn series(mode: &str) -> (R, Config) {
        let cfg = parse(&format!("recurrence=on\nrecurrence.limit=2\nrecurrence.confirmation={mode}\n")).config;
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
        Options { extras: Some(if yes { items.iter().map(|i| i.key.clone()).collect() } else { vec![] }), ..Options::default() }
    }

    #[tokio::test]
    async fn by_default_editing_a_recurring_task_asks_and_changes_nothing_until_answered() {
        let (mut r, cfg) = series("prompt").await;
        let before = descriptions(&mut r).await;
        let (res, wrote) = run(&mut r, &cfg, "1 modify Feed plants").await;
        assert!(!wrote);
        let (ask, items) = asked(&res);
        assert!(ask == Ask::Extras && items[0].question.contains("pending recurrences"), "{res:?}");
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
        assert!(all(&mut r).await.iter().all(|f| f.description == "Feed plants"), "{:?}", descriptions(&mut r).await);

        let (mut r, cfg) = series("prompt").await;
        let (asked_res, _) = run(&mut r, &cfg, "1 modify Feed plants").await;
        let (_, wrote) = run_opts(&mut r, &cfg, "1 modify Feed plants", answer(&asked_res, false)).await;
        assert!(wrote);
        let v = all(&mut r).await;
        assert_eq!(template(&v).description, "Feed plants", "the edited task itself always changes");
        assert!(instances(&v).iter().all(|f| f.description == "Water plants"), "{:?}", descriptions(&mut r).await);
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
        assert!(v.iter().all(|f| f.priority.as_deref() == Some("H") && f.project.as_deref() == Some("Garden")), "{v:?}");

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

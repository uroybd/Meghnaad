//! Real command lines against a real taskchampion `Replica` (in-memory storage).

use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::Replica;
use tc_core::cli::{execute, load_facts, CliResult, Options, UndoStack};
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
    let (res, _) = run(&mut r, &cfg, "1 delete").await;
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
        run_opts(&mut r, &cfg, "project:P done", Options { confirmed: true, seed: 0 }).await;
    assert!(wrote);
    assert_eq!(message(&res), "Completed 3 tasks.");
}

#[tokio::test]
async fn empty_filter_writes_are_refused() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add a").await;
    for line in ["done", "delete", "modify +x"] {
        let (res, wrote) = run(&mut r, &cfg, line).await;
        assert!(!wrote && message(&res).contains("no tasks specified"), "{line}: {}", message(&res));
    }
}

#[tokio::test]
async fn undo_reverses_the_last_change() {
    let mut r = replica();
    let cfg = Config::default();
    run(&mut r, &cfg, "add keep me").await;
    run(&mut r, &cfg, "1 done").await;
    assert_eq!(load_facts(&mut r).await.unwrap()[0].status, "completed");
    let (res, wrote) = run(&mut r, &cfg, "undo").await;
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
        ("1 modify recur:weekly", "recurrence"),
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

        let (res, wrote) = run(&mut a, &cfg, "undo").await;
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
        let d = execute(r, cfg, clock(), &split_words(line), Options::default(), undo).await;
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
        run(&mut r, &cfg, "undo").await;
        let f = &load_facts(&mut r).await.unwrap()[0];
        assert!(f.tags.contains("a") && !f.tags.contains("b"));
        run(&mut r, &cfg, "undo").await;
        assert!(load_facts(&mut r).await.unwrap()[0].tags.is_empty());
        run(&mut r, &cfg, "undo").await; // undoes the add itself
        let f = load_facts(&mut r).await.unwrap();
        assert!(f.is_empty() || f[0].status == "deleted", "{f:?}");
        let (res, wrote) = run(&mut r, &cfg, "undo").await;
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

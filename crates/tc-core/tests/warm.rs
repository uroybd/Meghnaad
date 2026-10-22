//! The cache of the tasks as the engine reads them (`tc_core::warm`) must never show anything but what a fresh read
//! would. Four ways of looking: a long random run of commands and writes with the answer compared to a rebuild after
//! every step; a replica that pulls what another one pushed; the change counter itself (reads, writes, abandoned
//! transactions, a sync with nothing to do); and the same questions asked with and without the cache.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use taskchampion::chrono::{TimeZone, Utc};
use taskchampion::server::SnapshotUrgency;
use taskchampion::storage::Storage;
use taskchampion::{Operations, Replica, Status};
use tc_core::cli::{execute, load_facts, warm_facts, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::taskrc::Config;
use tc_core::warm::Warm;
use tc_core::{load_cryptor, CloudServer, LiveStorage, MemStore};
use uuid::Uuid;

type R = Replica<LiveStorage>;

/// A small deterministic generator, so a failure repeats.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn replica() -> (R, Warm) {
    let storage = LiveStorage::new();
    let warm = Warm::new(storage.generation());
    (Replica::new(storage), warm)
}

async fn run(r: &mut R, warm: Option<&Warm>, line: &str, step: i64) -> tc_core::cli::Done {
    let mut undo = UndoStack::default();
    let opts = Options {
        confirmed: true,
        warm: warm.cloned(),
        ..Options::default()
    };
    execute(
        r,
        &Config::default(),
        Clock::utc(1_790_000_000 + step * 3600),
        &split_words(line),
        opts,
        &mut undo,
    )
    .await
}

/// What a fresh read gives, against what the cache gives.
async fn assert_same(r: &mut R, warm: &Warm, when: &str) {
    let cached = warm_facts(r, Some(warm)).await.unwrap();
    let fresh = load_facts(r).await.unwrap();
    assert_eq!(*cached, fresh, "the cache is stale {when}");
}

#[tokio::test(flavor = "current_thread")]
async fn the_cache_is_never_stale_over_a_long_random_run() {
    for seed in [1u64, 7, 99, 12345] {
        let (mut r, warm) = replica();
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for step in 0..260i64 {
            let id = 1 + rng.below(12);
            let other = 1 + rng.below(12);
            let line = match rng.below(20) {
                0..=2 => format!(
                    "add Task {step} project:P{} +t{} due:{}d",
                    rng.below(3),
                    rng.below(3),
                    1 + rng.below(9)
                ),
                3 => format!("add Daily {step} recur:daily due:today"),
                4 => format!("add Waiting {step} wait:{}d", 1 + rng.below(5)),
                5 => format!("{id} modify project:Q{} +m{}", rng.below(3), rng.below(3)),
                6 => format!("{id} done"),
                7 => format!("{id} delete"),
                8 => format!("{id} start"),
                9 => format!("{id} stop"),
                10 => format!("{id} annotate note {step}"),
                11 => format!("{id} modify depends:{other}"),
                12 => "undo".to_owned(),
                13 => format!("{id} modify priority:H due:{}d", 1 + rng.below(5)),
                // Reads, which fill the cache that the next write has to invalidate.
                14 => "next".to_owned(),
                15 => "count".to_owned(),
                16 => "_tags".to_owned(),
                17 => format!("{id} info"),
                18 => "all".to_owned(),
                _ => "summary".to_owned(),
            };
            run(&mut r, Some(&warm), &line, step).await;
            assert_same(&mut r, &warm, &format!("after `{line}` (seed {seed}, step {step})")).await;
            // Something that bypasses the engine entirely: a task changed through the replica, as a sync does.
            if rng.below(15) == 0 {
                let mut ops = Operations::new();
                let mut t = r
                    .create_task(Uuid::from_u128(1_000 + step as u128), &mut ops)
                    .await
                    .unwrap();
                t.set_status(Status::Pending, &mut ops).unwrap();
                t.set_description(format!("direct {step}"), &mut ops).unwrap();
                r.commit_operations(ops).await.unwrap();
                assert_same(
                    &mut r,
                    &warm,
                    &format!("after a direct write (seed {seed}, step {step})"),
                )
                .await;
            }
        }
        // Most writes touch no dependency, and for those the list is brought up to date, not built again.
        assert!(warm.patched() > 100, "seed {seed}: only {} patches", warm.patched());
    }
}

/// A pending task made through the replica alone, as a sync would.
async fn direct_add(r: &mut R, n: u128) {
    let mut ops = Operations::new();
    let mut t = r.create_task(Uuid::from_u128(n), &mut ops).await.unwrap();
    t.set_status(Status::Pending, &mut ops).unwrap();
    t.set_description(format!("direct {n}"), &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn a_few_writes_between_reads_are_patched_and_many_are_built_again() {
    let (mut r, warm) = replica();
    for i in 0..20 {
        run(&mut r, Some(&warm), &format!("add Task {i} project:P{}", i % 3), i).await;
    }
    assert_same(&mut r, &warm, "after the first adds").await;
    let before = warm.patched();
    // Each write reads its own result, so each is a patch.
    for (i, line) in ["1 done", "2 modify +x", "add Another"].iter().enumerate() {
        run(&mut r, Some(&warm), line, 100 + i as i64).await;
    }
    assert_same(&mut r, &warm, "after three writes").await;
    assert_eq!(warm.patched(), before + 3);
    // Writes that nothing reads (a sync does this): a few are one patch ...
    let before = warm.patched();
    for i in 0..5 {
        direct_add(&mut r, 5_000 + i).await;
    }
    assert_same(&mut r, &warm, "after five direct writes").await;
    assert_eq!(warm.patched(), before + 1);
    // ... and more transactions than the generation remembers are built again, and still right.
    let before = warm.patched();
    for i in 0..80 {
        direct_add(&mut r, 6_000 + i).await;
    }
    assert_same(&mut r, &warm, "after a long run of writes").await;
    assert_eq!(warm.patched(), before, "too many changes to patch");
    let before = warm.patched();
    // A dependency is never patched around.
    run(&mut r, Some(&warm), "3 modify depends:4", 400).await;
    assert_same(&mut r, &warm, "after a dependency was added").await;
    assert_eq!(warm.patched(), before);
    run(&mut r, Some(&warm), "4 done", 401).await;
    assert_same(&mut r, &warm, "after what 3 waited for was done").await;
    assert_eq!(warm.patched(), before);
    run(&mut r, Some(&warm), "5 done", 402).await;
    assert_same(&mut r, &warm, "after an unrelated task").await;
    assert_eq!(warm.patched(), before + 1);
}

#[tokio::test(flavor = "current_thread")]
async fn what_another_replica_pushed_is_seen_after_a_pull() {
    let store = MemStore::new();
    let c = load_cryptor(&store, b"secret").await.unwrap();
    let server = |u| -> Box<dyn taskchampion::Server> {
        Box::new(CloudServer::with_cryptor(store.clone(), c.clone()).with_snapshot_urgency(u))
    };
    let (mut a, _) = replica();
    let (mut b, warm_b) = replica();

    run(&mut a, None, "add first project:Home", 0).await;
    a.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
    b.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
    assert_same(&mut b, &warm_b, "after the first pull").await;
    assert_eq!(warm_facts(&mut b, Some(&warm_b)).await.unwrap().len(), 1);

    // A adds one, changes the first and deletes nothing; B has the old list cached until it pulls.
    run(&mut a, None, "add second +x", 1).await;
    run(&mut a, None, "1 modify project:Work +y", 2).await;
    a.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
    assert_eq!(
        warm_facts(&mut b, Some(&warm_b)).await.unwrap().len(),
        1,
        "nothing is pulled before it is asked"
    );
    b.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
    let seen = warm_facts(&mut b, Some(&warm_b)).await.unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen
        .iter()
        .any(|f| f.project.as_deref() == Some("Work") && f.tags.contains("y")));
    assert_same(&mut b, &warm_b, "after the second pull").await;

    run(&mut a, None, "2 delete", 3).await;
    a.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
    b.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
    assert_same(&mut b, &warm_b, "after a deletion was pulled").await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_counter_moves_on_writes_and_abandoned_writes_and_stays_for_reads() {
    let mut s = LiveStorage::new();
    let g = s.generation();
    let id = Uuid::from_u128(1);
    let start = g.get();

    {
        let mut t = s.txn().await.unwrap();
        assert!(t.get_task(id).await.unwrap().is_none());
        t.commit().await.unwrap();
    }
    assert_eq!(g.get(), start, "a read changes nothing");

    {
        let mut t = s.txn().await.unwrap();
        t.commit().await.unwrap();
    }
    assert_eq!(g.get(), start, "a transaction that did nothing changes nothing");

    {
        let mut t = s.txn().await.unwrap();
        t.create_task(id).await.unwrap();
        t.commit().await.unwrap();
    }
    let after_write = g.get();
    assert_ne!(after_write, start, "a committed write is a change");

    {
        let mut t = s.txn().await.unwrap();
        t.set_task(id, [("status".to_owned(), "pending".to_owned())].into_iter().collect())
            .await
            .unwrap();
        // Dropped without commit: taken back. A reader between the two would have seen something else.
    }
    assert_ne!(
        g.get(),
        after_write,
        "a write that was taken back still moves the counter"
    );
    let mut t = s.txn().await.unwrap();
    assert_eq!(t.get_task(id).await.unwrap().unwrap().len(), 0, "and it was taken back");
}

#[tokio::test(flavor = "current_thread")]
async fn a_sync_with_nothing_to_do_leaves_the_cache_alone() {
    let store = MemStore::new();
    let c = load_cryptor(&store, b"secret").await.unwrap();
    let server = || -> Box<dyn taskchampion::Server> {
        Box::new(CloudServer::with_cryptor(store.clone(), c.clone()).with_snapshot_urgency(SnapshotUrgency::None))
    };
    let (mut r, warm) = replica();
    for i in 0..5 {
        run(&mut r, Some(&warm), &format!("add task {i}"), i).await;
    }
    r.sync(&mut server(), false).await.unwrap();
    warm_facts(&mut r, Some(&warm)).await.unwrap();
    let before = warm.generation();
    // What every request of the Worker does first: look for news, and find none.
    for _ in 0..3 {
        r.sync(&mut server(), false).await.unwrap();
    }
    assert_eq!(warm.generation(), before, "an idle sync must not throw the cache away");
    assert!(warm.get().is_some());
}

/// The same data in two replicas (fixed uuids and times), one asked with the cache and one without.
async fn world(n: usize) -> (R, Warm) {
    let (mut r, warm) = replica();
    let mut ops = Operations::new();
    for i in 0..n {
        let mut t = r.create_task(Uuid::from_u128(i as u128 + 1), &mut ops).await.unwrap();
        t.set_status(if i % 6 == 0 { Status::Completed } else { Status::Pending }, &mut ops)
            .unwrap();
        t.set_description(format!("Task number {i}"), &mut ops).unwrap();
        t.set_value("project", Some(format!("Work.Area{}", i % 4)), &mut ops)
            .unwrap();
        t.set_value("priority", Some(["H", "M", "L"][i % 3].into()), &mut ops)
            .unwrap();
        t.add_tag(&"next".parse().unwrap(), &mut ops).unwrap();
        t.set_entry(
            Some(Utc.timestamp_opt(1_780_000_000 + i as i64 * 60, 0).unwrap()),
            &mut ops,
        )
        .unwrap();
        t.set_value("due", Some((1_790_100_000 + i as i64 * 3600).to_string()), &mut ops)
            .unwrap();
        if i > 0 && i % 5 == 0 {
            t.add_dependency(Uuid::from_u128(i as u128), &mut ops).unwrap();
        }
    }
    r.commit_operations(ops).await.unwrap();
    (r, warm)
}

#[tokio::test(flavor = "current_thread")]
async fn the_same_questions_get_the_same_answers_with_and_without_the_cache() {
    let (mut with, warm) = world(60).await;
    let (mut without, _) = world(60).await;
    for round in 0..2 {
        for (i, line) in [
            "next",
            "list",
            "all",
            "count",
            "summary",
            "projects",
            "_tags",
            "_projects",
            "_rows",
            "export",
            "1 info",
            "5 info",
            "burndown.daily",
            "+BLOCKED list",
        ]
        .iter()
        .enumerate()
        {
            let a = serde_json::to_string(&run(&mut with, Some(&warm), line, 0).await.result).unwrap();
            let b = serde_json::to_string(&run(&mut without, None, line, 0).await.result).unwrap();
            assert_eq!(a, b, "`{line}` differs with the cache (round {round}, #{i})");
        }
    }
    // Written the same way to both, the answers still agree, and the cache was rebuilt.
    for line in ["3 modify +changed", "7 done", "add One more", "4 delete"] {
        run(&mut with, Some(&warm), line, 1).await;
        run(&mut without, None, line, 1).await;
    }
    let a = load_facts(&mut with).await.unwrap().len();
    assert_eq!(warm_facts(&mut with, Some(&warm)).await.unwrap().len(), a);
    assert_eq!(a, load_facts(&mut without).await.unwrap().len());
}

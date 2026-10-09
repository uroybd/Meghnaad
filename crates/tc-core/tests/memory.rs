//! How much memory a Worker needs. A Cloudflare isolate has 128 MB, and a WebAssembly heap never
//! gives memory back, so what matters is the *peak*: of catching up with the bucket (cold start)
//! and of answering a request. This test measures both with a counting allocator, and holds them
//! under budgets, so a change that makes either much hungrier fails here before it fails in prod.
//!
//! The heap measured is Rust's; the JavaScript side (R2 bodies on their way in) adds a copy of
//! each body, which the budgets leave room for.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use taskchampion::server::SnapshotUrgency;
use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::{Operations, Replica, Status};
use tc_core::cli::{execute, CliResult, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::taskrc::Config;
use tc_core::{load_cryptor, CloudServer, MemStore};

struct Counting;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let now = CURRENT.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(now, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CURRENT.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = System.realloc(p, l, new);
        if !q.is_null() {
            if new >= l.size() {
                let now = CURRENT.fetch_add(new - l.size(), Relaxed) + (new - l.size());
                PEAK.fetch_max(now, Relaxed);
            } else {
                CURRENT.fetch_sub(l.size() - new, Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const MB: f64 = 1024.0 * 1024.0;

/// Run `f` and report (peak above where it started, still held at the end), in MB.
async fn measure<T, F: std::future::Future<Output = T>>(f: F) -> (f64, f64, T) {
    let base = CURRENT.load(Relaxed);
    PEAK.store(base, Relaxed);
    let out = f.await;
    let peak = PEAK.load(Relaxed).saturating_sub(base);
    let held = CURRENT.load(Relaxed).saturating_sub(base);
    (peak as f64 / MB, held as f64 / MB, out)
}

type R = Replica<InMemoryStorage>;

async fn add_tasks(r: &mut R, from: usize, n: usize) {
    let mut ops = Operations::new();
    for i in from..from + n {
        let mut t = r.create_task(uuid::Uuid::new_v4(), &mut ops).await.unwrap();
        t.set_status(Status::Pending, &mut ops).unwrap();
        t.set_description(
            format!("Task number {i}: write the quarterly report for the team"),
            &mut ops,
        )
        .unwrap();
        t.set_value("project", Some(format!("Work.Area{}", i % 7)), &mut ops)
            .unwrap();
        t.set_value("priority", Some(["H", "M", "L"][i % 3].into()), &mut ops)
            .unwrap();
        t.add_tag(&"next".parse().unwrap(), &mut ops).unwrap();
        t.set_value("due", Some((1_800_000_000 + i as i64 * 3600).to_string()), &mut ops)
            .unwrap();
        t.add_annotation(
            taskchampion::Annotation {
                entry: taskchampion::chrono::Utc::now(),
                description: "a note about it".into(),
            },
            &mut ops,
        )
        .unwrap();
    }
    r.commit_operations(ops).await.unwrap();
}

fn server(store: &MemStore, c: &tc_core::crypto::Cryptor, urgency: SnapshotUrgency) -> Box<dyn taskchampion::Server> {
    Box::new(CloudServer::with_cryptor(store.clone(), c.clone()).with_snapshot_urgency(urgency))
}

async fn bucket() -> (MemStore, tc_core::crypto::Cryptor) {
    let store = MemStore::new();
    let c = load_cryptor(&store, b"secret").await.unwrap();
    (store, c)
}

fn clock() -> Clock {
    Clock::utc(1_790_000_000)
}

async fn run(r: &mut R, line: &str) -> CliResult {
    let mut undo = UndoStack::default();
    execute(
        r,
        &Config::default(),
        clock(),
        &split_words(line),
        Options::default(),
        &mut undo,
    )
    .await
    .result
}

#[tokio::test(flavor = "current_thread")]
async fn catching_up_from_scratch_over_many_versions_stays_small() {
    let (store, c) = bucket().await;
    let mut a: R = Replica::new(InMemoryStorage::new());
    // 1,200 versions of one change each, the way a busy week of edits from several devices looks.
    for i in 0..1200 {
        add_tasks(&mut a, i, 1).await;
        let mut s = server(&store, &c, SnapshotUrgency::None);
        a.sync(&mut s, true).await.unwrap();
    }
    let mut b: R = Replica::new(InMemoryStorage::new());
    let mut s = server(&store, &c, SnapshotUrgency::None);
    let (peak, held, _) = measure(async { b.sync(&mut s, true).await.unwrap() }).await;
    println!("cold start, 1200 versions: peak {peak:.1} MB, held afterwards {held:.1} MB");
    assert!(peak < 40.0, "a cold start took {peak:.1} MB");
}

#[tokio::test(flavor = "current_thread")]
async fn restoring_a_snapshot_and_answering_requests_stay_small() {
    let (store, c) = bucket().await;
    let mut a: R = Replica::new(InMemoryStorage::new());
    for batch in 0..6 {
        add_tasks(&mut a, batch * 500, 500).await;
        let mut s = server(&store, &c, SnapshotUrgency::None);
        a.sync(&mut s, true).await.unwrap();
    }
    // The last push writes a snapshot, as the CLI and the Worker do now and then.
    add_tasks(&mut a, 3000, 1).await;
    let mut s = server(&store, &c, SnapshotUrgency::High);
    a.sync(&mut s, false).await.unwrap();

    let mut b: R = Replica::new(InMemoryStorage::new());
    let mut s = server(&store, &c, SnapshotUrgency::None);
    let (peak, held, _) = measure(async { b.sync(&mut s, true).await.unwrap() }).await;
    println!("cold start from a snapshot of 3001 tasks: peak {peak:.1} MB, held {held:.1} MB");
    assert!(peak < 60.0, "restoring a snapshot took {peak:.1} MB");

    for line in ["next", "list", "all", "export", "summary", "burndown.daily", "projects"] {
        let (peak, held, res) = measure(async {
            let r = run(&mut b, line).await;
            // What the Worker does with it: serialise it for the response.
            serde_json::to_vec(&r).unwrap().len()
        })
        .await;
        println!(
            "`{line}` over 3001 tasks: peak {peak:.1} MB (held {held:.2} MB), response {:.1} MB",
            res as f64 / MB
        );
        assert!(peak < 60.0, "`{line}` took {peak:.1} MB");
    }
}

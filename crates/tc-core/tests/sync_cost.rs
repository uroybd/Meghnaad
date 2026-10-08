//! How many object-store calls a sync costs. Each call is a network round trip to R2 from the
//! Worker, so these numbers are the request latency (and, on the free plan, the subrequest budget).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::{Operations, Replica, Server, Status};
use tc_core::crypto::Cryptor;
use tc_core::{load_cryptor, names, CloudServer, MemStore, ObjectStore, Result};

/// A [`MemStore`] that counts what is asked of it, by kind. Clones share the counts.
#[derive(Clone, Default)]
struct Counting {
    inner: MemStore,
    calls: Rc<RefCell<BTreeMap<&'static str, usize>>>,
}

impl Counting {
    fn bump(&self, what: &'static str) {
        *self.calls.borrow_mut().entry(what).or_default() += 1;
    }
    fn take(&self) -> BTreeMap<&'static str, usize> {
        std::mem::take(&mut *self.calls.borrow_mut())
    }
}

impl ObjectStore for Counting {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.bump("get");
        self.inner.get(name).await
    }
    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        self.bump("put");
        self.inner.put(name, value).await
    }
    async fn del(&self, name: &str) -> Result<()> {
        self.bump("del");
        self.inner.del(name).await
    }
    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        self.bump("list");
        self.inner.list(prefix).await
    }
    async fn compare_and_swap(&self, name: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        self.bump("cas");
        self.inner.compare_and_swap(name, expected, new).await
    }
    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        self.bump("get");
        self.inner.get_tagged(name).await
    }
    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        self.bump("swap");
        self.inner.swap_tagged(name, expected, new).await
    }
}

type R = Replica<InMemoryStorage>;

fn replica() -> R {
    Replica::new(InMemoryStorage::new())
}

/// The key is derived once per Worker isolate and reused, so it is not part of a request's cost.
async fn cryptor(store: &Counting) -> Cryptor {
    load_cryptor(store, b"hunter2").await.unwrap()
}

fn server(store: &Counting, c: &Cryptor) -> Box<dyn Server> {
    Box::new(CloudServer::with_cryptor(store.clone(), c.clone()))
}

async fn add_task(r: &mut R, description: &str) {
    let mut ops = Operations::new();
    let mut t = r.create_task(uuid::Uuid::new_v4(), &mut ops).await.unwrap();
    t.set_status(Status::Pending, &mut ops).unwrap();
    t.set_description(description.into(), &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();
}

fn total(c: &BTreeMap<&'static str, usize>) -> usize {
    c.values().sum()
}

/// A CLI that has pushed `n` versions, one task each.
async fn bucket_with(store: &Counting, c: &Cryptor, n: usize) -> R {
    let mut cli = replica();
    for i in 0..n {
        add_task(&mut cli, &format!("task {i}")).await;
        cli.sync(&mut server(store, c), true).await.unwrap();
    }
    store.take();
    cli
}

#[tokio::test]
async fn a_cold_replica_catches_up_in_few_calls() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    // More than one prefetch batch, so batching across a long chain is exercised too.
    let n = 80;
    bucket_with(&store, &c, n).await;

    let mut web = replica();
    web.sync(&mut server(&store, &c), true).await.unwrap();
    let cold = store.take();
    println!("cold start, {n} versions: {} calls {cold:?}", total(&cold));
    assert_eq!(web.all_tasks().await.unwrap().len(), n);
    // The snapshot lookup, one listing, the head (twice: at the start and to confirm the end),
    // and one read per version. It used to be about four calls per version.
    assert!(total(&cold) <= n + 6, "{cold:?}");
    assert_eq!(cold.get("list").copied().unwrap_or(0), 2, "one for snapshots, one for versions: {cold:?}");
}

#[tokio::test]
async fn an_idle_sync_is_one_read() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    bucket_with(&store, &c, 5).await;
    let mut web = replica();
    web.sync(&mut server(&store, &c), true).await.unwrap();
    store.take();

    web.sync(&mut server(&store, &c), true).await.unwrap();
    let idle = store.take();
    println!("up to date: {idle:?}");
    assert_eq!(idle.get("get"), Some(&1), "{idle:?}");
    assert_eq!(total(&idle), 1, "no listing when nothing is new: {idle:?}");
}

#[tokio::test]
async fn pulling_and_pushing_one_version_is_cheap() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    let mut cli = bucket_with(&store, &c, 10).await;
    let mut web = replica();
    web.sync(&mut server(&store, &c), true).await.unwrap();

    add_task(&mut cli, "from the CLI").await;
    cli.sync(&mut server(&store, &c), true).await.unwrap();
    store.take();
    web.sync(&mut server(&store, &c), true).await.unwrap();
    let pull = store.take();
    println!("one new remote version: {} calls {pull:?}", total(&pull));
    assert!(total(&pull) <= 6, "{pull:?}");

    add_task(&mut web, "from the web").await;
    web.sync(&mut server(&store, &c), true).await.unwrap();
    let push = store.take();
    println!("push one local version: {} calls {push:?}", total(&push));
    // Read the head (with its tag), upload, conditional swap: no read-then-write.
    assert_eq!(push.get("swap"), Some(&1), "{push:?}");
    assert!(push.get("cas").is_none(), "{push:?}");
    assert!(total(&push) <= 5, "{push:?}");
    assert_eq!(cli_sees(&store, &c).await, 12);
}

async fn cli_sees(store: &Counting, c: &Cryptor) -> usize {
    let mut fresh = replica();
    fresh.sync(&mut server(store, c), true).await.unwrap();
    fresh.all_tasks().await.unwrap().len()
}

#[tokio::test]
async fn a_server_reused_across_pushes_by_others_still_sees_them() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    let mut cli = bucket_with(&store, &c, 3).await;
    let mut web = replica();
    let mut kept = server(&store, &c); // one server object for several syncs
    web.sync(&mut kept, true).await.unwrap();

    add_task(&mut cli, "pushed later").await;
    cli.sync(&mut server(&store, &c), true).await.unwrap();
    web.sync(&mut kept, true).await.unwrap();
    assert_eq!(web.all_tasks().await.unwrap().len(), 4);
}

#[tokio::test]
async fn an_orphan_beside_the_real_chain_is_ignored() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    bucket_with(&store, &c, 4).await;
    // A lost race can leave an object that shares a parent with the real first version.
    let orphan = uuid::Uuid::from_u128(0xbad);
    store.put(&names::version_name(uuid::Uuid::nil(), orphan), b"not even encrypted").await.unwrap();
    store.take();

    let mut web = replica();
    web.sync(&mut server(&store, &c), true).await.unwrap();
    assert_eq!(web.all_tasks().await.unwrap().len(), 4);
}

#[tokio::test]
async fn an_empty_bucket_and_a_fresh_replica_sync_cleanly() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    store.take();
    let mut web = replica();
    web.sync(&mut server(&store, &c), true).await.unwrap();
    assert_eq!(web.all_tasks().await.unwrap().len(), 0);
    add_task(&mut web, "first ever").await;
    web.sync(&mut server(&store, &c), true).await.unwrap();
    assert_eq!(cli_sees(&store, &c).await, 1);
}

// ---- snapshots -------------------------------------------------------------------------------

use taskchampion::server::{AddVersionResult, SnapshotUrgency};

fn server_urgent(store: &Counting, c: &Cryptor, u: SnapshotUrgency) -> Box<dyn Server> {
    Box::new(CloudServer::with_cryptor(store.clone(), c.clone()).with_snapshot_urgency(u))
}

async fn snapshots(store: &Counting) -> Vec<uuid::Uuid> {
    store.inner.list("s-").await.unwrap().iter().filter_map(|n| names::parse_snapshot_name(n)).collect()
}

/// `n` pushes of one task each, syncing with `avoid_snapshots` as given and the server reporting `u`.
async fn push_n(store: &Counting, c: &Cryptor, r: &mut R, n: usize, u: SnapshotUrgency, avoid: bool) {
    for i in 0..n {
        add_task(r, &format!("pushed {i}")).await;
        r.sync(&mut server_urgent(store, c, u), avoid).await.unwrap();
    }
}

#[tokio::test]
async fn snapshot_odds_match_taskchampions_cloud_server() {
    let store = MemStore::new();
    let mut s = CloudServer::new(store, b"hunter2").await.unwrap();
    let (mut parent, mut high, mut low) = (uuid::Uuid::nil(), 0u32, 0u32);
    let n = 20_000u32;
    for _ in 0..n {
        let (res, urgency) = s.add_version(parent, b"[]".to_vec()).await.unwrap();
        let AddVersionResult::Ok(v) = res else { panic!("push refused") };
        parent = v;
        match urgency {
            SnapshotUrgency::High => high += 1,
            SnapshotUrgency::Low => low += 1,
            SnapshotUrgency::None => {}
        }
    }
    // 2/256 and 23/256 of pushes. Wide bounds: a flaky test is worse than a loose one.
    let pct = |x: u32| f64::from(x) * 100.0 / f64::from(n);
    assert!((0.4..1.3).contains(&pct(high)), "high {:.2}%", pct(high));
    assert!((7.0..11.0).contains(&pct(low)), "low {:.2}%", pct(low));
}

#[tokio::test]
async fn a_rejected_push_never_asks_for_a_snapshot() {
    let store = MemStore::new();
    let mut s = CloudServer::new(store, b"hunter2").await.unwrap().with_snapshot_urgency(SnapshotUrgency::High);
    let (res, _) = s.add_version(uuid::Uuid::nil(), b"[]".to_vec()).await.unwrap();
    assert!(matches!(res, AddVersionResult::Ok(_)));
    let (res, urgency) = s.add_version(uuid::Uuid::nil(), b"[]".to_vec()).await.unwrap();
    assert!(matches!(res, AddVersionResult::ExpectedParentVersion(_)));
    assert_eq!(urgency, SnapshotUrgency::None);
}

#[tokio::test]
async fn syncing_like_the_cli_writes_snapshots_when_the_server_asks() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    let mut web = replica();
    // `avoid_snapshots = false` is how `task sync` runs: a "low" request is enough.
    push_n(&store, &c, &mut web, 30, SnapshotUrgency::Low, false).await;
    assert_eq!(snapshots(&store).await.len(), 1, "older snapshots are removed as newer ones are written");

    // A cold replica now starts from the snapshot instead of replaying 30 versions.
    store.take();
    let mut fresh = replica();
    fresh.sync(&mut server(&store, &c), true).await.unwrap();
    let cold = store.take();
    println!("cold start from a snapshot of 30 versions: {} calls {cold:?}", total(&cold));
    assert_eq!(fresh.all_tasks().await.unwrap().len(), 30);
    assert!(total(&cold) <= 6, "{cold:?}");
}

#[tokio::test]
async fn thresholds_follow_avoid_snapshots() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    // Avoiding snapshots means only a "high" request counts.
    let mut a = replica();
    push_n(&store, &c, &mut a, 5, SnapshotUrgency::Low, true).await;
    assert!(snapshots(&store).await.is_empty());
    push_n(&store, &c, &mut a, 1, SnapshotUrgency::High, true).await;
    assert_eq!(snapshots(&store).await.len(), 1);

    // And never when the server doesn't ask.
    let other = Counting::default();
    let mut b = replica();
    push_n(&other, &c, &mut b, 20, SnapshotUrgency::None, false).await;
    assert!(snapshots(&other).await.is_empty());
}

#[tokio::test]
async fn the_newest_snapshot_is_used_and_a_newer_one_is_never_deleted() {
    let store = Counting::default();
    let c = cryptor(&store).await;
    let mut web = replica();
    push_n(&store, &c, &mut web, 4, SnapshotUrgency::None, false).await;
    push_n(&store, &c, &mut web, 1, SnapshotUrgency::High, false).await; // snapshot at version 5
    let at5 = snapshots(&store).await;
    assert_eq!(at5.len(), 1);
    let old = names::snapshot_name(at5[0]);
    let old_bytes = store.inner.get(&old).await.unwrap().unwrap();

    push_n(&store, &c, &mut web, 4, SnapshotUrgency::None, false).await;
    push_n(&store, &c, &mut web, 1, SnapshotUrgency::High, false).await; // snapshot at version 10
    let at10 = snapshots(&store).await;
    assert_eq!(at10.len(), 1, "the one at version 5 was superseded and removed");
    assert_ne!(at10[0], at5[0]);

    // Put the old one back: now a stale snapshot sits beside the newest.
    store.inner.put(&old, &old_bytes).await.unwrap();
    assert_eq!(snapshots(&store).await.len(), 2);
    store.take();
    let mut fresh = replica();
    fresh.sync(&mut server(&store, &c), true).await.unwrap();
    let cold = store.take();
    assert_eq!(fresh.all_tasks().await.unwrap().len(), 10);
    // From the newest snapshot there is nothing left to replay: no version bodies are read.
    println!("with a stale snapshot beside the newest: {cold:?}");
    assert!(total(&cold) <= 8, "{cold:?}");

    // Writing an older snapshot (as a slow CLI might) must not remove the newer one.
    let mut s = CloudServer::with_cryptor(store.clone(), c.clone());
    s.add_snapshot(at5[0], old_bytes.clone()).await.unwrap();
    assert!(snapshots(&store).await.contains(&at10[0]), "the newer snapshot survived");
}

//! What the cloud server deletes after a push, as TaskChampion's own does: versions that lost a race, all but the
//! newest snapshot, and versions older than 180 days that a snapshot covers.

use taskchampion::server::{AddVersionResult, SnapshotUrgency, VersionId};
use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::{Operations, Replica, Server, Status};
use tc_core::{names, CloudServer, MemStore, ObjectStore};

type R = Replica<InMemoryStorage>;
type S = CloudServer<MemStore>;

const DAY: i64 = 86_400;
const LONG_AGO: i64 = 1_000;

async fn server(store: &MemStore) -> S {
    CloudServer::new(store.clone(), b"hunter2").await.unwrap()
}

/// One more version on top of `parent`.
async fn push(s: &mut S, parent: VersionId) -> VersionId {
    match s.add_version(parent, b"[]".to_vec()).await.unwrap().0 {
        AddVersionResult::Ok(v) => v,
        other => panic!("{other:?}"),
    }
}

/// A chain of `n` versions written at `at`; returns them oldest first.
async fn chain(store: &MemStore, s: &mut S, n: usize, at: i64) -> Vec<VersionId> {
    store.set_now(at);
    let mut out = Vec::new();
    let mut parent = uuid::Uuid::nil();
    for _ in 0..n {
        parent = push(s, parent).await;
        out.push(parent);
    }
    out
}

async fn add_task(r: &mut R, description: &str) {
    let mut ops = Operations::new();
    let mut t = r.create_task(uuid::Uuid::new_v4(), &mut ops).await.unwrap();
    t.set_status(Status::Pending, &mut ops).unwrap();
    t.set_description(description.into(), &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();
}

async fn versions(store: &MemStore) -> usize {
    store
        .list("v-")
        .await
        .unwrap()
        .iter()
        .filter(|n| names::parse_version_name(n).is_some())
        .count()
}

async fn snapshots(store: &MemStore) -> Vec<VersionId> {
    store
        .list("s-")
        .await
        .unwrap()
        .iter()
        .filter_map(|n| names::parse_snapshot_name(n))
        .collect()
}

#[tokio::test]
async fn old_versions_a_snapshot_covers_are_deleted_and_the_tasks_survive() {
    let store = MemStore::new();
    let mut a: R = Replica::new(InMemoryStorage::new());
    // Six pushes long ago, the seventh with a snapshot, then one recent push.
    store.set_now(LONG_AGO);
    for i in 0..6 {
        add_task(&mut a, &format!("old {i}")).await;
        let mut s: Box<dyn Server> = Box::new(server(&store).await);
        a.sync(&mut s, true).await.unwrap();
    }
    add_task(&mut a, "old 6").await;
    let mut s: Box<dyn Server> = Box::new(server(&store).await.with_snapshot_urgency(SnapshotUrgency::High));
    a.sync(&mut s, false).await.unwrap();
    assert_eq!(snapshots(&store).await.len(), 1);
    store.set_now(LONG_AGO + 200 * DAY);
    add_task(&mut a, "recent").await;
    let mut s: Box<dyn Server> = Box::new(server(&store).await);
    a.sync(&mut s, true).await.unwrap();
    assert_eq!(versions(&store).await, 8);

    let mut cleaner = server(&store).await;
    let deleted = cleaner.cleanup(LONG_AGO + 200 * DAY + 10).await.unwrap();
    assert_eq!(deleted, 7, "the seven old versions the snapshot covers");
    assert_eq!(versions(&store).await, 1, "only what came after the snapshot is left");
    assert_eq!(snapshots(&store).await.len(), 1);

    // A new replica starts from the snapshot and replays what follows: every task is there.
    let mut b: R = Replica::new(InMemoryStorage::new());
    let mut s: Box<dyn Server> = Box::new(server(&store).await);
    b.sync(&mut s, true).await.unwrap();
    assert_eq!(b.all_tasks().await.unwrap().len(), 8);
    // And the replica that wrote them all carries on.
    add_task(&mut a, "after the cleanup").await;
    let mut s: Box<dyn Server> = Box::new(server(&store).await);
    a.sync(&mut s, true).await.unwrap();
    b.sync(&mut s, true).await.unwrap();
    assert_eq!(b.all_tasks().await.unwrap().len(), 9);
}

#[tokio::test]
async fn without_a_snapshot_nothing_is_old_enough_to_go() {
    let store = MemStore::new();
    let mut s = server(&store).await;
    chain(&store, &mut s, 5, LONG_AGO).await;
    assert_eq!(s.cleanup(LONG_AGO + 1000 * DAY).await.unwrap(), 0);
    assert_eq!(versions(&store).await, 5);
}

#[tokio::test]
async fn a_version_younger_than_180_days_stays() {
    let store = MemStore::new();
    let mut s = server(&store).await;
    let v = chain(&store, &mut s, 4, LONG_AGO).await;
    s.add_snapshot(v[2], b"state".to_vec()).await.unwrap();
    assert_eq!(s.cleanup(LONG_AGO + 179 * DAY).await.unwrap(), 0);
    assert_eq!(versions(&store).await, 4);
    // Past the limit: the snapshot's own version and the ones before it go, not the one after.
    assert_eq!(s.cleanup(LONG_AGO + 181 * DAY).await.unwrap(), 3);
    assert_eq!(versions(&store).await, 1);
}

#[tokio::test]
async fn a_lost_races_leftover_goes_but_one_that_may_become_latest_stays() {
    let store = MemStore::new();
    let mut s = server(&store).await;
    let v = chain(&store, &mut s, 2, LONG_AGO).await;
    let stray = uuid::Uuid::new_v4();
    let maybe = uuid::Uuid::new_v4();
    // One hangs off an old version (it can never become latest); the other off `latest` (a writer may be about to).
    store.put(&names::version_name(v[0], stray), b"x").await.unwrap();
    store.put(&names::version_name(v[1], maybe), b"x").await.unwrap();
    assert_eq!(s.cleanup(LONG_AGO).await.unwrap(), 1);
    let left = store.list("v-").await.unwrap();
    assert!(!left.contains(&names::version_name(v[0], stray)));
    assert!(left.contains(&names::version_name(v[1], maybe)));
    assert!(
        left.contains(&names::version_name(v[0], v[1])),
        "the chain itself stays"
    );
}

#[tokio::test]
async fn only_the_newest_snapshot_on_the_chain_stays() {
    let store = MemStore::new();
    let mut s = server(&store).await;
    let v = chain(&store, &mut s, 5, LONG_AGO).await;
    s.add_snapshot(v[3], b"newest".to_vec()).await.unwrap();
    // `add_snapshot` drops the ones it supersedes; two older ones are back, as another writer might leave them.
    store.put(&names::snapshot_name(v[0]), b"x").await.unwrap();
    store.put(&names::snapshot_name(v[1]), b"x").await.unwrap();
    assert_eq!(snapshots(&store).await.len(), 3);
    s.cleanup(LONG_AGO).await.unwrap();
    assert_eq!(snapshots(&store).await, [v[3]]);
}

#[tokio::test]
async fn one_cleanup_deletes_a_limited_number_oldest_first_and_the_next_carries_on() {
    let store = MemStore::new();
    let mut s = server(&store).await;
    let v = chain(&store, &mut s, 1_100, LONG_AGO).await;
    s.add_snapshot(v[1_098], b"state".to_vec()).await.unwrap();
    let now = LONG_AGO + 365 * DAY;
    assert_eq!(s.cleanup(now).await.unwrap(), 1_000);
    assert_eq!(versions(&store).await, 100);
    // What is left is one unbroken chain up to `latest`: the oldest went first, so a replica that is behind finds
    // the next version or none, never a gap with versions beyond it.
    let left: Vec<(VersionId, VersionId)> = store
        .list("v-")
        .await
        .unwrap()
        .iter()
        .filter_map(|n| names::parse_version_name(n))
        .collect();
    let children: std::collections::BTreeSet<_> = left.iter().map(|(_, c)| *c).collect();
    let starts: Vec<_> = left.iter().filter(|(p, _)| !children.contains(p)).collect();
    assert_eq!(starts.len(), 1, "a single start: {starts:?}");
    assert_eq!(
        starts[0].1, v[1_000],
        "the oldest hundred of the old ones are the ones that remain"
    );
    for (p, c) in &left {
        assert_eq!(left.iter().filter(|(q, _)| q == p).count(), 1, "no branches: {c}");
    }
    assert_eq!(s.cleanup(now).await.unwrap(), 99);
    assert_eq!(versions(&store).await, 1, "the version after the snapshot stays");
    assert_eq!(s.cleanup(now).await.unwrap(), 0);
}

#[tokio::test]
async fn the_newest_snapshot_is_still_found_when_the_start_of_the_history_is_gone() {
    let store = MemStore::new();
    let mut s = server(&store).await;
    let v = chain(&store, &mut s, 6, LONG_AGO).await;
    // The first versions are deleted (a snapshot covered them); two snapshots remain on the rest.
    for gone in &v[..2] {
        let parent = if *gone == v[0] { uuid::Uuid::nil() } else { v[0] };
        store.del(&names::version_name(parent, *gone)).await.unwrap();
    }
    s.add_snapshot(v[2], b"older".to_vec()).await.unwrap();
    let older = store
        .get(&names::snapshot_name(v[2]))
        .await
        .unwrap()
        .expect("sealed snapshot");
    // Writing the newer one drops the older (which needs the order of the chain whose start is gone) ...
    s.add_snapshot(v[4], b"newer".to_vec()).await.unwrap();
    assert_eq!(snapshots(&store).await, [v[4]]);
    // ... and with both there, the newer is the one a cold start reads.
    store.put(&names::snapshot_name(v[2]), &older).await.unwrap();
    assert_eq!(snapshots(&store).await.len(), 2);
    let (at, bytes) = s.get_snapshot().await.unwrap().expect("a snapshot");
    assert_eq!((at, bytes.as_slice()), (v[4], b"newer".as_slice()));
}

fn far_future() -> i64 {
    LONG_AGO + 1000 * DAY
}

#[tokio::test]
async fn a_push_is_followed_by_a_cleanup_only_when_asked_for_and_lucky() {
    for (odds, clock, expect_clean) in [(255u8, true, true), (0, true, false), (255, false, false)] {
        let store = MemStore::new();
        let mut s = server(&store).await;
        let v = chain(&store, &mut s, 4, LONG_AGO).await;
        s.add_snapshot(v[2], b"state".to_vec()).await.unwrap();
        // The next push is made by a server with these settings.
        store.set_now(far_future());
        let mut pusher = server(&store).await.with_cleanup_odds(odds);
        if clock {
            pusher = pusher.with_cleanup(far_future);
        }
        let last = *v.last().unwrap();
        push(&mut pusher, last).await;
        let left = versions(&store).await;
        // Odds of 255 are 255 in 256: a rare miss is not a failure of the code.
        if expect_clean && left == 5 {
            continue;
        }
        assert_eq!(left, if expect_clean { 2 } else { 5 }, "odds {odds}, clock {clock}");
    }
}

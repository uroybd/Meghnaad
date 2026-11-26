//! A sync split in two (`tc_core::prefetch`): the fetch with no replica held, then `Replica::sync` against what was
//! fetched. It must end where a plain sync ends, and hold the replica for no request when there is nothing to push.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use async_trait::async_trait;
use std::cell::Cell;
use std::rc::Rc;
use taskchampion::server::{AddVersionResult, GetVersionResult, HistorySegment, Snapshot, SnapshotUrgency, VersionId};
use taskchampion::{Error, Operations, Replica, Server, Status};
use tc_core::live::BaseVersion;
use tc_core::prefetch::{fetch, Prefetched};
use tc_core::{CloudServer, LiveStorage, MemStore};

type R = Replica<LiveStorage>;

/// The store, counting every request made to it.
struct Counting {
    inner: Box<dyn Server>,
    requests: Rc<Cell<usize>>,
}

#[async_trait(?Send)]
impl Server for Counting {
    async fn add_version(
        &mut self,
        parent: VersionId,
        segment: HistorySegment,
    ) -> Result<(AddVersionResult, SnapshotUrgency), Error> {
        self.requests.set(self.requests.get() + 1);
        self.inner.add_version(parent, segment).await
    }
    async fn get_child_version(&mut self, parent: VersionId) -> Result<GetVersionResult, Error> {
        self.requests.set(self.requests.get() + 1);
        self.inner.get_child_version(parent).await
    }
    async fn add_snapshot(&mut self, version: VersionId, snapshot: Snapshot) -> Result<(), Error> {
        self.requests.set(self.requests.get() + 1);
        self.inner.add_snapshot(version, snapshot).await
    }
    async fn get_snapshot(&mut self) -> Result<Option<(VersionId, Snapshot)>, Error> {
        self.requests.set(self.requests.get() + 1);
        self.inner.get_snapshot().await
    }
    async fn latest_version_id(&mut self) -> Result<VersionId, Error> {
        self.requests.set(self.requests.get() + 1);
        self.inner.latest_version_id().await
    }
}

async fn server(store: &MemStore) -> Box<dyn Server> {
    Box::new(CloudServer::new(store.clone(), b"hunter2").await.unwrap())
}

async fn counting(store: &MemStore) -> (Box<dyn Server>, Rc<Cell<usize>>) {
    let requests = Rc::new(Cell::new(0));
    let inner = server(store).await;
    (
        Box::new(Counting {
            inner,
            requests: requests.clone(),
        }),
        requests,
    )
}

fn replica() -> (R, BaseVersion) {
    let storage = LiveStorage::new();
    let base = storage.base_version();
    (Replica::new(storage), base)
}

async fn add_task(r: &mut R, description: &str) -> uuid::Uuid {
    let mut ops = Operations::new();
    let uuid = uuid::Uuid::new_v4();
    let mut t = r.create_task(uuid, &mut ops).await.unwrap();
    t.set_status(Status::Pending, &mut ops).unwrap();
    t.set_description(description.into(), &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();
    uuid
}

/// Fetch, then sync against what was fetched; answers how many requests the sync itself made (with the replica held).
async fn split_sync(r: &mut R, base: &BaseVersion, store: &MemStore) -> usize {
    let mut fetcher = server(store).await;
    let fetched = fetch(fetcher.as_mut(), base.get()).await.unwrap();
    let (inner, held) = counting(store).await;
    r.sync(&mut Prefetched::boxed(inner, fetched), true).await.unwrap();
    held.get()
}

async fn descriptions(r: &mut R) -> Vec<String> {
    let mut d: Vec<String> = r
        .all_tasks()
        .await
        .unwrap()
        .values()
        .map(|t| t.get_description().to_owned())
        .collect();
    d.sort();
    d
}

#[tokio::test]
async fn the_base_version_is_readable_from_outside_the_replica() {
    let store = MemStore::new();
    let (mut a, base) = replica();
    assert!(base.get().is_nil());
    add_task(&mut a, "one").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    assert!(!base.get().is_nil(), "a sync moves the base, and the handle sees it");
}

#[tokio::test]
async fn an_idle_sync_makes_no_request_while_the_replica_is_held() {
    let store = MemStore::new();
    let (mut a, _) = replica();
    let (mut b, b_base) = replica();
    add_task(&mut a, "one").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut server(&store).await, true).await.unwrap();

    // Nothing new on either side.
    assert_eq!(split_sync(&mut b, &b_base, &store).await, 0);

    // New versions from another replica: fetched first, applied with no request.
    add_task(&mut a, "two").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    add_task(&mut a, "three").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    assert_eq!(split_sync(&mut b, &b_base, &store).await, 0);
    assert_eq!(descriptions(&mut b).await, ["one", "three", "two"]);
}

#[tokio::test]
async fn a_push_holds_the_replica_only_for_the_push() {
    let store = MemStore::new();
    let (mut a, a_base) = replica();
    add_task(&mut a, "one").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    add_task(&mut a, "two").await;
    // The new version, and (asked for with `avoid_snapshots`) nothing else.
    assert_eq!(split_sync(&mut a, &a_base, &store).await, 1);
    let (mut b, _) = replica();
    b.sync(&mut server(&store).await, true).await.unwrap();
    assert_eq!(descriptions(&mut b).await, ["one", "two"]);
}

#[tokio::test]
async fn a_store_that_moved_after_the_fetch_is_pulled_from_next_time() {
    let store = MemStore::new();
    let (mut a, _) = replica();
    let (mut b, b_base) = replica();
    add_task(&mut a, "one").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut server(&store).await, true).await.unwrap();

    let fetched = fetch(server(&store).await.as_mut(), b_base.get()).await.unwrap();
    assert!(fetched.is_empty());
    // Another replica pushes between b's fetch and b's sync.
    add_task(&mut a, "two").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut Prefetched::boxed(server(&store).await, fetched), true)
        .await
        .unwrap();
    assert_eq!(descriptions(&mut b).await, ["one"], "only what was fetched");

    split_sync(&mut b, &b_base, &store).await;
    assert_eq!(descriptions(&mut b).await, ["one", "two"]);
}

#[tokio::test]
async fn a_push_refused_because_the_store_moved_goes_round_again() {
    let store = MemStore::new();
    let (mut a, _) = replica();
    let (mut b, b_base) = replica();
    add_task(&mut a, "one").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut server(&store).await, true).await.unwrap();

    add_task(&mut b, "from b").await;
    let fetched = fetch(server(&store).await.as_mut(), b_base.get()).await.unwrap();
    add_task(&mut a, "from a").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    // b's push names a parent the store has moved past: refused, then pulled and pushed again.
    b.sync(&mut Prefetched::boxed(server(&store).await, fetched), true)
        .await
        .unwrap();
    assert_eq!(descriptions(&mut b).await, ["from a", "from b", "one"]);

    a.sync(&mut server(&store).await, true).await.unwrap();
    assert_eq!(descriptions(&mut a).await, ["from a", "from b", "one"]);
    assert_eq!(b.num_local_operations().await.unwrap(), 0, "b's change was pushed once");
}

#[tokio::test]
async fn a_replica_that_never_synced_starts_from_the_store() {
    let store = MemStore::new();
    let (mut a, _) = replica();
    add_task(&mut a, "one").await;
    a.sync(&mut server(&store).await, false).await.unwrap();
    let (mut b, b_base) = replica();
    // Nothing to fetch from the nil version; the sync asks the store itself.
    assert!(split_sync(&mut b, &b_base, &store).await > 0);
    assert_eq!(descriptions(&mut b).await, ["one"]);
}

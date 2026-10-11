//! A replica that reaches the bucket through the Worker's plain protocol (`tc_core::wire`: `RemoteServer` on one side,
//! `pull` and `push` on the other, the bytes in between encoded and decoded as they are over HTTP) must end up exactly
//! where one that reaches the bucket directly does: the same tasks, pulling runs of versions longer than one batch,
//! pushing, losing a race to another replica, and starting from a snapshot.

use async_trait::async_trait;
use std::cell::RefCell;
use std::rc::Rc;
use taskchampion::server::SnapshotUrgency;
use taskchampion::{Replica, Server};
use tc_core::cli::{execute, load_facts, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::taskrc::Config;
use tc_core::wire::{decode_versions, encode_versions, pull, push, Pushed, RemoteServer, Transport, Version, PULL_MAX};
use tc_core::{load_cryptor, CloudServer, LiveStorage, MemStore};
use uuid::Uuid;

type R = Replica<LiveStorage>;
type TcResult<T> = Result<T, taskchampion::Error>;

/// The Worker, in the same process: a TaskChampion server over the bucket, and a count of what was asked of it.
#[derive(Clone)]
struct Worker {
    store: MemStore,
    cryptor: tc_core::crypto::Cryptor,
    urgency: SnapshotUrgency,
    asked: Rc<RefCell<Vec<&'static str>>>,
}

impl Worker {
    fn server(&self) -> CloudServer<MemStore> {
        CloudServer::with_cryptor(self.store.clone(), self.cryptor.clone()).with_snapshot_urgency(self.urgency)
    }
}

#[async_trait(?Send)]
impl Transport for Worker {
    async fn pull(&mut self, since: Uuid) -> TcResult<(Vec<Version>, bool)> {
        self.asked.borrow_mut().push("pull");
        let (versions, more) = pull(&mut self.server(), since).await?;
        // Over the wire and back.
        Ok((decode_versions(&encode_versions(&versions)).unwrap(), more))
    }

    async fn push(&mut self, parent: Uuid, segment: Vec<u8>) -> TcResult<Pushed> {
        self.asked.borrow_mut().push("push");
        let reply = push(&mut self.server(), parent, segment).await?;
        Ok(serde_json::from_str(&serde_json::to_string(&reply).unwrap()).unwrap())
    }

    async fn get_snapshot(&mut self) -> TcResult<Option<(Uuid, Vec<u8>)>> {
        self.asked.borrow_mut().push("get_snapshot");
        self.server().get_snapshot().await
    }

    async fn put_snapshot(&mut self, version: Uuid, snapshot: Vec<u8>) -> TcResult<()> {
        self.asked.borrow_mut().push("put_snapshot");
        self.server().add_snapshot(version, snapshot).await
    }
}

async fn worker(urgency: SnapshotUrgency) -> Worker {
    let store = MemStore::new();
    let cryptor = load_cryptor(&store, b"secret").await.unwrap();
    Worker {
        store,
        cryptor,
        urgency,
        asked: Rc::default(),
    }
}

fn remote(w: &Worker) -> Box<dyn Server> {
    Box::new(RemoteServer::new(w.clone()))
}

fn direct(w: &Worker) -> Box<dyn Server> {
    Box::new(w.server())
}

async fn run(r: &mut R, line: &str, step: i64) {
    let mut undo = UndoStack::default();
    let opts = Options {
        confirmed: true,
        ..Options::default()
    };
    execute(
        r,
        &Config::default(),
        Clock::utc(1_790_000_000 + step * 60),
        &split_words(line),
        opts,
        &mut undo,
    )
    .await;
}

async fn tasks(r: &mut R) -> Vec<tc_core::model::Facts> {
    let mut all = load_facts(r).await.unwrap();
    for f in &mut all {
        f.depends.sort();
    }
    all
}

fn replica() -> R {
    Replica::new(LiveStorage::new())
}

#[tokio::test(flavor = "current_thread")]
async fn a_replica_far_behind_catches_up_in_a_few_requests_and_gets_every_task() {
    let w = worker(SnapshotUrgency::None).await;
    let mut a = replica();
    // More versions than one pull carries, so the batching is used: one version per change, as the CLI makes them.
    let n = PULL_MAX * 2 + 7;
    for i in 0..n {
        run(
            &mut a,
            &format!("add Task {i} project:P{} +t{}", i % 3, i % 2),
            i as i64,
        )
        .await;
        a.sync(&mut direct(&w), true).await.unwrap();
    }
    let mut b = replica();
    w.asked.borrow_mut().clear();
    b.sync(&mut remote(&w), true).await.unwrap();
    assert_eq!(tasks(&mut b).await, tasks(&mut a).await);
    assert_eq!(tasks(&mut b).await.len(), n);
    let pulls = w.asked.borrow().iter().filter(|r| **r == "pull").count();
    // 79 versions at 24 a pull: four requests (the last short one ends it), not 80.
    assert!(
        (3..=5).contains(&pulls),
        "{pulls} pulls for {n} versions ({:?})",
        w.asked.borrow()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn what_a_replica_pushes_through_the_worker_is_what_a_direct_one_sees() {
    let w = worker(SnapshotUrgency::None).await;
    let mut browser = replica();
    let mut cli = replica();
    run(&mut browser, "add from the browser +b", 0).await;
    run(&mut browser, "1 modify project:Home", 1).await;
    browser.sync(&mut remote(&w), true).await.unwrap();
    cli.sync(&mut direct(&w), true).await.unwrap();
    assert_eq!(tasks(&mut cli).await, tasks(&mut browser).await);
    assert_eq!(tasks(&mut cli).await.len(), 1);

    // And back: a change made by the CLI reaches the browser's replica.
    run(&mut cli, "add from the cli", 2).await;
    run(&mut cli, "1 done", 3).await;
    cli.sync(&mut direct(&w), true).await.unwrap();
    browser.sync(&mut remote(&w), true).await.unwrap();
    assert_eq!(tasks(&mut cli).await, tasks(&mut browser).await);
    assert_eq!(tasks(&mut cli).await.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn losing_a_race_to_another_replica_pulls_then_pushes_again() {
    let w = worker(SnapshotUrgency::None).await;
    let mut a = replica();
    let mut b = replica();
    run(&mut a, "add first", 0).await;
    a.sync(&mut direct(&w), true).await.unwrap();
    b.sync(&mut remote(&w), true).await.unwrap();
    // Both change something, a gets to the bucket first; b has to take a's version in before its own goes in.
    run(&mut a, "add from a", 1).await;
    run(&mut b, "add from b", 2).await;
    a.sync(&mut direct(&w), true).await.unwrap();
    w.asked.borrow_mut().clear();
    b.sync(&mut remote(&w), true).await.unwrap();
    assert!(w.asked.borrow().contains(&"push"));
    a.sync(&mut direct(&w), true).await.unwrap();
    assert_eq!(tasks(&mut a).await, tasks(&mut b).await);
    assert_eq!(tasks(&mut a).await.len(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn a_snapshot_goes_through_both_ways() {
    let w = worker(SnapshotUrgency::High).await;
    let mut a = replica();
    for i in 0..30 {
        run(&mut a, &format!("add Task {i}"), i).await;
    }
    // The server wants a snapshot, so the browser's replica sends one through the worker.
    a.sync(&mut remote(&w), false).await.unwrap();
    assert!(w.asked.borrow().contains(&"put_snapshot"), "{:?}", w.asked.borrow());
    assert!(w.store.names().iter().any(|n| n.starts_with("s-")));
    // A replica that starts later restores it through the worker instead of replaying versions.
    let mut b = replica();
    w.asked.borrow_mut().clear();
    b.sync(&mut remote(&w), true).await.unwrap();
    assert!(w.asked.borrow().contains(&"get_snapshot"));
    assert_eq!(tasks(&mut b).await, tasks(&mut a).await);
    assert_eq!(tasks(&mut b).await.len(), 30);
}

#[tokio::test(flavor = "current_thread")]
async fn an_empty_bucket_is_an_empty_replica() {
    let w = worker(SnapshotUrgency::None).await;
    let mut b = replica();
    b.sync(&mut remote(&w), true).await.unwrap();
    assert!(tasks(&mut b).await.is_empty());
    // And it can still push its first version.
    run(&mut b, "add first ever", 0).await;
    b.sync(&mut remote(&w), true).await.unwrap();
    let mut c = replica();
    c.sync(&mut direct(&w), true).await.unwrap();
    assert_eq!(tasks(&mut c).await.len(), 1);
}

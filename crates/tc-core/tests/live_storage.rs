//! `LiveStorage` against TaskChampion's own `InMemoryStorage`: the same random writes, commits and abandoned
//! transactions go to both, and every answer either gives (and the whole state after each transaction) must match.

use taskchampion::chrono::{TimeZone, Utc};
use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::storage::{Storage, StorageTxn, TaskMap};
use taskchampion::Operation;
use tc_core::LiveStorage;
use uuid::Uuid;

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

fn id(n: usize) -> Uuid {
    Uuid::from_u128(n as u128 + 1)
}

fn task_map(rng: &mut Rng) -> TaskMap {
    let mut m = TaskMap::new();
    for k in ["status", "description", "project"] {
        if rng.below(3) > 0 {
            m.insert(k.into(), format!("v{}", rng.below(5)));
        }
    }
    m
}

fn operation(rng: &mut Rng) -> Operation {
    let uuid = id(rng.below(8));
    let timestamp = Utc.timestamp_opt(1_700_000_000 + rng.below(1000) as i64, 0).unwrap();
    match rng.below(4) {
        0 => Operation::Create { uuid },
        1 => Operation::Update {
            uuid,
            property: "description".into(),
            old_value: None,
            value: Some(format!("v{}", rng.below(5))),
            timestamp,
        },
        2 => Operation::Delete {
            uuid,
            old_task: task_map(rng),
        },
        _ => Operation::UndoPoint,
    }
}

/// Everything either storage can show, in a comparable form.
async fn state(txn: &mut dyn StorageTxn) -> String {
    let mut tasks = txn.all_tasks().await.unwrap();
    tasks.sort_by_key(|(u, _)| *u);
    let tasks: Vec<_> = tasks
        .into_iter()
        .map(|(u, t)| {
            let mut t: Vec<_> = t.into_iter().collect();
            t.sort();
            (u, t)
        })
        .collect();
    let mut per_task = Vec::new();
    for n in 0..8 {
        per_task.push(txn.get_task_operations(id(n)).await.unwrap());
    }
    format!(
        "{tasks:?}\nbase {:?}\nunsynced {:?} ({})\nper task {per_task:?}\nworking set {:?}\npending {:?}",
        txn.base_version().await.unwrap(),
        txn.unsynced_operations().await.unwrap(),
        txn.num_unsynced_operations().await.unwrap(),
        txn.get_working_set().await.unwrap(),
        {
            let mut p = txn.get_pending_tasks().await.unwrap();
            p.sort_by_key(|(u, _)| *u);
            p
        },
    )
}

/// One random call on both, which must answer alike.
async fn step(rng: &mut Rng, a: &mut dyn StorageTxn, b: &mut dyn StorageTxn) {
    let u = id(rng.below(8));
    let (x, y) = match rng.below(14) {
        0 => (
            format!("{:?}", a.create_task(u).await),
            format!("{:?}", b.create_task(u).await),
        ),
        1 | 2 => {
            let t = task_map(rng);
            (
                format!("{:?}", a.set_task(u, t.clone()).await),
                format!("{:?}", b.set_task(u, t).await),
            )
        }
        3 => (
            format!("{:?}", a.delete_task(u).await),
            format!("{:?}", b.delete_task(u).await),
        ),
        4 => {
            let v = Uuid::from_u128(1000 + rng.below(4) as u128);
            (
                format!("{:?}", a.set_base_version(v).await),
                format!("{:?}", b.set_base_version(v).await),
            )
        }
        5 | 6 => {
            let op = operation(rng);
            (
                format!("{:?}", a.add_operation(op.clone()).await),
                format!("{:?}", b.add_operation(op).await),
            )
        }
        7 => {
            // Usually the operation that is last (which can go), sometimes another.
            let ops = a.unsynced_operations().await.unwrap();
            let op = match (ops.last(), rng.below(4)) {
                (Some(last), 1..) => last.clone(),
                _ => operation(rng),
            };
            (
                format!("{:?}", a.remove_operation(op.clone()).await),
                format!("{:?}", b.remove_operation(op).await),
            )
        }
        8 => (
            format!("{:?}", a.sync_complete().await),
            format!("{:?}", b.sync_complete().await),
        ),
        9 | 10 => (
            format!("{:?}", a.add_to_working_set(u).await),
            format!("{:?}", b.add_to_working_set(u).await),
        ),
        11 | 12 => {
            let len = a.get_working_set().await.unwrap().len();
            let i = rng.below(len + 1);
            let v = if rng.below(2) == 0 { None } else { Some(u) };
            (
                format!("{:?}", a.set_working_set_item(i, v).await),
                format!("{:?}", b.set_working_set_item(i, v).await),
            )
        }
        _ => (
            format!("{:?}", a.clear_working_set().await),
            format!("{:?}", b.clear_working_set().await),
        ),
    };
    assert_eq!(x, y);
}

#[tokio::test(flavor = "current_thread")]
async fn behaves_like_taskchampions_storage_through_commits_and_abandoned_transactions() {
    for seed in 1..=40u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut theirs = InMemoryStorage::new();
        let mut ours = LiveStorage::new();
        for round in 0..60 {
            let commit = rng.below(3) > 0;
            {
                let mut a = theirs.txn().await.unwrap();
                let mut b = ours.txn().await.unwrap();
                for _ in 0..rng.below(12) {
                    step(&mut rng, a.as_mut(), b.as_mut()).await;
                    // Mid-transaction, the changes can be seen by this transaction.
                    assert_eq!(
                        state(a.as_mut()).await,
                        state(b.as_mut()).await,
                        "seed {seed} round {round}"
                    );
                }
                if commit {
                    a.commit().await.unwrap();
                    b.commit().await.unwrap();
                }
                // Not committed: dropped, and so abandoned.
            }
            let mut a = theirs.txn().await.unwrap();
            let mut b = ours.txn().await.unwrap();
            assert_eq!(
                state(a.as_mut()).await,
                state(b.as_mut()).await,
                "seed {seed} round {round} (committed: {commit})"
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_abandoned_sync_completion_puts_back_what_it_dropped_and_unmarks_what_it_marked() {
    let mut db = LiveStorage::new();
    let ts = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    {
        let mut t = db.txn().await.unwrap();
        t.create_task(id(0)).await.unwrap();
        // Operations: one for a task that exists, one for a task that does not, one more for the first.
        t.add_operation(Operation::Create { uuid: id(0) }).await.unwrap();
        t.add_operation(Operation::Create { uuid: id(5) }).await.unwrap();
        t.add_operation(Operation::Update {
            uuid: id(0),
            property: "p".into(),
            old_value: None,
            value: Some("1".into()),
            timestamp: ts,
        })
        .await
        .unwrap();
        t.commit().await.unwrap();
    }
    let before = {
        let mut t = db.txn().await.unwrap();
        state(t.as_mut()).await
    };
    {
        let mut t = db.txn().await.unwrap();
        t.sync_complete().await.unwrap();
        assert_eq!(t.num_unsynced_operations().await.unwrap(), 0);
        assert!(
            t.get_task_operations(id(5)).await.unwrap().is_empty(),
            "dropped with its task"
        );
        // Dropped without commit.
    }
    let mut t = db.txn().await.unwrap();
    assert_eq!(state(t.as_mut()).await, before);
    assert_eq!(t.num_unsynced_operations().await.unwrap(), 3);
}

//! A replica saved and loaded again (what the browser keeps between visits) must be the replica that was saved: the
//! same tasks, the same working-set numbers, the same changes still waiting to be pushed, and it must carry on the
//! same way, including syncing with the server.

use taskchampion::server::SnapshotUrgency;
use taskchampion::Replica;
use tc_core::cli::{execute, load_facts, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::taskrc::Config;
use tc_core::{load_cryptor, save_replica, CloudServer, LiveStorage, MemStore};

type R = Replica<LiveStorage>;

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

async fn run(r: &mut R, undo: &mut UndoStack, line: &str, step: i64) {
    let opts = Options {
        confirmed: true,
        ..Options::default()
    };
    execute(
        r,
        &Config::default(),
        Clock::utc(1_790_000_000 + step * 3600),
        &split_words(line),
        opts,
        undo,
    )
    .await;
}

/// A random command, many of them writes.
fn command(rng: &mut Rng, step: i64) -> String {
    let id = 1 + rng.below(10);
    match rng.below(14) {
        0..=2 => format!(
            "add Task {step} project:P{} +t{} due:{}d",
            rng.below(3),
            rng.below(3),
            1 + rng.below(9)
        ),
        3 => format!("add Daily {step} recur:daily due:today"),
        4 => format!("{id} modify project:Q{} +m{}", rng.below(3), rng.below(3)),
        5 => format!("{id} done"),
        6 => format!("{id} delete"),
        7 => format!("{id} start"),
        8 => format!("{id} annotate note {step}"),
        9 => format!("{id} modify depends:{}", 1 + rng.below(10)),
        10 => "undo".to_owned(),
        11 => format!("{id} modify priority:H wait:{}d", 1 + rng.below(4)),
        12 => "next".to_owned(),
        _ => format!("{id} stop"),
    }
}

/// The tasks as the engine reads them, with the order of `depends` (a hash map's, so it varies) made the same.
async fn tasks(r: &mut R) -> Vec<tc_core::model::Facts> {
    let mut all = load_facts(r).await.unwrap();
    for f in &mut all {
        f.depends.sort();
    }
    all
}

async fn same(a: &mut R, b: &mut R, when: &str) {
    assert_eq!(tasks(a).await, tasks(b).await, "tasks differ {when}");
    let numbers = |w: taskchampion::WorkingSet| (1..=w.largest_index()).map(|i| w.by_index(i)).collect::<Vec<_>>();
    assert_eq!(
        numbers(a.working_set().await.unwrap()),
        numbers(b.working_set().await.unwrap()),
        "working set differs {when}"
    );
    assert_eq!(
        a.get_undo_operations().await.unwrap(),
        b.get_undo_operations().await.unwrap(),
        "waiting changes differ {when}"
    );
    assert_eq!(
        a.num_local_operations().await.unwrap(),
        b.num_local_operations().await.unwrap()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_saved_replica_is_the_same_replica_and_carries_on_the_same_way() {
    for seed in [3u64, 41, 977] {
        let store = MemStore::new();
        let c = load_cryptor(&store, b"secret").await.unwrap();
        let server = |u| -> Box<dyn taskchampion::Server> {
            Box::new(CloudServer::with_cryptor(store.clone(), c.clone()).with_snapshot_urgency(u))
        };
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let storage = LiveStorage::new();
        let mut saver = storage.saver();
        let mut a: R = Replica::new(storage);
        let mut undo = UndoStack::default();
        let mut step = 0;
        for round in 0..6 {
            for _ in 0..25 {
                let line = command(&mut rng, step);
                run(&mut a, &mut undo, &line, step).await;
                step += 1;
            }
            // Save part way, with changes still waiting (sometimes pushed first, sometimes not).
            if round % 2 == 0 {
                a.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
                for _ in 0..5 {
                    let line = command(&mut rng, step);
                    run(&mut a, &mut undo, &line, step).await;
                    step += 1;
                }
            }
            let bytes = save_replica(&mut a, &saver).await.unwrap();
            let restored = LiveStorage::load(&bytes).unwrap();
            let next_saver = restored.saver();
            let mut b: R = Replica::new(restored);
            same(&mut a, &mut b, &format!("after loading (seed {seed}, round {round})")).await;

            // From here on it is the restored one that carries on: more commands, then the push of what was
            // waiting when it was saved and of what came after.
            (a, saver) = (b, next_saver);
            for _ in 0..10 {
                let line = command(&mut rng, step);
                run(&mut a, &mut undo, &line, step).await;
                step += 1;
            }
            a.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
            // Somebody who starts from the server sees the same tasks (not the same numbers: those are local).
            let mut fresh: R = Replica::new(LiveStorage::new());
            fresh.sync(&mut server(SnapshotUrgency::None), false).await.unwrap();
            assert_eq!(
                tasks(&mut fresh).await,
                tasks(&mut a).await,
                "a fresh replica differs from the restored one (seed {seed}, round {round})"
            );
        }
    }
}

#[test]
fn a_copy_that_cannot_be_read_is_refused() {
    let good = LiveStorage::new().save().unwrap();
    assert!(LiveStorage::load(&good).is_ok());
    assert!(LiveStorage::load(b"").is_err());
    assert!(LiveStorage::load(b"not json").is_err());
    assert!(LiveStorage::load(&good[..good.len() - 3]).is_err());
    let other_format = String::from_utf8(good.clone())
        .unwrap()
        .replace("\"format\":1", "\"format\":99");
    assert!(LiveStorage::load(other_format.as_bytes()).is_err());
    let no_zero = String::from_utf8(good)
        .unwrap()
        .replace("\"working_set\":[null]", "\"working_set\":[]");
    assert!(LiveStorage::load(no_zero.as_bytes()).is_err());
}

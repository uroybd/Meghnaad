//! Saving a replica's storage to bytes and loading it again: what the browser keeps in IndexedDB between visits.
//! (Behind the `save` feature: the Worker never does this and leaves the code out.)

use super::{Error, Generation, LiveStorage, Result, TaskMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use taskchampion::Operation;
use uuid::Uuid;

/// A way to ask a storage that a replica owns for a [`save`](LiveStorage::save) of itself: [`request`](Self::request),
/// then do anything that starts a transaction, then [`take`](Self::take) the bytes. (A replica keeps its storage to
/// itself, so there is no other way in; the storage looks at this whenever a transaction begins, when its data is
/// whole.) See [`save_replica`].
#[derive(Clone, Debug, Default)]
pub struct Saver(Arc<SaverState>);

#[derive(Debug, Default)]
struct SaverState {
    wanted: AtomicBool,
    out: Mutex<Option<Vec<u8>>>,
}

impl Saver {
    pub fn request(&self) {
        self.0.wanted.store(true, Ordering::Release);
    }

    pub fn take(&self) -> Option<Vec<u8>> {
        self.0
            .out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

/// The bytes that [`LiveStorage::load`] reads back, from a replica that was made with this storage.
pub async fn save_replica(
    replica: &mut taskchampion::Replica<LiveStorage>,
    saver: &Saver,
) -> std::result::Result<Vec<u8>, Error> {
    saver.request();
    // Any transaction will do; the cheapest one that reads.
    replica.num_local_operations().await?;
    saver
        .take()
        .ok_or_else(|| Error::Database("the storage did not save itself".into()))
}

/// What is kept of the storage between visits (see [`LiveStorage::save`]).
#[derive(Serialize, Deserialize)]
struct Saved {
    format: u32,
    base_version: Uuid,
    tasks: HashMap<Uuid, TaskMap>,
    operations: Vec<(bool, Operation)>,
    working_set: Vec<Option<Uuid>>,
}

/// Bumped if `Saved` changes in a way an older copy cannot be read after.
const SAVED_FORMAT: u32 = 1;

impl LiveStorage {
    /// Everything the storage holds, as bytes to keep somewhere (the browser keeps them in IndexedDB), so a visit
    /// can start from where the last one ended instead of downloading the history again.
    pub fn save(&self) -> std::result::Result<Vec<u8>, serde_json::Error> {
        // Borrowed, not copied: the tasks are most of the memory.
        #[derive(Serialize)]
        struct View<'a> {
            format: u32,
            base_version: Uuid,
            tasks: &'a HashMap<Uuid, TaskMap>,
            operations: &'a [(bool, Operation)],
            working_set: &'a [Option<Uuid>],
        }
        serde_json::to_vec(&View {
            format: SAVED_FORMAT,
            base_version: self.base_version,
            tasks: &self.tasks,
            operations: &self.operations,
            working_set: &self.working_set,
        })
    }

    /// The storage [`save`](Self::save) wrote. A copy that cannot be read (another format, damaged) is an error, and
    /// the caller starts from the server instead.
    pub fn load(bytes: &[u8]) -> Result<Self> {
        let bad = |e: String| Error::Database(format!("the saved copy of the tasks is unreadable: {e}"));
        let saved: Saved = serde_json::from_slice(bytes).map_err(|e| bad(e.to_string()))?;
        if saved.format != SAVED_FORMAT {
            return Err(bad(format!("format {}", saved.format)));
        }
        if saved.working_set.first() != Some(&None) {
            return Err(bad("the working set does not start empty".into()));
        }
        Ok(LiveStorage {
            generation: Generation::default(),
            saver: Saver::default(),
            tasks: saved.tasks,
            base_version: saved.base_version,
            operations: saved.operations,
            working_set: saved.working_set,
        })
    }
}

impl LiveStorage {
    /// A handle for saving the storage after it has moved into a replica.
    pub fn saver(&self) -> Saver {
        self.saver.clone()
    }

    /// Called when a transaction begins, when the data is whole: if a save was asked for, make it.
    pub(super) fn save_if_asked(&self) {
        if self.saver.0.wanted.swap(false, Ordering::AcqRel) {
            if let Ok(bytes) = self.save() {
                *self
                    .saver
                    .0
                    .out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(bytes);
            }
        }
    }
}

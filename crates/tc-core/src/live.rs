//! The replica's storage: everything in memory, changed in place.
//!
//! TaskChampion's own `InMemoryStorage` copies the whole database (every task and every recorded change) the first
//! time a transaction writes, then swaps the copy in. That is fine for a test and wasteful here, where each sync and
//! each write would copy thousands of tasks and briefly hold twice the memory. This one changes its data directly and
//! keeps a short journal of what each transaction did, so a transaction that is dropped without `commit` (an error
//! halfway through a sync, say) is undone exactly, as TaskChampion requires.
//!
//! `tests/live_storage.rs` runs random sequences of writes, commits and abandoned transactions against this and
//! `InMemoryStorage` and compares everything either can show.

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use taskchampion::storage::{Storage, StorageTxn, TaskMap};
use taskchampion::{Error, Operation};
use uuid::Uuid;

type Result<T> = std::result::Result<T, Error>;

/// The nil version: what a replica that has never synced is based on.
const NO_VERSION: Uuid = Uuid::nil();

pub struct LiveStorage {
    tasks: HashMap<Uuid, TaskMap>,
    base_version: Uuid,
    /// Every operation, oldest first, with whether the server has it.
    operations: Vec<(bool, Operation)>,
    /// Element 0 is always `None`; a task's number is its index.
    working_set: Vec<Option<Uuid>>,
}

impl Default for LiveStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveStorage {
    pub fn new() -> Self {
        LiveStorage {
            tasks: HashMap::new(),
            base_version: NO_VERSION,
            operations: Vec::new(),
            working_set: vec![None],
        }
    }
}

/// How to take back one change.
enum Undo {
    /// A task as it was (`None`: it did not exist).
    Task(Uuid, Option<TaskMap>),
    BaseVersion(Uuid),
    /// This many operations were added at the end (a run of adds is one entry: an import adds tens of thousands).
    OperationsAdded(usize),
    /// The last operation was removed.
    OperationRemoved((bool, Operation)),
    /// `sync_complete` dropped these operations (with their old positions) and marked these kept ones (by their new
    /// position) as synced.
    SyncComplete {
        dropped: Vec<(usize, (bool, Operation))>,
        marked: Vec<usize>,
    },
    WorkingSetAdded(usize),
    WorkingSetItem(usize, Option<Uuid>),
    /// This many empty entries were trimmed from the end of the working set.
    WorkingSetTrimmed(usize),
    WorkingSetAll(Vec<Option<Uuid>>),
}

struct Txn<'a> {
    db: &'a mut LiveStorage,
    journal: Vec<Undo>,
    /// Tasks whose original is already in the journal. A task is written once per operation applied to it, and
    /// only the state before the first write is worth keeping (the rest would be a copy per write until commit).
    saved: HashSet<Uuid>,
}

impl Txn<'_> {
    /// Take back everything done in this transaction, latest first.
    fn roll_back(&mut self) {
        while let Some(undo) = self.journal.pop() {
            let db = &mut *self.db;
            match undo {
                Undo::Task(uuid, Some(old)) => {
                    db.tasks.insert(uuid, old);
                }
                Undo::Task(uuid, None) => {
                    db.tasks.remove(&uuid);
                }
                Undo::BaseVersion(v) => db.base_version = v,
                Undo::OperationsAdded(n) => {
                    let keep = db.operations.len().saturating_sub(n);
                    db.operations.truncate(keep);
                }
                Undo::OperationRemoved(op) => db.operations.push(op),
                Undo::SyncComplete { dropped, marked } => {
                    for i in marked {
                        db.operations[i].0 = false;
                    }
                    // Oldest first, so each goes back where it was.
                    for (i, op) in dropped {
                        db.operations.insert(i, op);
                    }
                }
                Undo::WorkingSetAdded(n) => {
                    let keep = db.working_set.len().saturating_sub(n);
                    db.working_set.truncate(keep);
                }
                Undo::WorkingSetItem(i, old) => db.working_set[i] = old,
                Undo::WorkingSetTrimmed(n) => db.working_set.resize(db.working_set.len() + n, None),
                Undo::WorkingSetAll(old) => db.working_set = old,
            }
        }
    }

    /// Remember a task's state before this transaction's first change to it.
    fn save_task(&mut self, uuid: Uuid, old: Option<TaskMap>) {
        if self.saved.insert(uuid) {
            self.journal.push(Undo::Task(uuid, old));
        }
    }

    /// Drop empty entries from the end of the working set (the first entry stays).
    fn trim_working_set(&mut self) {
        let mut trimmed = 0;
        while self.db.working_set.len() > 1 && self.db.working_set.last() == Some(&None) {
            self.db.working_set.pop();
            trimmed += 1;
        }
        if trimmed > 0 {
            self.journal.push(Undo::WorkingSetTrimmed(trimmed));
        }
    }
}

impl Drop for Txn<'_> {
    /// A transaction that was not committed is abandoned.
    fn drop(&mut self) {
        self.roll_back();
    }
}

#[async_trait]
impl Storage for LiveStorage {
    async fn txn<'a>(&'a mut self) -> Result<Box<dyn StorageTxn + Send + 'a>> {
        Ok(Box::new(Txn {
            db: self,
            journal: Vec::new(),
            saved: HashSet::new(),
        }))
    }
}

#[async_trait]
impl StorageTxn for Txn<'_> {
    async fn get_task(&mut self, uuid: Uuid) -> Result<Option<TaskMap>> {
        Ok(self.db.tasks.get(&uuid).cloned())
    }

    async fn get_pending_tasks(&mut self) -> Result<Vec<(Uuid, TaskMap)>> {
        Ok(self
            .db
            .working_set
            .iter()
            .flatten()
            .filter_map(|uuid| self.db.tasks.get(uuid).map(|t| (*uuid, t.clone())))
            .collect())
    }

    async fn create_task(&mut self, uuid: Uuid) -> Result<bool> {
        if self.db.tasks.contains_key(&uuid) {
            return Ok(false);
        }
        self.db.tasks.insert(uuid, TaskMap::new());
        self.save_task(uuid, None);
        Ok(true)
    }

    async fn set_task(&mut self, uuid: Uuid, task: TaskMap) -> Result<()> {
        let old = self.db.tasks.insert(uuid, task);
        self.save_task(uuid, old);
        Ok(())
    }

    async fn delete_task(&mut self, uuid: Uuid) -> Result<bool> {
        let old = self.db.tasks.remove(&uuid);
        let existed = old.is_some();
        if existed {
            self.save_task(uuid, old);
        }
        Ok(existed)
    }

    async fn all_tasks(&mut self) -> Result<Vec<(Uuid, TaskMap)>> {
        Ok(self.db.tasks.iter().map(|(u, t)| (*u, t.clone())).collect())
    }

    async fn all_task_uuids(&mut self) -> Result<Vec<Uuid>> {
        Ok(self.db.tasks.keys().copied().collect())
    }

    async fn base_version(&mut self) -> Result<Uuid> {
        Ok(self.db.base_version)
    }

    async fn set_base_version(&mut self, version: Uuid) -> Result<()> {
        let old = std::mem::replace(&mut self.db.base_version, version);
        self.journal.push(Undo::BaseVersion(old));
        Ok(())
    }

    async fn get_task_operations(&mut self, uuid: Uuid) -> Result<Vec<Operation>> {
        Ok(self
            .db
            .operations
            .iter()
            .filter(|(_, op)| op.get_uuid() == Some(uuid))
            .map(|(_, op)| op.clone())
            .collect())
    }

    async fn unsynced_operations(&mut self) -> Result<Vec<Operation>> {
        Ok(self
            .db
            .operations
            .iter()
            .filter(|(synced, _)| !synced)
            .map(|(_, op)| op.clone())
            .collect())
    }

    async fn num_unsynced_operations(&mut self) -> Result<usize> {
        Ok(self.db.operations.iter().filter(|(synced, _)| !synced).count())
    }

    async fn add_operation(&mut self, op: Operation) -> Result<()> {
        self.db.operations.push((false, op));
        match self.journal.last_mut() {
            Some(Undo::OperationsAdded(n)) => *n += 1,
            _ => self.journal.push(Undo::OperationsAdded(1)),
        }
        Ok(())
    }

    async fn remove_operation(&mut self, op: Operation) -> Result<()> {
        match self.db.operations.last() {
            Some((true, _)) => Err(Error::Database(
                "Last operation has been synced -- cannot remove".to_string(),
            )),
            Some((false, last)) if last == &op => {
                if let Some(removed) = self.db.operations.pop() {
                    self.journal.push(Undo::OperationRemoved(removed));
                }
                Ok(())
            }
            _ => Err(Error::Database(
                "Last operation does not match -- cannot remove".to_string(),
            )),
        }
    }

    async fn sync_complete(&mut self) -> Result<()> {
        // Mark every operation as synced, and drop those whose task is gone. Most syncs have nothing to do.
        let tasks = &self.db.tasks;
        let keep = |op: &Operation| op.get_uuid().is_none_or(|u| tasks.contains_key(&u));
        if self.db.operations.iter().all(|(synced, op)| *synced && keep(op)) {
            return Ok(());
        }
        let old = std::mem::take(&mut self.db.operations);
        let mut dropped = Vec::new();
        let mut marked = Vec::new();
        let mut kept = Vec::with_capacity(old.len());
        for (i, (synced, op)) in old.into_iter().enumerate() {
            if keep(&op) {
                if !synced {
                    marked.push(kept.len());
                }
                kept.push((true, op));
            } else {
                dropped.push((i, (synced, op)));
            }
        }
        self.db.operations = kept;
        self.journal.push(Undo::SyncComplete { dropped, marked });
        Ok(())
    }

    async fn get_working_set(&mut self) -> Result<Vec<Option<Uuid>>> {
        Ok(self.db.working_set.clone())
    }

    async fn add_to_working_set(&mut self, uuid: Uuid) -> Result<usize> {
        self.db.working_set.push(Some(uuid));
        match self.journal.last_mut() {
            Some(Undo::WorkingSetAdded(n)) => *n += 1,
            _ => self.journal.push(Undo::WorkingSetAdded(1)),
        }
        Ok(self.db.working_set.len())
    }

    async fn set_working_set_item(&mut self, index: usize, uuid: Option<Uuid>) -> Result<()> {
        if index >= self.db.working_set.len() {
            return Err(Error::Database(format!("Index {index} is not in the working set")));
        }
        let old = std::mem::replace(&mut self.db.working_set[index], uuid);
        self.journal.push(Undo::WorkingSetItem(index, old));
        self.trim_working_set();
        Ok(())
    }

    async fn clear_working_set(&mut self) -> Result<()> {
        let old = std::mem::replace(&mut self.db.working_set, vec![None]);
        self.journal.push(Undo::WorkingSetAll(old));
        Ok(())
    }

    async fn commit(&mut self) -> Result<()> {
        // The changes are already in place; nothing to take back any more.
        self.journal.clear();
        Ok(())
    }
}

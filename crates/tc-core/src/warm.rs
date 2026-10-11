//! The tasks as the engine reads them, kept between requests while nothing has changed.
//!
//! Every command starts by turning every stored task into a [`Facts`] (rebuilding the dependency map, copying each
//! task's properties, converting, sorting): about 2 µs a task, which at 3,000 tasks is most of what a read costs, and
//! a Worker on Cloudflare's free plan has 10 ms of CPU for the whole request. The replica lives on between requests,
//! and most requests change nothing, so the result is worth keeping.
//!
//! What makes that safe is [`Generation`], which the storage bumps whenever a transaction that changed anything
//! ends. A cached list is used only if the number is the one it was built under; any other number, and it is built
//! again. So it can only be wrong if the storage changed without bumping it, which `tests/live_storage.rs` and
//! `tests/warm.rs` look for.

use crate::live::Generation;
use crate::model::Facts;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A cached list, and the generation of the data it was built from.
type Slot = Option<(u64, Arc<Vec<Facts>>)>;

#[derive(Clone)]
pub struct Warm {
    generation: Generation,
    slot: Arc<Mutex<Slot>>,
    patched: Arc<AtomicUsize>,
}

impl Warm {
    pub fn new(generation: Generation) -> Self {
        Warm {
            generation,
            slot: Arc::new(Mutex::new(None)),
            patched: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// The generation the data is at now.
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// The cached list, if nothing has changed since it was built.
    pub fn get(&self) -> Option<Arc<Vec<Facts>>> {
        let slot = self.slot.lock().unwrap_or_else(|p| p.into_inner());
        match &*slot {
            Some((g, facts)) if *g == self.generation.get() => Some(facts.clone()),
            _ => None,
        }
    }

    /// Keep a list built from the data as it was at `generation` (read *before* building it, so that a change in the
    /// middle can only make the list look older than it is, never newer).
    pub fn put(&self, generation: u64, facts: Arc<Vec<Facts>>) {
        *self.slot.lock().unwrap_or_else(|p| p.into_inner()) = Some((generation, facts));
    }
}

impl Warm {
    /// The list that was kept, taken out of the slot, with the generation it was built under: the start of bringing
    /// it up to date. (Taken, so that it is the only holder and can be changed in place.)
    pub fn take(&self) -> Option<(u64, Arc<Vec<Facts>>)> {
        self.slot.lock().unwrap_or_else(|p| p.into_inner()).take()
    }

    /// The generation now, and the tasks changed since `since` (see [`Generation::touched_since`]).
    pub fn changes_since(&self, since: u64) -> Option<(u64, Vec<Uuid>)> {
        self.generation.touched_since(since)
    }

    /// How many times a list was brought up to date instead of being built again.
    pub fn patched(&self) -> usize {
        self.patched.load(Ordering::Relaxed)
    }

    pub(crate) fn count_patch(&self) {
        self.patched.fetch_add(1, Ordering::Relaxed);
    }
}

/// Bring `list` (sorted by entry, then uuid) up to date with these tasks as they are now (`None`: gone). Only when
/// that is all it takes: a task that depends on others, or that others depend on, changes whether those are blocked
/// or blocking, and that is worked out for the whole list, so then `false` is given and the list is built again.
pub fn patch(list: &mut Vec<Facts>, now: Vec<(Uuid, Option<Facts>)>) -> bool {
    let changed = |u: &Uuid| now.iter().any(|(c, _)| c == u);
    let involved = |f: &Facts| !f.depends.is_empty() || f.blocked || f.blocking;
    if now.iter().any(|(_, f)| f.as_ref().is_some_and(involved)) {
        return false;
    }
    if list
        .iter()
        .any(|f| (changed(&f.uuid) && involved(f)) || (!changed(&f.uuid) && f.depends.iter().any(changed)))
    {
        return false;
    }
    for (uuid, fresh) in now {
        if let Some(at) = list.iter().position(|f| f.uuid == uuid) {
            list.remove(at);
        }
        if let Some(f) = fresh {
            let key = (f.entry.unwrap_or(0), f.uuid);
            let at = list.partition_point(|g| (g.entry.unwrap_or(0), g.uuid) < key);
            list.insert(at, f);
        }
    }
    true
}

impl fmt::Debug for Warm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Warm").field("generation", &self.generation()).finish()
    }
}

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
use std::sync::{Arc, Mutex};

/// A cached list, and the generation of the data it was built from.
type Slot = Option<(u64, Arc<Vec<Facts>>)>;

#[derive(Clone)]
pub struct Warm {
    generation: Generation,
    slot: Arc<Mutex<Slot>>,
}

impl Warm {
    pub fn new(generation: Generation) -> Self {
        Warm {
            generation,
            slot: Arc::new(Mutex::new(None)),
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

impl fmt::Debug for Warm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Warm").field("generation", &self.generation()).finish()
    }
}

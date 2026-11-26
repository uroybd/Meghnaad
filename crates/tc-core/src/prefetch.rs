//! A sync in two parts, so the replica is held for as little of the network as can be.
//!
//! TaskChampion's `Replica::sync` asks the store for the child of its base version, then the child of that, until there
//! is none, applying each; then, if it has changes of its own, it adds them as a new version. The asking is most of the
//! time a sync takes with a slow store (a git branch, a request to the forge per step), and the host's replica is held
//! for all of it. [`fetch`] does the asking first, with no replica, from the base version the host read
//! ([`crate::live::BaseVersion`]); [`Prefetched`] is then the store `Replica::sync` is given, answering those questions
//! from what was fetched and passing everything else on. A sync with nothing new and nothing to push then makes no
//! request while the replica is held.
//!
//! If the store moved after the fetch (another replica pushed), the sync still ends as it would have: a pull sees only
//! what was fetched, and the next one brings the rest; a push is refused by the store (`ExpectedParentVersion`), after
//! which [`Prefetched`] forgets what it fetched and TaskChampion's retry asks the store itself.

use async_trait::async_trait;
use std::collections::HashMap;
use taskchampion::server::{
    AddVersionResult, GetVersionResult, HistorySegment, Server, Snapshot, SnapshotUrgency, VersionId,
};
use taskchampion::Error;

/// What the store said after a base version: each version's child, oldest first, and the last has none.
#[derive(Debug, Default)]
pub struct Fetched {
    /// parent -> (child, its history segment).
    children: HashMap<VersionId, (VersionId, HistorySegment)>,
    /// The newest version fetched (the base if there was nothing new): the store said it had no child.
    tip: Option<VersionId>,
}

impl Fetched {
    /// How many versions were fetched.
    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

/// Ask `server` for every version after `base`, as `Replica::sync` would. A replica that has never synced (`base` is
/// the nil version) starts from a snapshot instead, which this does not fetch: the sync then asks the store itself.
pub async fn fetch(server: &mut dyn Server, base: VersionId) -> Result<Fetched, Error> {
    let mut fetched = Fetched::default();
    if base.is_nil() {
        return Ok(fetched);
    }
    let mut at = base;
    loop {
        match server.get_child_version(at).await? {
            GetVersionResult::Version {
                version_id,
                history_segment,
                ..
            } => {
                fetched.children.insert(at, (version_id, history_segment));
                at = version_id;
            }
            GetVersionResult::NoSuchVersion => {
                fetched.tip = Some(at);
                return Ok(fetched);
            }
        }
    }
}

/// The store, with the versions [`fetch`] found in front of it: what is handed to `Replica::sync`.
pub struct Prefetched {
    server: Box<dyn Server>,
    fetched: Fetched,
}

impl Prefetched {
    pub fn boxed(server: Box<dyn Server>, fetched: Fetched) -> Box<dyn Server> {
        Box::new(Prefetched { server, fetched })
    }
}

#[async_trait(?Send)]
impl Server for Prefetched {
    async fn add_version(
        &mut self,
        parent_version_id: VersionId,
        history_segment: HistorySegment,
    ) -> Result<(AddVersionResult, SnapshotUrgency), Error> {
        let answer = self.server.add_version(parent_version_id, history_segment).await?;
        match answer.0 {
            // Refused: the store has moved on since the fetch, so what was fetched no longer says what is newest.
            AddVersionResult::ExpectedParentVersion(_) => self.fetched = Fetched::default(),
            // Taken: TaskChampion then asks for its child before it stops. Nothing is the answer the store would almost
            // always give; if another replica pushed in that moment, the next sync, starting from this version, pulls it.
            AddVersionResult::Ok(version) => self.fetched.tip = Some(version),
        }
        Ok(answer)
    }

    async fn get_child_version(&mut self, parent_version_id: VersionId) -> Result<GetVersionResult, Error> {
        if let Some((child, segment)) = self.fetched.children.remove(&parent_version_id) {
            return Ok(GetVersionResult::Version {
                version_id: child,
                parent_version_id,
                history_segment: segment,
            });
        }
        if self.fetched.tip == Some(parent_version_id) {
            // Answered once: asked again (after a push of ours, say), it is the store's to answer.
            self.fetched.tip = None;
            return Ok(GetVersionResult::NoSuchVersion);
        }
        self.server.get_child_version(parent_version_id).await
    }

    async fn add_snapshot(&mut self, version_id: VersionId, snapshot: Snapshot) -> Result<(), Error> {
        self.server.add_snapshot(version_id, snapshot).await
    }

    async fn get_snapshot(&mut self) -> Result<Option<(VersionId, Snapshot)>, Error> {
        self.server.get_snapshot().await
    }

    async fn latest_version_id(&mut self) -> Result<VersionId, Error> {
        self.server.latest_version_id().await
    }
}

//! [`taskchampion::Server`] over an [`ObjectStore`], wire-compatible with TaskChampion's own
//! cloud server (AWS/GCP backends) so the `task` CLI and this code can share one bucket.
//!
//! Layout: `salt`; `latest` (uuid of newest version, updated by compare-and-swap);
//! `v-<parent>-<version>` (encrypted history segment); `s-<version>` (encrypted snapshot).
//!
//! Snapshots are made the way the CLI makes them: after a push, the server side draws a random
//! "urgency" (about 1% high, 9% low) and the replica writes a snapshot when it is at least the
//! threshold it syncs with. Writing one also removes the snapshots it supersedes. Unlike upstream
//! we do not delete old *versions* (the CLI does that); the only other objects we delete are our
//! own orphaned uploads.
//!
//! Every call here is a network round trip from the Worker to R2, so a sync is built to need few
//! of them: an idle sync is one read of `latest`; catching up lists the versions once and fetches
//! them concurrently, instead of asking one version at a time.

use crate::crypto::{new_uuid, random_bytes, Cryptor};
use crate::error::{Error, Result};
use crate::names::{self, LATEST, SALT};
use crate::store::ObjectStore;
use async_trait::async_trait;
use futures_util::future::join_all;
use std::collections::BTreeMap;
use taskchampion::server::{
    AddVersionResult, GetVersionResult, HistorySegment, Server, Snapshot, SnapshotUrgency, VersionId,
};
use uuid::Uuid;

type TcResult<T> = std::result::Result<T, taskchampion::Error>;

/// How many versions are fetched together while catching up. R2 queues requests beyond what the
/// runtime will run at once, so this only needs to be comfortably more than that.
const PREFETCH: usize = 32;

pub struct CloudServer<S> {
    store: S,
    cryptor: Cryptor,
    seen: Seen,
    /// Fixes the snapshot urgency instead of drawing it at random (tests and tools).
    urgency: Option<SnapshotUrgency>,
}

/// What this server object has learned about the bucket during one sync. Built lazily, and
/// dropped whenever we write, so it never outlives the sync it was made for.
#[derive(Default)]
struct Seen {
    /// The newest version, as last read.
    head: Option<VersionId>,
    /// parent -> children, from one listing of every version object.
    index: Option<BTreeMap<VersionId, Vec<VersionId>>>,
    /// Encrypted version bodies already fetched, by child id.
    bodies: BTreeMap<VersionId, Vec<u8>>,
}

/// The child of `parent` that is on the chain leading to `head`. Several objects can share a
/// parent after races; only the one that is `head` itself, or that has children of its own, is real.
fn next_on_chain(index: &BTreeMap<VersionId, Vec<VersionId>>, parent: VersionId, head: VersionId) -> Option<VersionId> {
    let children = index.get(&parent)?;
    if let Some(c) = children.iter().find(|c| **c == head) {
        return Some(*c);
    }
    children
        .iter()
        .rev()
        .find(|c| index.get(*c).is_some_and(|g| !g.is_empty()))
        .copied()
}

/// Read the bucket's salt, creating it if this is the first client.
pub async fn load_salt<S: ObjectStore>(store: &S) -> Result<Vec<u8>> {
    loop {
        if let Some(salt) = store.get(SALT).await? {
            return Ok(salt);
        }
        let salt = random_bytes::<16>()?;
        store.compare_and_swap(SALT, None, &salt).await?;
    }
}

/// Read the bucket's salt and derive the key from it with this crate's own PBKDF2. Slow (600k
/// rounds); build once and reuse via [`CloudServer::with_cryptor`]. A host with a faster PBKDF2
/// of its own (a browser engine's, a Worker's) can take [`load_salt`] and
/// [`crate::crypto::Cryptor::from_key`] instead.
pub async fn load_cryptor<S: ObjectStore>(store: &S, secret: &[u8]) -> Result<Cryptor> {
    Ok(Cryptor::new(&load_salt(store).await?, secret))
}

fn id_bytes(id: VersionId) -> Vec<u8> {
    id.as_simple().to_string().into_bytes()
}

impl<S: ObjectStore> CloudServer<S> {
    pub fn with_cryptor(store: S, cryptor: Cryptor) -> Self {
        Self {
            store,
            cryptor,
            seen: Seen::default(),
            urgency: None,
        }
    }

    /// Always report this snapshot urgency after a push, instead of drawing one at random.
    pub fn with_snapshot_urgency(mut self, urgency: SnapshotUrgency) -> Self {
        self.urgency = Some(urgency);
        self
    }

    /// The same odds as TaskChampion's own cloud server: a random byte below 2 is "high"
    /// (about 1%), below 25 is "low" (about 9%).
    fn snapshot_urgency(&self) -> Result<SnapshotUrgency> {
        if let Some(u) = self.urgency {
            return Ok(u);
        }
        Ok(match random_bytes::<1>()?[0] {
            0..=1 => SnapshotUrgency::High,
            2..=24 => SnapshotUrgency::Low,
            _ => SnapshotUrgency::None,
        })
    }

    pub async fn new(store: S, secret: &[u8]) -> Result<Self> {
        let cryptor = load_cryptor(&store, secret).await?;
        Ok(Self::with_cryptor(store, cryptor))
    }

    /// The raw `latest` value, which also serves as a cheap cache key for materialized state.
    pub async fn latest(&self) -> Result<Option<VersionId>> {
        match self.store.get(LATEST).await? {
            Some(raw) => parse_latest(&raw).map(Some),
            None => Ok(None),
        }
    }

    /// Every version object in the bucket, in one listing.
    async fn load_index(&self) -> Result<BTreeMap<VersionId, Vec<VersionId>>> {
        let mut index: BTreeMap<VersionId, Vec<VersionId>> = BTreeMap::new();
        for name in self.store.list(names::VERSION_PREFIX).await? {
            if let Some((parent, child)) = names::parse_version_name(&name) {
                index.entry(parent).or_default().push(child);
            }
        }
        Ok(index)
    }

    /// Where each version sits on the chain from the first version up to `latest` (0 for the
    /// first). Versions that are not on it, such as leftovers from a lost race, are absent.
    async fn chain_positions(&mut self) -> Result<BTreeMap<VersionId, usize>> {
        let mut position = BTreeMap::new();
        let Some(head) = self.latest().await? else {
            return Ok(position);
        };
        self.seen.head = Some(head);
        if self.seen.index.is_none() {
            self.seen.index = Some(self.load_index().await?);
        }
        let index = self.seen.index.as_ref().expect("index was just loaded");
        let mut at = Uuid::nil();
        while let Some(next) = next_on_chain(index, at, head) {
            if position.insert(next, position.len()).is_some() {
                break; // a cycle; not something a real bucket has
            }
            at = next;
        }
        Ok(position)
    }

    fn snapshot_versions(names: &[String]) -> Vec<VersionId> {
        names.iter().filter_map(|n| names::parse_snapshot_name(n)).collect()
    }

    /// The newest snapshot, which is the one furthest along the chain: replaying from it is the
    /// shortest. There is normally only one.
    async fn newest_snapshot(&mut self) -> Result<Option<VersionId>> {
        let versions = Self::snapshot_versions(&self.store.list(names::SNAPSHOT_PREFIX).await?);
        match versions.as_slice() {
            [] => Ok(None),
            [only] => Ok(Some(*only)),
            many => {
                let position = self.chain_positions().await?;
                Ok(many
                    .iter()
                    .filter_map(|v| position.get(v).map(|p| (*p, *v)))
                    .max()
                    .map(|(_, v)| v))
            }
        }
    }

    /// Delete the snapshots that the one just written for `version` makes redundant: those
    /// earlier on the chain. Anything else is left alone, including a newer one a CLI may have
    /// written meanwhile.
    async fn drop_superseded_snapshots(&mut self, version: VersionId) -> Result<()> {
        let all = self.store.list(names::SNAPSHOT_PREFIX).await?;
        let others: Vec<VersionId> = Self::snapshot_versions(&all)
            .into_iter()
            .filter(|v| *v != version)
            .collect();
        if others.is_empty() {
            return Ok(());
        }
        let position = self.chain_positions().await?;
        let Some(mine) = position.get(&version).copied() else {
            return Ok(());
        };
        for other in others {
            if position.get(&other).is_some_and(|p| *p < mine) {
                self.store.del(&names::snapshot_name(other)).await?;
            }
        }
        Ok(())
    }

    async fn add_version_inner(&mut self, parent: VersionId, segment: HistorySegment) -> Result<AddVersionResult> {
        // Whatever we learned about the bucket is about to be out of date.
        self.seen = Seen::default();

        // One read gives both the head and the tag that the swap below is conditional on.
        let (latest, tag) = match self.store.get_tagged(LATEST).await? {
            Some((raw, tag)) => (Some(parse_latest(&raw)?), Some(tag)),
            None => (None, None),
        };
        if let Some(l) = latest {
            if l != parent {
                return Ok(AddVersionResult::ExpectedParentVersion(l));
            }
        }

        // Upload under a fresh id first; it only becomes visible once `latest` points at it.
        let version = new_uuid()?;
        let name = names::version_name(parent, version);
        let sealed = self.cryptor.seal(version, &segment)?;
        self.store.put(&name, &sealed).await?;

        if !self
            .store
            .swap_tagged(LATEST, tag.as_deref(), &id_bytes(version))
            .await?
        {
            // Lost the race; our upload is an orphan.
            self.store.del(&name).await?;
            let latest = self.latest().await?.unwrap_or(Uuid::nil());
            return Ok(AddVersionResult::ExpectedParentVersion(latest));
        }
        Ok(AddVersionResult::Ok(version))
    }

    async fn get_child_inner(&mut self, parent: VersionId) -> Result<GetVersionResult> {
        // Is there anything newer than `parent`? While walking a chain the head read at the start
        // is reused; once `parent` reaches it, look again. That is how versions added meanwhile
        // are noticed, and it is the whole cost of a sync with nothing new: one read, no listing.
        let head = match self.seen.head {
            Some(h) if h != parent => h,
            _ => {
                self.seen.head = self.latest().await?;
                match self.seen.head {
                    Some(h) => h,
                    None => return Ok(GetVersionResult::NoSuchVersion),
                }
            }
        };
        if head == parent {
            return Ok(GetVersionResult::NoSuchVersion);
        }

        // Which child continues the chain? Answered from one listing of the bucket. If that
        // listing predates the version we need (a writer got in after it), list again once.
        let mut listed_now = false;
        let version = loop {
            if self.seen.index.is_none() {
                self.seen.index = Some(self.load_index().await?);
                listed_now = true;
            }
            let index = self.seen.index.as_ref().expect("index was just loaded");
            if let Some(v) = next_on_chain(index, parent, head) {
                break v;
            }
            if listed_now {
                return Ok(GetVersionResult::NoSuchVersion);
            }
            self.seen.index = None;
        };

        if !self.seen.bodies.contains_key(&version) {
            self.prefetch(parent, version, head).await?;
        }
        let Some(sealed) = self.seen.bodies.remove(&version) else {
            return Ok(GetVersionResult::NoSuchVersion);
        };
        Ok(GetVersionResult::Version {
            version_id: version,
            parent_version_id: parent,
            history_segment: self.cryptor.unseal(version, &sealed)?,
        })
    }

    /// Fetch `version` and the next stretch of the chain after it together, so the following
    /// calls are answered from memory.
    async fn prefetch(&mut self, parent: VersionId, version: VersionId, head: VersionId) -> Result<()> {
        let mut want = vec![(parent, version)];
        if let Some(index) = &self.seen.index {
            let mut at = version;
            while want.len() < PREFETCH {
                let Some(next) = next_on_chain(index, at, head) else {
                    break;
                };
                want.push((at, next));
                at = next;
            }
        }
        let store = &self.store;
        let fetched = join_all(
            want.iter()
                .map(|(p, c)| async move { (*c, store.get(&names::version_name(*p, *c)).await) }),
        )
        .await;
        for (child, body) in fetched {
            if let Some(body) = body? {
                self.seen.bodies.insert(child, body);
            }
        }
        Ok(())
    }
}

fn parse_latest(raw: &[u8]) -> Result<VersionId> {
    Uuid::try_parse_ascii(raw).map_err(|_| Error::Corrupt("'latest' object contains invalid data".into()))
}

#[async_trait(?Send)]
impl<S: ObjectStore> Server for CloudServer<S> {
    async fn add_version(
        &mut self,
        parent_version_id: VersionId,
        history_segment: HistorySegment,
    ) -> TcResult<(AddVersionResult, SnapshotUrgency)> {
        let res = self.add_version_inner(parent_version_id, history_segment).await?;
        let urgency = match res {
            AddVersionResult::Ok(_) => self.snapshot_urgency()?,
            AddVersionResult::ExpectedParentVersion(_) => SnapshotUrgency::None,
        };
        Ok((res, urgency))
    }

    async fn get_child_version(&mut self, parent_version_id: VersionId) -> TcResult<GetVersionResult> {
        Ok(self.get_child_inner(parent_version_id).await?)
    }

    async fn add_snapshot(&mut self, version_id: VersionId, snapshot: Snapshot) -> TcResult<()> {
        let sealed = self.cryptor.seal(version_id, &snapshot)?;
        // A snapshot only makes the next cold start shorter, and the version it describes is
        // already on the server. Failing the sync here would leave the replica believing that
        // version was never sent, and it would send it again. So problems are not reported.
        let wrote = self.store.put(&names::snapshot_name(version_id), &sealed).await.is_ok();
        if wrote {
            let _ = self.drop_superseded_snapshots(version_id).await;
        }
        self.seen = Seen::default();
        Ok(())
    }

    async fn get_snapshot(&mut self) -> TcResult<Option<(VersionId, Snapshot)>> {
        let Some(version) = self.newest_snapshot().await? else {
            return Ok(None);
        };
        let Some(sealed) = self.store.get(&names::snapshot_name(version)).await? else {
            return Ok(None);
        };
        Ok(Some((version, self.cryptor.unseal(version, &sealed)?)))
    }
}

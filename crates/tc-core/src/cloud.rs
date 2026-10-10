//! [`taskchampion::Server`] over an [`ObjectStore`], wire-compatible with TaskChampion's own
//! cloud server (AWS/GCP backends) so the `task` CLI and this code can share one bucket.
//!
//! Layout: `salt`; `latest` (uuid of newest version, updated by compare-and-swap);
//! `v-<parent>-<version>` (encrypted history segment); `s-<version>` (encrypted snapshot).
//!
//! Snapshots are made the way the CLI makes them: after a push, the server side draws a random
//! "urgency" (about 1% high, 9% low) and the replica writes a snapshot when it is at least the
//! threshold it syncs with. Writing one also removes the snapshots it supersedes.
//!
//! Like TaskChampion's own server, one push in about twenty is followed by a cleanup (when the host
//! says what time it is, see [`CloudServer::with_cleanup`]): versions that lost a race, all but the
//! newest snapshot, and versions older than 180 days that a snapshot covers are deleted, so that a
//! bucket the web app writes to does not keep its whole history forever.
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

/// A version this old that a snapshot covers is of no use to anyone: TaskChampion deletes it.
const MAX_VERSION_AGE_SECS: i64 = 3600 * 24 * 180;

/// How many objects one cleanup deletes (R2 deletes this many in a single request). What is left waits for the
/// next cleanup.
const MAX_DELETES: usize = 1000;

/// Out of 256: how likely a push is to be followed by a cleanup (13 is about 5%, as in TaskChampion).
const CLEANUP_ODDS: u8 = 13;

pub struct CloudServer<S> {
    store: S,
    cryptor: Cryptor,
    seen: Seen,
    /// Fixes the snapshot urgency instead of drawing it at random (tests and tools).
    urgency: Option<SnapshotUrgency>,
    /// The time, in seconds since the epoch, when cleanups are wanted; `None` never cleans up.
    clock: Option<fn() -> i64>,
    cleanup_odds: u8,
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
            clock: None,
            cleanup_odds: CLEANUP_ODDS,
        }
    }

    /// Tidy the bucket now and then, after a push, as TaskChampion's own server does (see
    /// [`CloudServer::cleanup`]). `now` is the time in seconds since the epoch. Without it, nothing is deleted.
    pub fn with_cleanup(mut self, now: fn() -> i64) -> Self {
        self.clock = Some(now);
        self
    }

    /// How likely a push is to be followed by a cleanup, out of 256 (0 never, 255 nearly always). For tests.
    pub fn with_cleanup_odds(mut self, odds: u8) -> Self {
        self.cleanup_odds = odds;
        self
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

    /// Where each version sits on the chain up to `latest`: a number that grows along it, so that a later
    /// version has a bigger one. Versions that are not on it, such as leftovers from a lost race, are absent.
    /// The chain is followed back from `latest`, since its start may be gone: versions that a snapshot
    /// covers are deleted once they are old.
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
        // (Inserted one by one: collecting into a map sorts first, which is more code.)
        let mut parent_of: BTreeMap<VersionId, VersionId> = BTreeMap::new();
        for (parent, children) in index {
            for child in children {
                parent_of.insert(*child, *parent);
            }
        }
        let mut back = vec![head];
        while let Some(parent) = parent_of.get(back.last().expect("it starts with the head")) {
            if back.len() > parent_of.len() {
                break; // a cycle; not something a real bucket has
            }
            back.push(*parent);
        }
        for (steps, version) in back.iter().enumerate() {
            position.insert(*version, back.len() - steps);
        }
        Ok(position)
    }

    /// Delete what no replica needs any more, as TaskChampion's own server does: versions that are not on the
    /// chain to `latest` (leftovers of lost races, except one a writer may be about to make `latest`), every
    /// snapshot but the newest on the chain, and versions older than 180 days that this snapshot covers.
    /// At most [`MAX_DELETES`] objects go in one call, in one request to the store; the next call carries on. Of the
    /// old versions the oldest go first, so that what is left is always one unbroken chain up to `latest`: a replica
    /// that is behind then finds either the next version or none at all, never a gap with versions beyond it.
    /// Returns how many were deleted.
    pub async fn cleanup(&mut self, now: i64) -> Result<usize> {
        let Some(head) = self.latest().await? else {
            return Ok(0);
        };
        // (parent, child, uploaded) of every version object, and the chain back from `latest`.
        let versions: Vec<(VersionId, VersionId, i64)> = self
            .store
            .list_dated(names::VERSION_PREFIX)
            .await?
            .into_iter()
            .filter_map(|(name, at)| names::parse_version_name(&name).map(|(p, c)| (p, c, at)))
            .collect();
        let mut parent_of: BTreeMap<VersionId, VersionId> = BTreeMap::new();
        let mut uploaded: BTreeMap<VersionId, i64> = BTreeMap::new();
        for (parent, child, at) in &versions {
            parent_of.insert(*child, *parent);
            uploaded.insert(*child, *at);
        }
        let mut chain: BTreeMap<VersionId, VersionId> = BTreeMap::new();
        let mut at = head;
        while let Some(parent) = parent_of.get(&at) {
            if chain.insert(at, *parent).is_some() {
                return Err(Error::Corrupt("the versions form a cycle".into()));
            }
            at = *parent;
        }

        let mut doomed: Vec<String> = versions
            .iter()
            .filter(|(p, c, _)| chain.get(c) != Some(p) && *p != head)
            .map(|(p, c, _)| names::version_name(*p, *c))
            .collect();

        // The newest snapshot is the first one found going back from `latest`; the others are not needed.
        let snapshots = Self::snapshot_versions(&self.store.list(names::SNAPSHOT_PREFIX).await?);
        let mut newest = None;
        let mut at = head;
        loop {
            if snapshots.contains(&at) {
                newest = Some(at);
                break;
            }
            match chain.get(&at) {
                Some(parent) => at = *parent,
                None => break,
            }
        }
        if let Some(newest) = newest {
            doomed.extend(
                snapshots
                    .iter()
                    .filter(|s| **s != newest)
                    .map(|s| names::snapshot_name(*s)),
            );
            // Versions the snapshot covers (it and those before it) are old enough to go. Found newest first, so
            // reversed: the oldest go first.
            let mut old = Vec::new();
            let mut at = newest;
            while let Some(parent) = chain.get(&at) {
                if uploaded.get(&at).is_some_and(|t| *t < now - MAX_VERSION_AGE_SECS) {
                    old.push(names::version_name(*parent, at));
                }
                at = *parent;
            }
            doomed.extend(old.into_iter().rev());
        }

        doomed.truncate(MAX_DELETES);
        self.store.del_many(&doomed).await?;
        Ok(doomed.len())
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
        if let (AddVersionResult::Ok(_), Some(now)) = (&res, self.clock) {
            if random_bytes::<1>().is_ok_and(|b| b[0] < self.cleanup_odds) {
                // Best effort: the push has happened, and a cleanup that fails is only tried again later.
                let _ = self.cleanup(now()).await;
                self.seen = Seen::default();
            }
        }
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

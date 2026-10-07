//! [`taskchampion::Server`] over an [`ObjectStore`], wire-compatible with TaskChampion's own
//! cloud server (AWS/GCP backends) so the `task` CLI and this code can share one bucket.
//!
//! Layout: `salt`; `latest` (uuid of newest version, updated by compare-and-swap);
//! `v-<parent>-<version>` (encrypted history segment); `s-<version>` (encrypted snapshot).
//!
//! Unlike upstream we never run cleanup and never ask for snapshots: the CLI does both, and
//! a web replica should stay cheap. The only objects we delete are our own orphaned uploads.

use crate::crypto::{new_uuid, random_bytes, Cryptor};
use crate::error::{Error, Result};
use crate::names::{self, LATEST, SALT};
use crate::store::ObjectStore;
use async_trait::async_trait;
use taskchampion::server::{
    AddVersionResult, GetVersionResult, HistorySegment, Server, Snapshot, SnapshotUrgency,
    VersionId,
};
use uuid::Uuid;

type TcResult<T> = std::result::Result<T, taskchampion::Error>;

pub struct CloudServer<S> {
    store: S,
    cryptor: Cryptor,
}

/// Read the bucket's salt, creating it if this is the first client, and derive the key.
/// Slow (PBKDF2); build once and reuse via [`CloudServer::with_cryptor`].
pub async fn load_cryptor<S: ObjectStore>(store: &S, secret: &[u8]) -> Result<Cryptor> {
    loop {
        if let Some(salt) = store.get(SALT).await? {
            return Ok(Cryptor::new(&salt, secret));
        }
        let salt = random_bytes::<16>()?;
        store.compare_and_swap(SALT, None, &salt).await?;
    }
}

fn id_bytes(id: VersionId) -> Vec<u8> {
    id.as_simple().to_string().into_bytes()
}

impl<S: ObjectStore> CloudServer<S> {
    pub fn with_cryptor(store: S, cryptor: Cryptor) -> Self {
        Self { store, cryptor }
    }

    pub async fn new(store: S, secret: &[u8]) -> Result<Self> {
        let cryptor = load_cryptor(&store, secret).await?;
        Ok(Self::with_cryptor(store, cryptor))
    }

    /// The raw `latest` value, which also serves as a cheap cache key for materialized state.
    pub async fn latest(&self) -> Result<Option<VersionId>> {
        let Some(raw) = self.store.get(LATEST).await? else {
            return Ok(None);
        };
        Uuid::try_parse_ascii(&raw)
            .map(Some)
            .map_err(|_| Error::Corrupt("'latest' object contains invalid data".into()))
    }

    async fn children(&self, parent: VersionId) -> Result<Vec<VersionId>> {
        Ok(self
            .store
            .list(&names::child_prefix(parent))
            .await?
            .iter()
            .filter_map(|n| names::parse_version_name(n).map(|(_, c)| c))
            .collect())
    }

    async fn snapshot_version(&self) -> Result<Option<VersionId>> {
        Ok(self
            .store
            .list(names::SNAPSHOT_PREFIX)
            .await?
            .iter()
            .find_map(|n| names::parse_snapshot_name(n)))
    }

    async fn add_version_inner(
        &self,
        parent: VersionId,
        segment: HistorySegment,
    ) -> Result<AddVersionResult> {
        let latest = self.latest().await?;
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

        let expected = latest.map(id_bytes);
        if !self
            .store
            .compare_and_swap(LATEST, expected.as_deref(), &id_bytes(version))
            .await?
        {
            // Lost the race; our upload is an orphan.
            self.store.del(&name).await?;
            let latest = self.latest().await?.unwrap_or(Uuid::nil());
            return Ok(AddVersionResult::ExpectedParentVersion(latest));
        }
        Ok(AddVersionResult::Ok(version))
    }

    async fn get_child_inner(&self, parent: VersionId) -> Result<GetVersionResult> {
        // Several objects may share a parent after races; only the one on the chain to `latest`
        // is real. That is `latest` itself, or the one that has children of its own.
        let children = self.children(parent).await?;
        if children.is_empty() {
            return Ok(GetVersionResult::NoSuchVersion);
        }
        let latest = self.latest().await?;
        let mut true_child = children.iter().copied().find(|c| Some(*c) == latest);
        if true_child.is_none() {
            for child in &children {
                if !self.children(*child).await?.is_empty() {
                    true_child = Some(*child);
                }
            }
        }
        let Some(version) = true_child else {
            return Ok(GetVersionResult::NoSuchVersion);
        };
        let Some(sealed) = self.store.get(&names::version_name(parent, version)).await? else {
            return Ok(GetVersionResult::NoSuchVersion);
        };
        Ok(GetVersionResult::Version {
            version_id: version,
            parent_version_id: parent,
            history_segment: self.cryptor.unseal(version, &sealed)?,
        })
    }
}

#[async_trait(?Send)]
impl<S: ObjectStore> Server for CloudServer<S> {
    async fn add_version(
        &mut self,
        parent_version_id: VersionId,
        history_segment: HistorySegment,
    ) -> TcResult<(AddVersionResult, SnapshotUrgency)> {
        let res = self.add_version_inner(parent_version_id, history_segment).await?;
        Ok((res, SnapshotUrgency::None))
    }

    async fn get_child_version(
        &mut self,
        parent_version_id: VersionId,
    ) -> TcResult<GetVersionResult> {
        Ok(self.get_child_inner(parent_version_id).await?)
    }

    async fn add_snapshot(
        &mut self,
        version_id: VersionId,
        snapshot: Snapshot,
    ) -> TcResult<()> {
        let sealed = self.cryptor.seal(version_id, &snapshot).map_err(Error::from)?;
        self.store
            .put(&names::snapshot_name(version_id), &sealed)
            .await?;
        Ok(())
    }

    async fn get_snapshot(&mut self) -> TcResult<Option<(VersionId, Snapshot)>> {
        let Some(version) = self.snapshot_version().await? else {
            return Ok(None);
        };
        let Some(sealed) = self.store.get(&names::snapshot_name(version)).await? else {
            return Ok(None);
        };
        Ok(Some((version, self.cryptor.unseal(version, &sealed)?)))
    }
}

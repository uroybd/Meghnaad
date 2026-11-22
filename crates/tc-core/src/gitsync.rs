//! [`taskchampion::Server`] over a git branch, file for file what TaskChampion's own git server (`sync.git.*`, its
//! `server/gitsync`) writes, so the `task` CLI, the web app and the phone can share one branch.
//!
//! Layout, at the root of the branch:
//! - `meta`: JSON `{"latest_version": <simple uuid>, "salt": <base64>}`, the newest version and the key's salt.
//! - `v-<parent>-<child>` (simple uuids): a history segment, sealed with the child's id, as in a bucket.
//! - `snapshot`: JSON `{"version_id": <simple uuid>, "payload": <base64 sealed snapshot>}`, overwritten by the next.
//!
//! A version is added in one commit that holds its file and the new `meta`, made only if the branch has not moved since
//! `meta` was read (the check a rejected `git push` is to the CLI). How much a snapshot is wanted is counted, as the CLI
//! counts it: the versions since the snapshot. After a snapshot, the versions it covers that are older than 180 days are
//! deleted, in one commit.
//!
//! The branch is reached through a [`GitRepo`], which a host builds over a forge's API (`crate::forge`); this module
//! does no I/O of its own. Settings files the web app keeps (`web/…`) live on the same branch, through [`GitObjects`].

use crate::crypto::{new_uuid, random_bytes, Cryptor};
use crate::error::{Error, Result};
use crate::names;
use crate::store::ObjectStore;
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use taskchampion::server::{
    AddVersionResult, GetVersionResult, HistorySegment, Server, Snapshot, SnapshotUrgency, VersionId,
};
use uuid::Uuid;

type TcResult<T> = std::result::Result<T, taskchampion::Error>;

/// The file holding the newest version and the salt.
pub const META: &str = "meta";
/// The file holding the one snapshot.
pub const SNAPSHOT: &str = "snapshot";

/// A version this old that the snapshot covers is deleted, as the CLI does (`VERSION_RETENTION`).
const MAX_VERSION_AGE_SECS: i64 = 180 * 24 * 3600;
/// How many versions are fetched together while catching up.
const PREFETCH: usize = 32;
/// How many times a write is tried again when the branch moved under it for a reason that does not matter to it (a
/// settings file saved meanwhile).
const RETRIES: usize = 4;

/// One file on the branch: its path from the root and its blob id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub id: String,
}

/// A file read from the branch, with its blob id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub id: String,
    pub bytes: Vec<u8>,
}

/// One change in a commit. `was` is the blob id the file is expected to have, so a forge that checks files (Gitea,
/// Forgejo) refuses the commit when somebody changed it meanwhile; GitHub checks the whole branch instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Create { path: String, bytes: Vec<u8> },
    Update { path: String, bytes: Vec<u8>, was: String },
    Delete { path: String, was: String },
}

impl Change {
    /// The file it changes.
    pub fn path(&self) -> &str {
        match self {
            Change::Create { path, .. } | Change::Update { path, .. } | Change::Delete { path, .. } => path,
        }
    }
}

/// What came of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Committed {
    /// Made; the branch's new head.
    Done(String),
    /// Not made: the branch, or a file the commit expected, changed since it was read (or the branch was only just
    /// created). Read again and decide.
    Moved,
}

/// A branch of a git repository, as far as this module needs one. `head` is the commit the branch points at, `None`
/// when there is no such branch yet; a commit is based on the head its changes were worked out from.
#[expect(async_fn_in_trait, reason = "Workers futures are !Send, so no Send bound is wanted")]
pub trait GitRepo {
    /// The head and the file at `path`, read from the same commit.
    async fn read(&self, path: &str) -> Result<(Option<String>, Option<Blob>)>;
    /// The head and the files directly at the root (not those in folders).
    async fn root(&self) -> Result<(Option<String>, Vec<Entry>)>;
    /// Every file under the folder `dir` (no trailing slash), however deep, by its path from the root.
    async fn tree(&self, dir: &str) -> Result<Vec<Entry>>;
    /// A file's contents by its blob id.
    async fn blob(&self, id: &str) -> Result<Vec<u8>>;
    /// Every file under the folder `dir` with its contents, by its path from the root, when the forge can give them in
    /// a few requests (GitHub's GraphQL); `None` where it can't, and the folder is read with [`tree`](Self::tree) and
    /// [`blob`](Self::blob) instead.
    async fn read_dir(&self, _dir: &str) -> Result<Option<Vec<(String, Blob)>>> {
        Ok(None)
    }
    /// Make one commit of `changes` on top of `head`.
    async fn commit(&self, head: Option<&str>, message: &str, changes: &[Change]) -> Result<Committed>;
    /// When each of `paths` was last changed on the branch (seconds since the epoch), `None` if never.
    async fn last_changed(&self, paths: &[String]) -> Result<Vec<Option<i64>>>;
    /// How many paths one call of [`last_changed`](Self::last_changed) should be given at most.
    fn history_batch(&self) -> usize {
        20
    }
}

impl<R: GitRepo> GitRepo for &R {
    async fn read(&self, path: &str) -> Result<(Option<String>, Option<Blob>)> {
        (**self).read(path).await
    }
    async fn root(&self) -> Result<(Option<String>, Vec<Entry>)> {
        (**self).root().await
    }
    async fn tree(&self, dir: &str) -> Result<Vec<Entry>> {
        (**self).tree(dir).await
    }
    async fn blob(&self, id: &str) -> Result<Vec<u8>> {
        (**self).blob(id).await
    }
    async fn read_dir(&self, dir: &str) -> Result<Option<Vec<(String, Blob)>>> {
        (**self).read_dir(dir).await
    }
    async fn commit(&self, head: Option<&str>, message: &str, changes: &[Change]) -> Result<Committed> {
        (**self).commit(head, message, changes).await
    }
    async fn last_changed(&self, paths: &[String]) -> Result<Vec<Option<i64>>> {
        (**self).last_changed(paths).await
    }
    fn history_batch(&self) -> usize {
        (**self).history_batch()
    }
}

/// `meta`, as the CLI writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub latest_version: VersionId,
    pub salt: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct RawMeta {
    latest_version: String,
    salt: String,
}

impl Meta {
    pub fn parse(bytes: &[u8]) -> Result<Meta> {
        let bad = || Error::Corrupt("the branch's `meta` file is not what TaskChampion writes".into());
        let raw: RawMeta = serde_json::from_slice(bytes).map_err(|_| bad())?;
        Ok(Meta {
            latest_version: Uuid::try_parse(&raw.latest_version).map_err(|_| bad())?,
            salt: STANDARD.decode(raw.salt).map_err(|_| bad())?,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(&RawMeta {
            latest_version: self.latest_version.as_simple().to_string(),
            salt: STANDARD.encode(&self.salt),
        })
        .map_err(|e| Error::Corrupt(e.to_string()))
    }
}

#[derive(Serialize, Deserialize)]
struct RawSnapshot {
    version_id: String,
    payload: String,
}

/// `snapshot`: the version it is of, and the sealed snapshot.
fn parse_snapshot(bytes: &[u8]) -> Result<(VersionId, Vec<u8>)> {
    let bad = || Error::Corrupt("the branch's `snapshot` file is not what TaskChampion writes".into());
    let raw: RawSnapshot = serde_json::from_slice(bytes).map_err(|_| bad())?;
    Ok((
        Uuid::try_parse(&raw.version_id).map_err(|_| bad())?,
        STANDARD.decode(raw.payload).map_err(|_| bad())?,
    ))
}

fn snapshot_bytes(version: VersionId, sealed: &[u8]) -> Result<Vec<u8>> {
    serde_json::to_vec(&RawSnapshot {
        version_id: version.as_simple().to_string(),
        payload: STANDARD.encode(sealed),
    })
    .map_err(|e| Error::Corrupt(e.to_string()))
}

/// Read the branch's salt, writing `meta` (no versions yet, a new salt) if this is the first replica, as the CLI does.
pub async fn load_salt<R: GitRepo>(repo: &R) -> Result<Vec<u8>> {
    for _ in 0..RETRIES {
        let (head, meta) = repo.read(META).await?;
        if let Some(meta) = meta {
            return Ok(Meta::parse(&meta.bytes)?.salt);
        }
        let meta = Meta {
            latest_version: Uuid::nil(),
            salt: random_bytes::<16>()?.to_vec(),
        };
        let change = Change::Create {
            path: META.into(),
            bytes: meta.to_bytes()?,
        };
        if let Committed::Done(_) = repo
            .commit(head.as_deref(), "init taskchampion repo", &[change])
            .await?
        {
            return Ok(meta.salt);
        }
    }
    Err(Error::TooMuchContention)
}

/// Read the salt and derive the key with this crate's own PBKDF2 (slow; a host with a faster one takes [`load_salt`]
/// and [`Cryptor::from_key`]).
pub async fn load_cryptor<R: GitRepo>(repo: &R, secret: &[u8]) -> Result<Cryptor> {
    Ok(Cryptor::new(&load_salt(repo).await?, secret))
}

thread_local! {
    /// The version the snapshot file with this blob id is of: reading it means downloading the whole snapshot, and it
    /// changes rarely, so it is remembered between requests.
    static SNAPSHOT_OF: RefCell<Option<(String, VersionId)>> = const { RefCell::new(None) };
}

/// What this server learned about the branch during one sync; dropped whenever it writes.
#[derive(Default)]
struct Seen {
    /// The newest version, as last read from `meta`.
    latest: Option<VersionId>,
    /// The files at the root, from one listing.
    files: Option<Vec<Entry>>,
    /// Sealed version bodies fetched ahead, by child id.
    bodies: BTreeMap<VersionId, Vec<u8>>,
}

pub struct GitServer<R> {
    repo: R,
    cryptor: Cryptor,
    seen: Seen,
    /// The time, in seconds since the epoch, for the cleanup after a snapshot; `None` never cleans up.
    clock: Option<fn() -> i64>,
}

/// The versions among `files`: child -> (parent, blob id).
fn versions(files: &[Entry]) -> BTreeMap<VersionId, (VersionId, String)> {
    files
        .iter()
        .filter_map(|e| names::parse_version_name(&e.path).map(|(p, c)| (c, (p, e.id.clone()))))
        .collect()
}

impl<R: GitRepo> GitServer<R> {
    pub fn with_cryptor(repo: R, cryptor: Cryptor) -> Self {
        GitServer {
            repo,
            cryptor,
            seen: Seen::default(),
            clock: None,
        }
    }

    pub async fn new(repo: R, secret: &[u8]) -> Result<Self> {
        let cryptor = load_cryptor(&repo, secret).await?;
        Ok(Self::with_cryptor(repo, cryptor))
    }

    /// Delete old versions after a snapshot, as the CLI does. `now` is the time in seconds since the epoch.
    pub fn with_cleanup(mut self, now: fn() -> i64) -> Self {
        self.clock = Some(now);
        self
    }

    /// The newest version, `None` before `meta` exists.
    pub async fn latest(&self) -> Result<Option<VersionId>> {
        let (_, meta) = self.repo.read(META).await?;
        meta.map(|m| Meta::parse(&m.bytes).map(|m| m.latest_version))
            .transpose()
    }

    /// The version the snapshot file `entry` is of.
    async fn snapshot_version(&self, entry: &Entry) -> Result<VersionId> {
        if let Some(v) = SNAPSHOT_OF.with(|s| s.borrow().as_ref().filter(|(id, _)| *id == entry.id).map(|(_, v)| *v)) {
            return Ok(v);
        }
        let (version, _) = parse_snapshot(&self.repo.blob(&entry.id).await?)?;
        SNAPSHOT_OF.with(|s| *s.borrow_mut() = Some((entry.id.clone(), version)));
        Ok(version)
    }

    /// How much a snapshot is wanted now that `latest` was added: the versions since the snapshot, counted back from
    /// `latest` (none up to 50, low up to 100, high beyond), as the CLI counts.
    async fn urgency(&self, latest: VersionId) -> Result<SnapshotUrgency> {
        let (_, files) = self.repo.root().await?;
        let snapshot = match files.iter().find(|e| e.path == SNAPSHOT) {
            Some(entry) => self.snapshot_version(entry).await?,
            None => Uuid::nil(),
        };
        let parent_of = versions(&files);
        let mut count = 0usize;
        let mut at = latest;
        for _ in 0..=parent_of.len() {
            if at == snapshot || at.is_nil() {
                break;
            }
            match parent_of.get(&at) {
                Some((parent, _)) => {
                    count += 1;
                    at = *parent;
                }
                None => break,
            }
        }
        Ok(match count {
            0..=50 => SnapshotUrgency::None,
            51..=100 => SnapshotUrgency::Low,
            _ => SnapshotUrgency::High,
        })
    }

    async fn add_version_inner(&mut self, parent: VersionId, segment: &[u8]) -> Result<AddVersionResult> {
        for _ in 0..RETRIES {
            self.seen = Seen::default();
            let (head, meta_file) = self.repo.read(META).await?;
            let Some(meta_file) = meta_file else {
                return Err(Error::Corrupt("the branch has no `meta` file".into()));
            };
            let meta = Meta::parse(&meta_file.bytes)?;
            // Any parent is taken while there are no versions, as the CLI does.
            if !meta.latest_version.is_nil() && meta.latest_version != parent {
                return Ok(AddVersionResult::ExpectedParentVersion(meta.latest_version));
            }
            let version = new_uuid()?;
            let changes = [
                Change::Create {
                    path: names::version_name(parent, version),
                    bytes: self.cryptor.seal(version, segment)?,
                },
                Change::Update {
                    path: META.into(),
                    bytes: Meta {
                        latest_version: version,
                        salt: meta.salt,
                    }
                    .to_bytes()?,
                    was: meta_file.id,
                },
            ];
            // On `Moved`, read `meta` again: another version means this one is behind; anything else (a settings file
            // saved meanwhile) is no reason not to add it.
            if let Committed::Done(_) = self.repo.commit(head.as_deref(), "add version", &changes).await? {
                return Ok(AddVersionResult::Ok(version));
            }
        }
        Err(Error::TooMuchContention)
    }

    async fn get_child_inner(&mut self, parent: VersionId) -> Result<GetVersionResult> {
        // Is there anything after `parent`? One small read answers that, and it is the whole cost of a sync with
        // nothing new. While walking a chain the value read at the start is reused; once `parent` reaches it, look
        // again, to notice versions added meanwhile.
        let latest = match self.seen.latest {
            Some(l) if l != parent => l,
            _ => {
                self.seen.latest = self.latest().await?;
                match self.seen.latest {
                    Some(l) => l,
                    None => return Ok(GetVersionResult::NoSuchVersion),
                }
            }
        };
        if latest == parent || latest.is_nil() {
            return Ok(GetVersionResult::NoSuchVersion);
        }

        // Which file continues the chain? Answered from one listing; if that predates the version (a writer got in
        // after it), list again once.
        let mut listed_now = false;
        let (version, id) = loop {
            let files = match self.seen.files.take() {
                Some(f) => f,
                None => {
                    listed_now = true;
                    self.repo.root().await?.1
                }
            };
            let found = next_on_chain(&versions(&files), parent, latest);
            self.seen.files = Some(files);
            if let Some(found) = found {
                break found;
            }
            if listed_now {
                return Ok(GetVersionResult::NoSuchVersion);
            }
            self.seen.files = None;
        };

        if !self.seen.bodies.contains_key(&version) {
            self.prefetch(version, id, latest).await?;
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

    /// Fetch `version` and the next stretch of the chain after it together, so the following calls are answered from
    /// memory.
    async fn prefetch(&mut self, version: VersionId, id: String, latest: VersionId) -> Result<()> {
        let mut want = vec![(version, id)];
        if let Some(files) = &self.seen.files {
            let all = versions(files);
            let mut at = version;
            while want.len() < PREFETCH {
                let Some(next) = next_on_chain(&all, at, latest) else {
                    break;
                };
                at = next.0;
                want.push(next);
            }
        }
        let repo = &self.repo;
        let fetched = join_all(want.iter().map(|(v, id)| async move { (*v, repo.blob(id).await) })).await;
        for (version, body) in fetched {
            self.seen.bodies.insert(version, body?);
        }
        Ok(())
    }

    async fn add_snapshot_inner(&mut self, version: VersionId, snapshot: &[u8]) -> Result<()> {
        self.seen = Seen::default();
        let bytes = snapshot_bytes(version, &self.cryptor.seal(version, snapshot)?)?;
        for _ in 0..RETRIES {
            let (head, files) = self.repo.root().await?;
            let change = match files.iter().find(|e| e.path == SNAPSHOT) {
                Some(old) => Change::Update {
                    path: SNAPSHOT.into(),
                    bytes: bytes.clone(),
                    was: old.id.clone(),
                },
                None => Change::Create {
                    path: SNAPSHOT.into(),
                    bytes: bytes.clone(),
                },
            };
            if let Committed::Done(head) = self.repo.commit(head.as_deref(), "add snapshot", &[change]).await? {
                if let Some(now) = self.clock {
                    // Best effort, as in the CLI: the snapshot is stored, and versions left now go with the next one.
                    self.cleanup(&head, &files, version, now()).await.ok();
                }
                return Ok(());
            }
        }
        Err(Error::TooMuchContention)
    }

    /// Delete the versions the snapshot of `snapshot` covers (it and those before it on the chain) that were last
    /// changed more than 180 days ago, in one commit. Oldest first, and only as many as one round of
    /// [`GitRepo::last_changed`] answers for; the rest go after the next snapshot. Returns how many were deleted.
    async fn cleanup(&self, head: &str, files: &[Entry], snapshot: VersionId, now: i64) -> Result<usize> {
        let all = versions(files);
        let mut covered = Vec::new();
        let mut at = snapshot;
        while let Some((parent, id)) = all.get(&at) {
            if covered.len() > all.len() {
                return Err(Error::Corrupt("the versions form a cycle".into()));
            }
            covered.push((names::version_name(*parent, at), id.clone()));
            at = *parent;
        }
        covered.reverse();
        covered.truncate(self.repo.history_batch());
        let paths: Vec<String> = covered.iter().map(|(p, _)| p.clone()).collect();
        let changed = self.repo.last_changed(&paths).await?;
        let doomed: Vec<Change> = covered
            .into_iter()
            .zip(changed)
            // Oldest first: the first that is recent enough to keep ends the run, so what is left is one unbroken chain.
            .take_while(|(_, at)| at.is_some_and(|t| t <= now - MAX_VERSION_AGE_SECS))
            .map(|((path, was), _)| Change::Delete { path, was })
            .collect();
        if doomed.is_empty() {
            return Ok(0);
        }
        match self
            .repo
            .commit(Some(head), "cleanup: remove version files covered by snapshot", &doomed)
            .await?
        {
            Committed::Done(_) => Ok(doomed.len()),
            Committed::Moved => Ok(0),
        }
    }
}

/// The child of `parent` on the chain to `latest`, with its blob id. A branch written only by commits has one child
/// per parent; should there be more, the one that is `latest` or has children of its own is the real one.
fn next_on_chain(
    all: &BTreeMap<VersionId, (VersionId, String)>,
    parent: VersionId,
    latest: VersionId,
) -> Option<(VersionId, String)> {
    let children: Vec<(VersionId, &String)> = all
        .iter()
        .filter(|(_, (p, _))| *p == parent)
        .map(|(c, (_, id))| (*c, id))
        .collect();
    let pick = match children.as_slice() {
        [] => return None,
        [only] => *only,
        many => *many
            .iter()
            .find(|(c, _)| *c == latest || all.values().any(|(p, _)| p == c))
            .unwrap_or(&many[0]),
    };
    Some((pick.0, pick.1.clone()))
}

#[async_trait(?Send)]
impl<R: GitRepo> Server for GitServer<R> {
    async fn add_version(
        &mut self,
        parent_version_id: VersionId,
        history_segment: HistorySegment,
    ) -> TcResult<(AddVersionResult, SnapshotUrgency)> {
        let res = self.add_version_inner(parent_version_id, &history_segment).await?;
        let urgency = match res {
            // Counting needs a listing; if it fails the version is still added, and only the snapshot waits.
            AddVersionResult::Ok(v) => self.urgency(v).await.unwrap_or(SnapshotUrgency::None),
            AddVersionResult::ExpectedParentVersion(_) => SnapshotUrgency::None,
        };
        Ok((res, urgency))
    }

    async fn get_child_version(&mut self, parent_version_id: VersionId) -> TcResult<GetVersionResult> {
        Ok(self.get_child_inner(parent_version_id).await?)
    }

    async fn add_snapshot(&mut self, version_id: VersionId, snapshot: Snapshot) -> TcResult<()> {
        // A snapshot only makes the next cold start shorter, and the version it describes is already on the branch.
        // Failing the sync here would leave the replica believing that version was never sent, so problems are not
        // reported (as for a bucket, `crate::cloud`).
        self.add_snapshot_inner(version_id, &snapshot).await.ok();
        self.seen = Seen::default();
        Ok(())
    }

    async fn get_snapshot(&mut self) -> TcResult<Option<(VersionId, Snapshot)>> {
        let (_, file) = self.repo.read(SNAPSHOT).await?;
        let Some(file) = file else {
            return Ok(None);
        };
        let (version, sealed) = parse_snapshot(&file.bytes)?;
        SNAPSHOT_OF.with(|s| *s.borrow_mut() = Some((file.id, version)));
        Ok(Some((version, self.cryptor.unseal(version, &sealed)?)))
    }
}

/// The settings files the web app keeps (`web/…`) as an [`ObjectStore`] on the same branch: a name is a path from the
/// root, and each write is a commit of its own.
pub struct GitObjects<R>(pub R);

impl<R: GitRepo> GitObjects<R> {
    /// Commit one change worked out from the file as it is now; `None` from `change` means there is nothing to do.
    /// Returns whether a commit was made (or nothing needed doing).
    async fn write(&self, name: &str, message: &str, change: impl Fn(Option<&Blob>) -> Option<Change>) -> Result<bool> {
        for _ in 0..RETRIES {
            let (head, current) = self.0.read(name).await?;
            let Some(change) = change(current.as_ref()) else {
                return Ok(true);
            };
            if let Committed::Done(_) = self.0.commit(head.as_deref(), message, &[change]).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Replace `name` only if it still is `expected` (`None`: does not exist), as the bucket's conditional put.
    async fn swap(&self, name: &str, expected: impl Fn(Option<&Blob>) -> bool, new: &[u8]) -> Result<bool> {
        for _ in 0..RETRIES {
            let (head, current) = self.0.read(name).await?;
            if !expected(current.as_ref()) {
                return Ok(false);
            }
            let change = match current {
                Some(c) => Change::Update {
                    path: name.into(),
                    bytes: new.to_vec(),
                    was: c.id,
                },
                None => Change::Create {
                    path: name.into(),
                    bytes: new.to_vec(),
                },
            };
            if let Committed::Done(_) = self
                .0
                .commit(head.as_deref(), &format!("save {name}"), &[change])
                .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl<R: GitRepo> ObjectStore for GitObjects<R> {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.0.read(name).await?.1.map(|b| b.bytes))
    }

    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        let saved = self
            .write(name, &format!("save {name}"), |current| match current {
                Some(c) if c.bytes == value => None,
                Some(c) => Some(Change::Update {
                    path: name.into(),
                    bytes: value.to_vec(),
                    was: c.id.clone(),
                }),
                None => Some(Change::Create {
                    path: name.into(),
                    bytes: value.to_vec(),
                }),
            })
            .await?;
        if saved {
            Ok(())
        } else {
            Err(Error::TooMuchContention)
        }
    }

    /// One commit for them all: each file read for what it is now (and the head with it), then committed together; a
    /// head that moved meanwhile starts it again.
    async fn write_many(&self, writes: &[(String, Option<Vec<u8>>)]) -> Result<()> {
        for _ in 0..RETRIES {
            let mut head: Option<Option<String>> = None;
            let mut moved = false;
            let mut changes = Vec::new();
            for (name, value) in writes {
                let (h, current) = self.0.read(name).await?;
                // Each read gives the head it was made at; they must agree, or the files are from two commits.
                match &head {
                    Some(seen) if *seen != h => moved = true,
                    _ => head = Some(h),
                }
                // A file written twice in one go: the last write is the one kept.
                changes.retain(|c: &Change| c.path() != name);
                let change = match (current, value) {
                    (Some(c), Some(v)) if c.bytes == *v => None,
                    (Some(c), Some(v)) => Some(Change::Update {
                        path: name.clone(),
                        bytes: v.clone(),
                        was: c.id,
                    }),
                    (None, Some(v)) => Some(Change::Create {
                        path: name.clone(),
                        bytes: v.clone(),
                    }),
                    (Some(c), None) => Some(Change::Delete {
                        path: name.clone(),
                        was: c.id,
                    }),
                    (None, None) => None,
                };
                changes.extend(change);
            }
            if moved {
                continue;
            }
            if changes.is_empty() {
                return Ok(());
            }
            let message = match changes.as_slice() {
                [one] => format!("save {}", one.path()),
                _ => format!("save {} files", changes.len()),
            };
            let head = head.flatten();
            if let Committed::Done(_) = self.0.commit(head.as_deref(), &message, &changes).await? {
                return Ok(());
            }
        }
        Err(Error::TooMuchContention)
    }

    async fn del(&self, name: &str) -> Result<()> {
        let done = self
            .write(name, &format!("remove {name}"), |current| {
                current.map(|c| Change::Delete {
                    path: name.into(),
                    was: c.id.clone(),
                })
            })
            .await?;
        if done {
            Ok(())
        } else {
            Err(Error::TooMuchContention)
        }
    }

    /// The folder listed once, and each file read by its blob id, which a host may keep (the page does): a file that
    /// did not change is not fetched again.
    async fn get_all(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let dir = prefix.rsplit_once('/').map_or("", |(d, _)| d);
        if !dir.is_empty() {
            if let Some(files) = self.0.read_dir(dir).await? {
                return Ok(files
                    .into_iter()
                    .filter(|(p, _)| p.starts_with(prefix))
                    .map(|(p, b)| (p, b.bytes))
                    .collect());
            }
        }
        let entries = if dir.is_empty() {
            self.0.root().await?.1
        } else {
            self.0.tree(dir).await?
        };
        let mut out = Vec::new();
        for e in entries.into_iter().filter(|e| e.path.starts_with(prefix)) {
            let bytes = self.0.blob(&e.id).await?;
            out.push((e.path, bytes));
        }
        Ok(out)
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        // The folder the prefix is in, listed once.
        let dir = prefix.rsplit_once('/').map_or("", |(d, _)| d);
        let entries = if dir.is_empty() {
            self.0.root().await?.1
        } else {
            self.0.tree(dir).await?
        };
        Ok(entries
            .into_iter()
            .map(|e| e.path)
            .filter(|p| p.starts_with(prefix))
            .collect())
    }

    /// Each file with the time of the last commit that changed it, which is what a settings file shared with a phone is
    /// judged newer by when both sides changed it. It costs history queries, so [`list`](ObjectStore::list) is the
    /// listing for everything else.
    async fn list_dated(&self, prefix: &str) -> Result<Vec<(String, i64)>> {
        let names = self.list(prefix).await?;
        let mut out = Vec::with_capacity(names.len());
        for chunk in names.chunks(self.0.history_batch().max(1)) {
            let dates = self.0.last_changed(chunk).await?;
            out.extend(chunk.iter().cloned().zip(dates.into_iter().map(|d| d.unwrap_or(0))));
        }
        Ok(out)
    }

    async fn compare_and_swap(&self, name: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        self.swap(name, |c| c.map(|c| c.bytes.as_slice()) == expected, new)
            .await
    }

    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        Ok(self.0.read(name).await?.1.map(|b| (b.bytes, b.id)))
    }

    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        self.swap(name, |c| c.map(|c| c.id.as_str()) == expected, new).await
    }
}

/// An in-memory branch for tests: every commit is kept, with the time it was made, so the history can be asked about.
/// Clones share the branch, as two replicas share a remote.
#[derive(Clone, Default)]
pub struct MemRepo(Rc<RefCell<MemInner>>);

#[derive(Default)]
struct MemInner {
    /// The files at the head: path -> (blob id, bytes).
    files: BTreeMap<String, (String, Vec<u8>)>,
    /// Commits made, the newest last: (message, the time, the paths changed).
    log: Vec<(String, i64, Vec<String>)>,
    /// Blob ids are handed out from this counter.
    next_id: u64,
    now: i64,
    /// Whether the branch exists (it does after the first commit).
    exists: bool,
    /// Check every file a change names (Gitea, Forgejo), not only the head (GitHub).
    check_files: bool,
    /// The next commit is refused as `Moved` once, as if somebody committed first.
    lose_next: usize,
}

impl MemRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// A branch that checks files, as Gitea and Forgejo do, instead of the head.
    pub fn checking_files() -> Self {
        let repo = Self::default();
        repo.0.borrow_mut().check_files = true;
        repo
    }

    /// From now on, commits are made at `secs` (seconds since the epoch).
    pub fn set_now(&self, secs: i64) {
        self.0.borrow_mut().now = secs;
    }

    /// The next `n` commits are refused as `Moved`, as when other writers keep getting there first.
    pub fn lose_next(&self, n: usize) {
        self.0.borrow_mut().lose_next = n;
    }

    /// Write a file as another replica would (a commit of its own, with no checks): to set up a test.
    pub fn insert(&self, path: &str, bytes: &[u8]) {
        let mut s = self.0.borrow_mut();
        s.next_id += 1;
        let id = format!("b{}", s.next_id);
        s.files.insert(path.to_owned(), (id, bytes.to_vec()));
        let now = s.now;
        s.log.push((format!("write {path}"), now, vec![path.to_owned()]));
        s.exists = true;
    }

    pub fn paths(&self) -> Vec<String> {
        self.0.borrow().files.keys().cloned().collect()
    }

    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        self.0.borrow().files.get(path).map(|(_, b)| b.clone())
    }

    /// The commit messages, oldest first.
    pub fn messages(&self) -> Vec<String> {
        self.0.borrow().log.iter().map(|(m, _, _)| m.clone()).collect()
    }

    fn head_of(inner: &MemInner) -> Option<String> {
        inner.exists.then(|| format!("c{}", inner.log.len()))
    }
}

impl GitRepo for MemRepo {
    async fn read(&self, path: &str) -> Result<(Option<String>, Option<Blob>)> {
        let s = self.0.borrow();
        let blob = s.files.get(path).map(|(id, bytes)| Blob {
            id: id.clone(),
            bytes: bytes.clone(),
        });
        Ok((Self::head_of(&s), blob))
    }

    async fn root(&self) -> Result<(Option<String>, Vec<Entry>)> {
        let s = self.0.borrow();
        let entries = s
            .files
            .iter()
            .filter(|(p, _)| !p.contains('/'))
            .map(|(p, (id, _))| Entry {
                path: p.clone(),
                id: id.clone(),
            })
            .collect();
        Ok((Self::head_of(&s), entries))
    }

    async fn tree(&self, dir: &str) -> Result<Vec<Entry>> {
        let prefix = format!("{dir}/");
        Ok(self
            .0
            .borrow()
            .files
            .iter()
            .filter(|(p, _)| p.starts_with(&prefix))
            .map(|(p, (id, _))| Entry {
                path: p.clone(),
                id: id.clone(),
            })
            .collect())
    }

    async fn blob(&self, id: &str) -> Result<Vec<u8>> {
        self.0
            .borrow()
            .files
            .values()
            .find(|(i, _)| i == id)
            .map(|(_, b)| b.clone())
            .ok_or_else(|| Error::Store(format!("no blob {id}")))
    }

    async fn commit(&self, head: Option<&str>, message: &str, changes: &[Change]) -> Result<Committed> {
        let mut s = self.0.borrow_mut();
        if s.lose_next > 0 {
            s.lose_next -= 1;
            return Ok(Committed::Moved);
        }
        if s.check_files {
            for change in changes {
                let ok = match change {
                    Change::Create { path, .. } => !s.files.contains_key(path),
                    Change::Update { path, was, .. } | Change::Delete { path, was } => {
                        s.files.get(path).is_some_and(|(id, _)| id == was)
                    }
                };
                if !ok {
                    return Ok(Committed::Moved);
                }
            }
        } else if head.map(str::to_owned) != Self::head_of(&s) {
            return Ok(Committed::Moved);
        }
        let mut paths = Vec::new();
        for change in changes {
            match change {
                Change::Create { path, bytes } | Change::Update { path, bytes, .. } => {
                    s.next_id += 1;
                    let id = format!("b{}", s.next_id);
                    s.files.insert(path.clone(), (id, bytes.clone()));
                    paths.push(path.clone());
                }
                Change::Delete { path, .. } => {
                    s.files.remove(path);
                    paths.push(path.clone());
                }
            }
        }
        let now = s.now;
        s.log.push((message.to_owned(), now, paths));
        s.exists = true;
        Ok(Committed::Done(format!("c{}", s.log.len())))
    }

    async fn last_changed(&self, paths: &[String]) -> Result<Vec<Option<i64>>> {
        let s = self.0.borrow();
        Ok(paths
            .iter()
            .map(|p| s.log.iter().rev().find(|(_, _, ps)| ps.contains(p)).map(|(_, t, _)| *t))
            .collect())
    }
}

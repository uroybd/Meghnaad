//! What the browser's replica and the Worker say to each other.
//!
//! The browser runs the engine and holds the tasks; the Worker holds the secret and the bucket. Versions travel
//! between them in plain form over HTTPS (behind Cloudflare Access) and the Worker seals and unseals them on the
//! way to and from R2, so the secret never leaves it.
//!
//! A pull answers with a run of versions in one body, so a replica that is far behind needs a few requests, not one
//! for every version (TaskChampion's own protocol asks for them one at a time).

use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use taskchampion::server::{AddVersionResult, GetVersionResult, Server, SnapshotUrgency};
use uuid::Uuid;

/// TaskChampion's result type (its own module is private).
type TcResult<T> = std::result::Result<T, taskchampion::Error>;

/// The most versions one pull carries: each is a read from the bucket, and a Worker request on the free plan may
/// make 50 of them.
pub const PULL_MAX: usize = 24;

/// One version of the history, as the replica asks for it and the Worker hands it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub id: Uuid,
    pub parent: Uuid,
    pub segment: Vec<u8>,
}

/// `id`, `parent` (16 bytes each), the segment's length (4 bytes, big endian) and the segment, for each version.
pub fn encode_versions(versions: &[Version]) -> Vec<u8> {
    let mut out = Vec::with_capacity(versions.iter().map(|v| 36 + v.segment.len()).sum());
    for v in versions {
        out.extend_from_slice(v.id.as_bytes());
        out.extend_from_slice(v.parent.as_bytes());
        out.extend_from_slice(&(v.segment.len() as u32).to_be_bytes());
        out.extend_from_slice(&v.segment);
    }
    out
}

pub fn decode_versions(mut bytes: &[u8]) -> Result<Vec<Version>> {
    let bad = || Error::Corrupt("a pull answer ends in the middle of a version".into());
    let mut out = Vec::new();
    while !bytes.is_empty() {
        if bytes.len() < 36 {
            return Err(bad());
        }
        let id = Uuid::from_slice(&bytes[..16]).map_err(|_| bad())?;
        let parent = Uuid::from_slice(&bytes[16..32]).map_err(|_| bad())?;
        let len = u32::from_be_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]) as usize;
        let rest = &bytes[36..];
        if rest.len() < len {
            return Err(bad());
        }
        out.push(Version {
            id,
            parent,
            segment: rest[..len].to_vec(),
        });
        bytes = &rest[len..];
    }
    Ok(out)
}

/// The Worker's answer to a push.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Pushed {
    /// The version was added, and how much the server would like a snapshot now.
    Ok { version: Uuid, urgency: Urgency },
    /// Somebody else got there first: the server's newest version is this one.
    Behind { head: Uuid },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    None,
    Low,
    High,
}

impl From<SnapshotUrgency> for Urgency {
    fn from(u: SnapshotUrgency) -> Self {
        match u {
            SnapshotUrgency::None => Urgency::None,
            SnapshotUrgency::Low => Urgency::Low,
            SnapshotUrgency::High => Urgency::High,
        }
    }
}

impl From<Urgency> for SnapshotUrgency {
    fn from(u: Urgency) -> Self {
        match u {
            Urgency::None => SnapshotUrgency::None,
            Urgency::Low => SnapshotUrgency::Low,
            Urgency::High => SnapshotUrgency::High,
        }
    }
}

// The Worker's half: answers the replica's requests from the bucket (a TaskChampion server, which unseals and seals).

/// The versions after `since` (the nil id for "from the start"), up to [`PULL_MAX`], one after the other, and whether
/// there may be more after the last.
pub async fn pull(server: &mut dyn Server, since: Uuid) -> TcResult<(Vec<Version>, bool)> {
    let mut versions = Vec::new();
    let mut parent = since;
    while versions.len() < PULL_MAX {
        match server.get_child_version(parent).await? {
            GetVersionResult::NoSuchVersion => break,
            GetVersionResult::Version {
                version_id,
                parent_version_id,
                history_segment,
            } => {
                versions.push(Version {
                    id: version_id,
                    parent: parent_version_id,
                    segment: history_segment,
                });
                parent = version_id;
            }
        }
    }
    let more = versions.len() == PULL_MAX;
    Ok((versions, more))
}

/// Add a version after `parent`.
pub async fn push(server: &mut dyn Server, parent: Uuid, segment: Vec<u8>) -> TcResult<Pushed> {
    let (added, urgency) = server.add_version(parent, segment).await?;
    Ok(match added {
        AddVersionResult::Ok(version) => Pushed::Ok {
            version,
            urgency: urgency.into(),
        },
        AddVersionResult::ExpectedParentVersion(head) => Pushed::Behind { head },
    })
}

// The replica's half: a TaskChampion server that is really a conversation with the Worker.

/// How the replica reaches the Worker (HTTP in the browser; a direct call in tests). Four requests, as `/api/tc/*`.
#[async_trait(?Send)]
pub trait Transport {
    async fn pull(&mut self, since: Uuid) -> TcResult<(Vec<Version>, bool)>;
    async fn push(&mut self, parent: Uuid, segment: Vec<u8>) -> TcResult<Pushed>;
    async fn get_snapshot(&mut self) -> TcResult<Option<(Uuid, Vec<u8>)>>;
    async fn put_snapshot(&mut self, version: Uuid, snapshot: Vec<u8>) -> TcResult<()>;
}

/// A TaskChampion [`Server`] over a [`Transport`].
///
/// TaskChampion asks for the versions after the one it has one at a time. A pull answers with a run of them, and this
/// keeps what it has not been asked for yet, so a replica that is far behind costs a few requests, not one for each.
pub struct RemoteServer<T> {
    transport: T,
    /// Versions fetched and not asked for yet, by the version they follow.
    ahead: HashMap<Uuid, Version>,
    /// The Worker said there is nothing after this version.
    end: Option<Uuid>,
}

impl<T: Transport> RemoteServer<T> {
    pub fn new(transport: T) -> Self {
        RemoteServer {
            transport,
            ahead: HashMap::new(),
            end: None,
        }
    }
}

#[async_trait(?Send)]
impl<T: Transport> Server for RemoteServer<T> {
    async fn add_version(&mut self, parent: Uuid, segment: Vec<u8>) -> TcResult<(AddVersionResult, SnapshotUrgency)> {
        // The head moves, so what was fetched ahead is no longer the whole story.
        self.ahead.clear();
        self.end = None;
        Ok(match self.transport.push(parent, segment).await? {
            Pushed::Ok { version, urgency } => (AddVersionResult::Ok(version), urgency.into()),
            Pushed::Behind { head } => (AddVersionResult::ExpectedParentVersion(head), SnapshotUrgency::None),
        })
    }

    async fn get_child_version(&mut self, parent: Uuid) -> TcResult<GetVersionResult> {
        let found = |v: Version| GetVersionResult::Version {
            version_id: v.id,
            parent_version_id: v.parent,
            history_segment: v.segment,
        };
        if let Some(v) = self.ahead.remove(&parent) {
            return Ok(found(v));
        }
        if self.end == Some(parent) {
            return Ok(GetVersionResult::NoSuchVersion);
        }
        let (versions, more) = self.transport.pull(parent).await?;
        if !more {
            self.end = Some(versions.last().map_or(parent, |v| v.id));
        }
        for v in versions {
            self.ahead.insert(v.parent, v);
        }
        Ok(match self.ahead.remove(&parent) {
            Some(v) => found(v),
            None => GetVersionResult::NoSuchVersion,
        })
    }

    async fn add_snapshot(&mut self, version: Uuid, snapshot: Vec<u8>) -> TcResult<()> {
        // A snapshot only makes the next cold start shorter; not having stored one is not worth failing a sync for.
        self.transport.put_snapshot(version, snapshot).await.ok();
        Ok(())
    }

    async fn get_snapshot(&mut self) -> TcResult<Option<(Uuid, Vec<u8>)>> {
        self.transport.get_snapshot().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(n: u128, len: usize) -> Version {
        Version {
            id: Uuid::from_u128(n),
            parent: Uuid::from_u128(n - 1),
            segment: (0..len).map(|i| (i % 251) as u8).collect(),
        }
    }

    #[test]
    fn versions_survive_the_trip_in_order_including_empty_ones() {
        let all = vec![v(2, 0), v(3, 1), v(4, 70_000), v(5, 12)];
        assert_eq!(decode_versions(&encode_versions(&all)).unwrap(), all);
        assert_eq!(decode_versions(&[]).unwrap(), vec![]);
    }

    #[test]
    fn a_cut_off_answer_is_an_error_not_a_shorter_list() {
        let bytes = encode_versions(&[v(2, 10), v(3, 10)]);
        for cut in [1, 20, 35, 36, 40, bytes.len() - 1] {
            assert!(decode_versions(&bytes[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn replies_are_what_the_browser_and_the_worker_agree_on() {
        let ok = Pushed::Ok {
            version: Uuid::from_u128(7),
            urgency: Urgency::High,
        };
        let text = serde_json::to_string(&ok).unwrap();
        assert_eq!(
            text,
            r#"{"result":"ok","version":"00000000-0000-0000-0000-000000000007","urgency":"high"}"#
        );
        assert_eq!(serde_json::from_str::<Pushed>(&text).unwrap(), ok);
        let behind = Pushed::Behind {
            head: Uuid::from_u128(9),
        };
        assert_eq!(
            serde_json::from_str::<Pushed>(&serde_json::to_string(&behind).unwrap()).unwrap(),
            behind
        );
    }
}

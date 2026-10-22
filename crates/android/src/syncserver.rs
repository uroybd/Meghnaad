//! A TaskChampion sync server (`taskchampion-sync-server` and anything that speaks its protocol) as a
//! [`taskchampion::Server`]: the phone's other way to sync its tasks besides a bucket.
//!
//! The protocol is plain HTTP under `<url>/v1/client/…`, every request carrying the client's id in `X-Client-Id`. What is
//! sent is sealed exactly as for a bucket (`tc_core::crypto::Cryptor`); the one difference is the key, which is derived
//! with the client id as the salt, since there is no `salt` object to read.
//!
//! The requests are blocking (`ureq`), like the bucket's: the engine runs them on a thread of its own, one sync at a time.

use async_trait::async_trait;
use std::time::Duration;
use taskchampion::server::{
    AddVersionResult, GetVersionResult, HistorySegment, Server, Snapshot, SnapshotUrgency, VersionId,
};
use tc_core::crypto::Cryptor;
use tc_core::Error;
use uuid::Uuid;

type TcResult<T> = std::result::Result<T, taskchampion::Error>;

const HISTORY_SEGMENT: &str = "application/vnd.taskchampion.history-segment";
const SNAPSHOT: &str = "application/vnd.taskchampion.snapshot";

/// The key for a client id and encryption secret (600,000 rounds of PBKDF2: slow on a phone, so the engine keeps it).
pub fn cryptor(client_id: Uuid, secret: &str) -> Cryptor {
    Cryptor::new(client_id.as_bytes(), secret.as_bytes())
}

pub struct SyncServer {
    /// The server's address with a slash at the end, so that a path joins onto it.
    base: String,
    client_id: Uuid,
    cryptor: Cryptor,
    agent: ureq::Agent,
}

/// What came back from one request.
struct Reply {
    status: u16,
    content_type: Option<String>,
    version: Option<String>,
    parent: Option<String>,
    snapshot_request: Option<String>,
    body: Vec<u8>,
}

fn err(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Store(format!("{what}: {e}"))
}

/// The server's address as it should be used: `https://` (or `http://` to this machine), a host, an optional path.
pub fn base_url(url: &str) -> Result<String, Error> {
    let bad = || Error::Store("the server's address should look like https://tasks.example.com".into());
    let url = url.trim();
    let rest = match url.strip_prefix("https://") {
        Some(r) => r,
        // Only to this machine: a server anywhere else is reached over TLS.
        None => match url.strip_prefix("http://") {
            Some(r) if matches!(r.split(['/', ':']).next(), Some("localhost" | "127.0.0.1")) => r,
            _ => return Err(bad()),
        },
    };
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() || rest.contains(['?', '#']) || host.contains('@') {
        return Err(bad());
    }
    let scheme = &url[..url.len() - rest.len()];
    Ok(format!("{scheme}{}/", rest.trim_end_matches('/')))
}

impl SyncServer {
    pub fn new(url: &str, client_id: Uuid, cryptor: Cryptor) -> Result<Self, Error> {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .into();
        Ok(SyncServer {
            base: base_url(url)?,
            client_id,
            cryptor,
            agent,
        })
    }

    fn send(&self, method: &str, path: &str, content_type: Option<&str>, body: Vec<u8>) -> Result<Reply, Error> {
        let mut request = ureq::http::Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.base))
            .header("x-client-id", self.client_id.to_string());
        if let Some(t) = content_type {
            request = request.header("content-type", t);
        }
        let request = request.body(body).map_err(|e| err("building the request", e))?;
        let mut response = self
            .agent
            .run(request)
            .map_err(|e| err("could not reach the sync server", e))?;
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let reply = Reply {
            status: response.status().as_u16(),
            content_type: header("content-type"),
            version: header("x-version-id"),
            parent: header("x-parent-version-id"),
            snapshot_request: header("x-snapshot-request"),
            body: Vec::new(),
        };
        let body = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| err("reading the sync server's answer", e))?;
        Ok(Reply { body, ..reply })
    }

    /// The answer if it is a success; otherwise why not, in words that say what to change.
    fn ok(what: &str, r: Reply) -> Result<Reply, Error> {
        if (200..300).contains(&r.status) {
            return Ok(r);
        }
        Err(Error::Store(refusal(what, r.status, &String::from_utf8_lossy(&r.body))))
    }
}

/// What a refused request means for whoever typed the connection in.
fn refusal(what: &str, status: u16, body: &str) -> String {
    let hint = match status {
        401 | 403 => {
            " The server did not accept this client id. If the address is behind a sign-in (such as Cloudflare Access), the phone cannot get through it."
        }
        404 => " Nothing is answering there as a sync server: check the address (it is the server's own, without /v1/client…).",
        413 => " The server refuses a request this large: raise its size limit.",
        500..=599 => " The server failed; try again, and look at its log if it keeps happening.",
        _ => "",
    };
    let detail: String = body.trim().chars().take(120).collect();
    let detail = if detail.contains('<') { String::new() } else { detail };
    format!("{what}: status {status} {detail}.{hint}")
        .replace("  ", " ")
        .replace(" .", ".")
}

fn uuid_header(what: &str, value: Option<&str>, name: &str) -> Result<Uuid, Error> {
    value.and_then(|v| Uuid::parse_str(v.trim()).ok()).ok_or_else(|| {
        Error::Store(format!(
            "{what}: the answer has no valid {name} header: this does not look like a TaskChampion sync server"
        ))
    })
}

/// Whether an answer is a web page, which is what a server that does not know the paths gives.
fn is_page(r: &Reply) -> bool {
    r.content_type.as_deref().is_some_and(|t| t.starts_with("text/html")) || r.body.trim_ascii_start().starts_with(b"<")
}

/// The body, when the answer is the kind of thing asked for. Anything else is not a sync server (usually a web page).
fn expect_type(what: &str, r: &Reply, wanted: &str) -> Result<(), Error> {
    if r.content_type.as_deref().is_some_and(|t| t.starts_with(wanted)) {
        Ok(())
    } else {
        Err(Error::Store(format!(
            "{what}: the answer is not what a TaskChampion sync server sends: check the address"
        )))
    }
}

#[async_trait(?Send)]
impl Server for SyncServer {
    async fn add_version(
        &mut self,
        parent_version_id: VersionId,
        history_segment: HistorySegment,
    ) -> TcResult<(AddVersionResult, SnapshotUrgency)> {
        let what = "sending changes to the sync server";
        let sealed = self.cryptor.seal(parent_version_id, &history_segment)?;
        let r = self.send(
            "POST",
            &format!("v1/client/add-version/{parent_version_id}"),
            Some(HISTORY_SEGMENT),
            sealed,
        )?;
        if r.status == 409 {
            let parent = uuid_header(what, r.parent.as_deref(), "X-Parent-Version-Id")?;
            return Ok((AddVersionResult::ExpectedParentVersion(parent), SnapshotUrgency::None));
        }
        let r = Self::ok(what, r)?;
        let version = uuid_header(what, r.version.as_deref(), "X-Version-Id")?;
        let urgency = match r.snapshot_request.as_deref() {
            Some("urgency=low") => SnapshotUrgency::Low,
            Some("urgency=high") => SnapshotUrgency::High,
            _ => SnapshotUrgency::None,
        };
        Ok((AddVersionResult::Ok(version), urgency))
    }

    async fn get_child_version(&mut self, parent_version_id: VersionId) -> TcResult<GetVersionResult> {
        let what = "getting changes from the sync server";
        let r = self.send(
            "GET",
            &format!("v1/client/get-child-version/{parent_version_id}"),
            None,
            Vec::new(),
        )?;
        if r.status == 404 && !is_page(&r) {
            // The server's own "there is nothing after that version" (or "no such client", for one it has not met yet); a
            // 404 page is another matter.
            return Ok(GetVersionResult::NoSuchVersion);
        }
        let r = Self::ok(what, r)?;
        expect_type(what, &r, HISTORY_SEGMENT)?;
        let version_id = uuid_header(what, r.version.as_deref(), "X-Version-Id")?;
        let parent = uuid_header(what, r.parent.as_deref(), "X-Parent-Version-Id")?;
        Ok(GetVersionResult::Version {
            version_id,
            parent_version_id: parent,
            history_segment: self.cryptor.unseal(parent, &r.body)?,
        })
    }

    async fn add_snapshot(&mut self, version_id: VersionId, snapshot: Snapshot) -> TcResult<()> {
        let sealed = self.cryptor.seal(version_id, &snapshot)?;
        let r = self.send(
            "POST",
            &format!("v1/client/add-snapshot/{version_id}"),
            Some(SNAPSHOT),
            sealed,
        )?;
        Self::ok("sending a snapshot to the sync server", r)?;
        Ok(())
    }

    async fn get_snapshot(&mut self) -> TcResult<Option<(VersionId, Snapshot)>> {
        let what = "getting a snapshot from the sync server";
        let r = self.send("GET", "v1/client/snapshot", None, Vec::new())?;
        if r.status == 404 && !is_page(&r) {
            return Ok(None);
        }
        let r = Self::ok(what, r)?;
        expect_type(what, &r, SNAPSHOT)?;
        let version = uuid_header(what, r.version.as_deref(), "X-Version-Id")?;
        Ok(Some((version, self.cryptor.unseal(version, &r.body)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_made_ready_to_join_paths_onto() {
        assert_eq!(
            base_url("https://tasks.example.com").unwrap(),
            "https://tasks.example.com/"
        );
        assert_eq!(base_url(" https://a.example/tc/ ").unwrap(), "https://a.example/tc/");
        assert_eq!(base_url("http://127.0.0.1:8080").unwrap(), "http://127.0.0.1:8080/");
        assert_eq!(base_url("http://localhost").unwrap(), "http://localhost/");
        for bad in [
            "",
            "tasks.example.com",
            "http://tasks.example.com",
            "ftp://x",
            "https://",
            "https://user@host/",
            "https://host/?q=1",
        ] {
            assert!(base_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_refusal_says_what_to_change() {
        assert!(refusal("x", 403, "").contains("did not accept this client id"));
        assert!(refusal("x", 404, "").contains("check the address"));
        assert!(refusal("x", 502, "<html>bad gateway</html>").contains("failed"));
        // A web page is not quoted.
        assert!(!refusal("x", 502, "<html>bad gateway</html>").contains("html"));
        assert_eq!(refusal("x", 400, "bad request"), "x: status 400 bad request.");
    }
}

//! The bucket over S3, signed with the access key, as an [`ObjectStore`] for the sync server. Nothing in it is
//! specific to R2: any S3-compatible store that takes path-style requests and conditional writes (`If-Match`,
//! `If-None-Match`) works, such as R2, AWS S3, MinIO or Backblaze B2.
//!
//! The requests are blocking (`ureq`); the engine runs them on a thread of its own, one sync at a time, so nothing
//! waits on them but the sync itself.

use crate::sigv4::{authorization, sha256_hex, Credentials, Signed};
use serde::Deserialize;
use std::time::Duration;
use taskchampion::chrono::{DateTime, Utc};
use tc_core::{Error, ObjectStore, Result};

/// Where the bucket is and how to sign in to it. The phone keeps these in the Keystore and hands them over per sync.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Bucket {
    /// The store's S3 address: `https://<account>.r2.cloudflarestorage.com` for R2, `https://s3.<region>.amazonaws.com`
    /// for AWS.
    pub endpoint: String,
    pub bucket: String,
    /// `auto` for R2; the bucket's region (`eu-west-1`) for AWS.
    #[serde(default = "auto")]
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

fn auto() -> String {
    "auto".into()
}

pub struct S3Store {
    bucket: Bucket,
    /// `https`, or `http` for a bucket on this machine (a test's, or MinIO).
    scheme: &'static str,
    host: String,
    agent: ureq::Agent,
}

fn err(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Store(format!("{what}: {e}"))
}

/// What came back from one request.
struct Reply {
    status: u16,
    etag: Option<String>,
    body: Vec<u8>,
}

impl S3Store {
    pub fn new(bucket: Bucket) -> Result<Self> {
        let endpoint = bucket.endpoint.trim_end_matches('/');
        let bad = || {
            Error::Store(
                "the endpoint should be an https:// address, such as https://<account>.r2.cloudflarestorage.com".into(),
            )
        };
        let (scheme, host) = match endpoint.strip_prefix("https://") {
            Some(h) => ("https", h),
            // Only to this machine: a bucket anywhere else is reached over TLS.
            None => match endpoint.strip_prefix("http://") {
                Some(h) if matches!(h.split(':').next(), Some("localhost" | "127.0.0.1")) => ("http", h),
                _ => return Err(bad()),
            },
        };
        if host.is_empty() || host.contains('/') {
            return Err(bad());
        }
        let host = host.to_owned();
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .into();
        Ok(S3Store {
            bucket,
            scheme,
            host,
            agent,
        })
    }

    fn path(&self, name: &str) -> String {
        if name.is_empty() {
            format!("/{}", self.bucket.bucket)
        } else {
            format!("/{}/{name}", self.bucket.bucket)
        }
    }

    fn send(
        &self,
        method: &str,
        name: &str,
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Reply> {
        let amz_date = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let payload = sha256_hex(body);
        let path = self.path(name);
        let credentials = Credentials {
            access_key: &self.bucket.access_key,
            secret_key: &self.bucket.secret_key,
            region: &self.bucket.region,
        };
        let signed = Signed {
            method,
            host: &self.host,
            path: &path,
            query,
            headers,
            payload_sha256: &payload,
            amz_date: &amz_date,
        };
        let auth = authorization(&credentials, &signed);
        let mut url = format!("{}://{}{}", self.scheme, self.host, crate::sigv4::encode(&path, true));
        for (i, (k, v)) in query.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(&format!(
                "{}={}",
                crate::sigv4::encode(k, false),
                crate::sigv4::encode(v, false)
            ));
        }
        let mut request = ureq::http::Request::builder()
            .method(method)
            .uri(&url)
            .header("x-amz-date", &amz_date)
            .header("x-amz-content-sha256", &payload)
            .header("authorization", auth);
        for (k, v) in headers {
            request = request.header(*k, *v);
        }
        let request = request
            .body(body.to_vec())
            .map_err(|e| err("building the request", e))?;
        let mut response = self
            .agent
            .run(request)
            .map_err(|e| err("could not reach the bucket", e))?;
        let status = response.status().as_u16();
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| err("reading the bucket's answer", e))?;
        Ok(Reply { status, etag, body })
    }

    /// The answer if it is a success; otherwise why not, in words that say what to change.
    fn ok(&self, what: &str, r: Reply) -> Result<Reply> {
        if (200..300).contains(&r.status) {
            return Ok(r);
        }
        Err(Error::Store(refusal(what, r.status, &String::from_utf8_lossy(&r.body))))
    }
}

/// What a refused request means for whoever typed the connection in. S3 stores answer with an XML error (`<Code>`), so a
/// 403 with a web page behind it is not an S3 store at all: most often the app's own address, behind a sign-in.
fn refusal(what: &str, status: u16, body: &str) -> String {
    let code = between(body, "<Code>", "</Code>").unwrap_or_default();
    let message = between(body, "<Message>", "</Message>").unwrap_or_default();
    let hint = match (status, code) {
        (_, "SignatureDoesNotMatch") => " The secret access key is wrong (or the region: R2 wants \"auto\").",
        (_, "InvalidAccessKeyId") => " This store does not know that access key ID: check it and the endpoint.",
        (_, "AccessDenied") => {
            " The key is not allowed to do that: it needs Object Read & Write on this bucket (an R2 token is scoped to buckets)."
        }
        (_, "NoSuchBucket") => " There is no bucket with that name at this endpoint.",
        (401 | 403, "") => {
            " That address is not answering as an S3 store. The endpoint is the store's own address, such as https://<account id>.r2.cloudflarestorage.com, not the address of your app (which is behind Cloudflare Access)."
        }
        (_, "RequestTimeTooSkewed") => " The phone's clock is wrong: set the date and time automatically.",
        _ => "",
    };
    format!("{what}: status {status} {code} {message}.{hint}")
        .replace("  ", " ")
        .replace(" .", ".")
}

/// The text between two markers, the first time they appear.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// One page of a bucket listing: each object's name and upload time, and the token for the next page if there is one.
fn parse_listing(xml: &str) -> (Vec<(String, i64)>, Option<String>) {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Contents>") {
        let Some(len) = rest[start..].find("</Contents>") else {
            break;
        };
        let item = &rest[start..start + len];
        rest = &rest[start + len..];
        let Some(key) = between(item, "<Key>", "</Key>") else {
            continue;
        };
        let at = between(item, "<LastModified>", "</LastModified>")
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map_or(0, |t| t.timestamp());
        out.push((unescape(key), at));
    }
    let next = (between(xml, "<IsTruncated>", "</IsTruncated>") == Some("true"))
        .then(|| between(xml, "<NextContinuationToken>", "</NextContinuationToken>").map(unescape))
        .flatten();
    (out, next)
}

impl ObjectStore for S3Store {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let r = self.send("GET", name, &[], &[], &[])?;
        if r.status == 404 {
            return Ok(None);
        }
        Ok(Some(self.ok("reading an object", r)?.body))
    }

    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        let r = self.send("PUT", name, &[], &[], value)?;
        self.ok("writing an object", r).map(|_| ())
    }

    async fn del(&self, name: &str) -> Result<()> {
        let r = self.send("DELETE", name, &[], &[], &[])?;
        if r.status == 404 {
            return Ok(());
        }
        self.ok("deleting an object", r).map(|_| ())
    }

    async fn list_dated(&self, prefix: &str) -> Result<Vec<(String, i64)>> {
        let mut names = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut query = vec![("list-type", "2"), ("prefix", prefix)];
            if let Some(t) = &token {
                query.push(("continuation-token", t));
            }
            let r = self.send("GET", "", &query, &[], &[])?;
            let r = self.ok("listing the bucket", r)?;
            let (page, next) = parse_listing(&String::from_utf8_lossy(&r.body));
            names.extend(page);
            match next {
                Some(t) => token = Some(t),
                None => return Ok(names),
            }
        }
    }

    async fn compare_and_swap(&self, name: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        match (self.get_tagged(name).await?, expected) {
            (None, None) => self.swap_tagged(name, None, new).await,
            (Some((value, tag)), Some(expected)) if value == expected => self.swap_tagged(name, Some(&tag), new).await,
            _ => Ok(false),
        }
    }

    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        let r = self.send("GET", name, &[], &[], &[])?;
        if r.status == 404 {
            return Ok(None);
        }
        let r = self.ok("reading an object", r)?;
        let tag = r
            .etag
            .clone()
            .ok_or_else(|| Error::Store("the bucket did not give an ETag".into()))?;
        Ok(Some((r.body, tag)))
    }

    /// One conditional put: `If-Match` to replace the version we read, `If-None-Match: *` to create. A refused
    /// condition (412, or 409 when someone is writing at the same moment) means somebody else got there first.
    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        let condition = match expected {
            None => ("if-none-match", "*"),
            Some(tag) => ("if-match", tag),
        };
        let r = self.send("PUT", name, &[], &[condition], new)?;
        if r.status == 412 || r.status == 409 {
            return Ok(false);
        }
        self.ok("writing an object", r).map(|_| true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_gives_names_times_and_the_next_page() {
        let xml = "<?xml version=\"1.0\"?><ListBucketResult><IsTruncated>true</IsTruncated>\
            <Contents><Key>v-a</Key><LastModified>2026-10-11T20:33:15.000Z</LastModified><Size>3</Size></Contents>\
            <Contents><Key>v-b&amp;c</Key><LastModified>2026-10-11T20:33:16.000Z</LastModified></Contents>\
            <NextContinuationToken>1ueGcxLPRx1Tr/XYExHnhbYLgveDs2J/wm36Hy4vbOwM=</NextContinuationToken></ListBucketResult>";
        let (names, next) = parse_listing(xml);
        assert_eq!(
            names,
            vec![("v-a".to_owned(), 1_791_750_795), ("v-b&c".to_owned(), 1_791_750_796)]
        );
        assert_eq!(next.as_deref(), Some("1ueGcxLPRx1Tr/XYExHnhbYLgveDs2J/wm36Hy4vbOwM="));
        let (none, last) = parse_listing("<ListBucketResult><IsTruncated>false</IsTruncated></ListBucketResult>");
        assert!(none.is_empty() && last.is_none());
    }

    #[test]
    fn a_refusal_says_what_to_change() {
        let xml = |code: &str, message: &str| format!("<Error><Code>{code}</Code><Message>{message}</Message></Error>");
        let bad_secret = refusal(
            "listing the bucket",
            403,
            &xml(
                "SignatureDoesNotMatch",
                "The request signature we calculated does not match",
            ),
        );
        assert!(bad_secret.contains("status 403 SignatureDoesNotMatch"), "{bad_secret}");
        assert!(bad_secret.contains("secret access key is wrong"), "{bad_secret}");
        assert!(refusal("x", 403, &xml("InvalidAccessKeyId", "no")).contains("does not know that access key ID"));
        assert!(refusal("x", 403, &xml("AccessDenied", "Access Denied")).contains("Object Read & Write"));
        assert!(refusal("x", 404, &xml("NoSuchBucket", "gone")).contains("no bucket with that name"));
        // A 403 that is a sign-in page, not an S3 error: the app's own address.
        let page = refusal(
            "reading an object",
            403,
            "<html><body>Sign in with Cloudflare Access</body></html>",
        );
        assert!(page.starts_with("reading an object: status 403."), "{page}");
        assert!(page.contains("not answering as an S3 store"), "{page}");
        // An answer it has no advice for is still reported plainly.
        assert_eq!(
            refusal("x", 500, &xml("InternalError", "oops")),
            "x: status 500 InternalError oops."
        );
    }

    #[test]
    fn the_endpoint_has_to_be_an_https_host() {
        let bucket = |endpoint: &str| Bucket {
            endpoint: endpoint.into(),
            bucket: "b".into(),
            region: "auto".into(),
            access_key: "a".into(),
            secret_key: "s".into(),
        };
        assert!(S3Store::new(bucket("https://acct.r2.cloudflarestorage.com/")).is_ok());
        assert!(S3Store::new(bucket("http://acct.r2.cloudflarestorage.com")).is_err());
        assert!(S3Store::new(bucket("http://127.0.0.1:9000")).is_ok());
        assert!(S3Store::new(bucket("http://localhost.evil.example")).is_err());
        assert!(S3Store::new(bucket("http://127.0.0.1.evil.example:9000")).is_err());
        assert!(S3Store::new(bucket("https://acct.r2.cloudflarestorage.com/bucket")).is_err());
    }
}

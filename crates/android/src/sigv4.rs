//! AWS Signature Version 4 for S3, which is what R2's S3 endpoint expects: the signature that proves a request came from
//! the holder of the access key, without sending the secret key.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    #[allow(
        clippy::expect_used,
        reason = "HMAC takes a key of any length: `new_from_slice` cannot fail for it"
    )]
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Percent-encoding as S3 wants it: everything but unreserved characters, and `/` only where `slash` says it stays.
pub fn encode(text: &str, slash: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            b'/' if slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Who is asking: the bucket's access key and its secret.
pub struct Credentials<'a> {
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub region: &'a str,
}

/// What is signed of one request. `path` is the unencoded path (`/bucket/key`), `query` the unencoded pairs, and
/// `headers` the ones to sign besides `host`, `x-amz-content-sha256` and `x-amz-date`, which are always signed.
/// `amz_date` is `YYYYMMDDTHHMMSSZ`.
pub struct Signed<'a> {
    pub method: &'a str,
    pub host: &'a str,
    pub path: &'a str,
    pub query: &'a [(&'a str, &'a str)],
    pub headers: &'a [(&'a str, &'a str)],
    pub payload_sha256: &'a str,
    pub amz_date: &'a str,
}

/// The `Authorization` header for one request.
pub fn authorization(c: &Credentials, r: &Signed) -> String {
    let Signed {
        method,
        host,
        path,
        query,
        headers,
        payload_sha256,
        amz_date,
    } = *r;
    let mut query: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| (encode(k, false), encode(v, false)))
        .collect();
    query.sort();
    let query = query
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    let mut signed: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    signed.push(("host".into(), host.into()));
    signed.push(("x-amz-content-sha256".into(), payload_sha256.into()));
    signed.push(("x-amz-date".into(), amz_date.into()));
    signed.sort();
    let canonical_headers: String = signed.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed_names = signed.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(";");

    let canonical = format!(
        "{method}\n{}\n{query}\n{canonical_headers}\n{signed_names}\n{payload_sha256}",
        encode(path, true)
    );
    let date = &amz_date[..8];
    let scope = format!("{date}/{}/s3/aws4_request", c.region);
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let key = hmac(format!("AWS4{}", c.secret_key).as_bytes(), date);
    let key = hmac(&key, c.region);
    let key = hmac(&key, "s3");
    let key = hmac(&key, "aws4_request");
    let signature = hex(&hmac(&key, &to_sign));
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_names}, Signature={signature}",
        c.access_key
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: Credentials = Credentials {
        access_key: "AKIAIOSFODNN7EXAMPLE",
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        region: "us-east-1",
    };
    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    // The examples in Amazon's documentation ("Signature Calculations for the Authorization Header").
    #[test]
    fn get_object_with_a_range() {
        let got = authorization(
            &KEY,
            &Signed {
                method: "GET",
                host: "examplebucket.s3.amazonaws.com",
                path: "/test.txt",
                query: &[],
                headers: &[("Range", "bytes=0-9")],
                payload_sha256: EMPTY,
                amz_date: "20130524T000000Z",
            },
        );
        assert_eq!(
            got,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }

    #[test]
    fn put_object() {
        let body = "Welcome to Amazon S3.";
        let got = authorization(
            &KEY,
            &Signed {
                method: "PUT",
                host: "examplebucket.s3.amazonaws.com",
                path: "/test$file.text",
                query: &[],
                headers: &[
                    ("Date", "Fri, 24 May 2013 00:00:00 GMT"),
                    ("x-amz-storage-class", "REDUCED_REDUNDANCY"),
                ],
                payload_sha256: &sha256_hex(body.as_bytes()),
                amz_date: "20130524T000000Z",
            },
        );
        assert!(
            got.ends_with("Signature=98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd"),
            "{got}"
        );
    }

    #[test]
    fn list_objects_sorts_and_encodes_the_query() {
        let got = authorization(
            &KEY,
            &Signed {
                method: "GET",
                host: "examplebucket.s3.amazonaws.com",
                path: "/",
                query: &[("prefix", "J"), ("max-keys", "2")],
                headers: &[],
                payload_sha256: EMPTY,
                amz_date: "20130524T000000Z",
            },
        );
        assert!(
            got.ends_with("Signature=34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7"),
            "{got}"
        );
    }

    #[test]
    fn encoding_keeps_unreserved_and_slashes_only_in_paths() {
        assert_eq!(encode("a b/c~d-e_f.g", true), "a%20b/c~d-e_f.g");
        assert_eq!(encode("a b/c", false), "a%20b%2Fc");
        assert_eq!(encode("ü$", true), "%C3%BC%24");
    }
}

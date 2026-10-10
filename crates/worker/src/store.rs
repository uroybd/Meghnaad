//! [`ObjectStore`] over an R2 bucket binding.

use tc_core::{Error, ObjectStore, Result};
use worker::{Bucket, Conditional};

pub struct R2Store(pub Bucket);

fn err(e: worker::Error) -> Error {
    Error::Store(e.to_string())
}

impl ObjectStore for R2Store {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(obj) = self.0.get(name).execute().await.map_err(err)? else {
            return Ok(None);
        };
        let Some(body) = obj.body() else {
            return Ok(None);
        };
        Ok(Some(body.bytes().await.map_err(err)?))
    }

    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        self.0.put(name, value.to_vec()).execute().await.map_err(err)?;
        Ok(())
    }

    async fn del(&self, name: &str) -> Result<()> {
        self.0.delete(name).await.map_err(err)
    }

    async fn del_many(&self, names: &[String]) -> Result<()> {
        // One request for up to a thousand objects, instead of one each.
        for chunk in names.chunks(1000) {
            self.0.delete_multiple(chunk.to_vec()).await.map_err(err)?;
        }
        Ok(())
    }

    async fn list_dated(&self, prefix: &str) -> Result<Vec<(String, i64)>> {
        let mut names = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut req = self.0.list().prefix(prefix);
            if let Some(c) = cursor.take() {
                req = req.cursor(c);
            }
            let page = req.execute().await.map_err(err)?;
            names.extend(
                page.objects()
                    .iter()
                    .map(|o| (o.key(), (o.uploaded().as_millis() / 1000) as i64)),
            );
            match (page.truncated(), page.cursor()) {
                (true, Some(c)) => cursor = Some(c),
                _ => return Ok(names),
            }
        }
    }

    /// Compare-and-swap via R2 conditional puts: `etagMatches` for replacing an existing value,
    /// `etagDoesNotMatch: "*"` for create-if-absent. A failed condition makes `put` resolve to
    /// `null`, surfaced by workers-rs as `None`.
    async fn compare_and_swap(&self, name: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        let current = self.0.get(name).execute().await.map_err(err)?;
        let condition = match (current, expected) {
            (None, None) => Conditional {
                etag_does_not_match: Some("*".into()),
                ..Default::default()
            },
            (None, Some(_)) | (Some(_), None) => return Ok(false),
            (Some(obj), Some(expected)) => {
                let etag = obj.etag();
                let Some(body) = obj.body() else {
                    return Ok(false);
                };
                if body.bytes().await.map_err(err)? != expected {
                    return Ok(false);
                }
                Conditional {
                    etag_matches: Some(etag),
                    ..Default::default()
                }
            }
        };
        let stored = self
            .0
            .put(name, new.to_vec())
            .only_if(condition)
            .execute()
            .await
            .map_err(err)?;
        Ok(stored.is_some())
    }

    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        let Some(obj) = self.0.get(name).execute().await.map_err(err)? else {
            return Ok(None);
        };
        let etag = obj.etag();
        let Some(body) = obj.body() else {
            return Ok(None);
        };
        Ok(Some((body.bytes().await.map_err(err)?, etag)))
    }

    /// One conditional put: `etagMatches` to replace the version we read, `etagDoesNotMatch: "*"`
    /// to create. No read first, unlike [`compare_and_swap`](Self::compare_and_swap).
    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        let condition = match expected {
            None => Conditional {
                etag_does_not_match: Some("*".into()),
                ..Default::default()
            },
            Some(tag) => Conditional {
                etag_matches: Some(tag.to_owned()),
                ..Default::default()
            },
        };
        let stored = self
            .0
            .put(name, new.to_vec())
            .only_if(condition)
            .execute()
            .await
            .map_err(err)?;
        Ok(stored.is_some())
    }
}

//! Minimal object-storage abstraction the cloud server needs. The Worker implements it over an
//! R2 binding; tests use [`MemStore`].

use crate::error::{Error, Result};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[allow(async_fn_in_trait)] // Workers futures are !Send, so no Send bound is wanted.
pub trait ObjectStore {
    /// The object's value, or `None` if it does not exist.
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>>;

    /// Create or overwrite an object.
    async fn put(&self, name: &str, value: &[u8]) -> Result<()>;

    /// Delete an object; a missing object is not an error.
    async fn del(&self, name: &str) -> Result<()>;

    /// Names of all objects starting with `prefix`, across all pages.
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Atomically replace `name` with `new` iff its current value equals `expected` (`None`
    /// meaning "does not exist"). Returns whether the swap happened.
    async fn compare_and_swap(
        &self,
        name: &str,
        expected: Option<&[u8]>,
        new: &[u8],
    ) -> Result<bool>;

    /// The object's value together with an opaque tag naming exactly this version of it (an ETag).
    /// Hand the tag to [`swap_tagged`](Self::swap_tagged) to replace it only if nobody else has.
    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>>;

    /// Like [`compare_and_swap`](Self::compare_and_swap), but the expected state is a tag from
    /// [`get_tagged`](Self::get_tagged) (`None` = must not exist). One round trip instead of the
    /// read-then-write that `compare_and_swap` needs.
    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool>;
}

/// In-memory store for tests. Clones share the same underlying map, like handles to one bucket.
#[derive(Clone, Default)]
pub struct MemStore(Rc<RefCell<BTreeMap<String, Vec<u8>>>>);

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn names(&self) -> Vec<String> {
        self.0.borrow().keys().cloned().collect()
    }
}

impl ObjectStore for MemStore {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.0.borrow().get(name).cloned())
    }

    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        self.0.borrow_mut().insert(name.to_owned(), value.to_vec());
        Ok(())
    }

    async fn del(&self, name: &str) -> Result<()> {
        self.0.borrow_mut().remove(name);
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        Ok(self
            .0
            .borrow()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }

    async fn compare_and_swap(
        &self,
        name: &str,
        expected: Option<&[u8]>,
        new: &[u8],
    ) -> Result<bool> {
        let mut map = self.0.borrow_mut();
        if map.get(name).map(Vec::as_slice) != expected {
            return Ok(false);
        }
        map.insert(name.to_owned(), new.to_vec());
        Ok(true)
    }

    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        Ok(self.0.borrow().get(name).map(|v| (v.clone(), tag_of(v))))
    }

    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        let mut map = self.0.borrow_mut();
        if map.get(name).map(|v| tag_of(v)).as_deref() != expected {
            return Ok(false);
        }
        map.insert(name.to_owned(), new.to_vec());
        Ok(true)
    }
}

/// The tag [`MemStore`] hands out: the value itself, in hex.
fn tag_of(value: &[u8]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}

impl From<Error> for taskchampion::Error {
    fn from(e: Error) -> Self {
        taskchampion::Error::Server(e.to_string())
    }
}

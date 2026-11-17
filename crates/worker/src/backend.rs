//! Where the tasks and the settings files are kept: an R2 bucket (the default build) or a branch of a git repository
//! (built with the `git` feature). Two builds rather than a choice at run time, so each carries only its own.
//! Everything else in the Worker is the same for both.

use taskchampion::Server;
use tc_core::crypto::Cryptor;
use worker::Env;

#[allow(
    clippy::needless_pass_by_value,
    reason = "used as a `map_err` function, which hands the error over"
)]
fn rust_err(e: impl ToString) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

#[cfg(not(feature = "git"))]
mod imp {
    use super::{rust_err, Cryptor, Env, Server};
    use crate::store::R2Store;
    use tc_core::CloudServer;

    /// The settings files' store.
    pub type Store = R2Store;

    pub fn open(env: &Env) -> worker::Result<Store> {
        Ok(R2Store(env.bucket("TASKS")?))
    }

    /// The key's salt, written if this is the first client.
    pub async fn salt(env: &Env) -> worker::Result<Vec<u8>> {
        tc_core::load_salt(&open(env)?).await.map_err(rust_err)
    }

    /// The tasks as a TaskChampion server, opened with `cryptor`; `now` dates the cleanups.
    pub fn server(env: &Env, cryptor: Cryptor, now: fn() -> i64) -> worker::Result<Box<dyn Server>> {
        Ok(Box::new(
            CloudServer::with_cryptor(open(env)?, cryptor).with_cleanup(now),
        ))
    }

    /// Settings this build needs before it can do anything (names only).
    pub fn missing(_env: &Env) -> Vec<&'static str> {
        Vec::new()
    }
}

#[cfg(feature = "git")]
mod imp {
    use super::{rust_err, Cryptor, Env, Server};
    use crate::gitrepo::FetchHttp;
    use tc_core::forge::{ForgeRepo, GitRemote};
    use tc_core::gitsync::{GitObjects, GitServer};

    pub type Store = GitObjects<ForgeRepo<FetchHttp>>;

    /// A text variable, if set and not empty.
    fn var(env: &Env, name: &str) -> Option<String> {
        env.var(name)
            .ok()
            .map(|v| v.to_string())
            .filter(|v| !v.trim().is_empty())
    }

    /// The branch named by `GIT_API` (GitHub's by default), `GIT_REPO`, `GIT_BRANCH` (`taskchampion` by default) and the
    /// secret `GIT_TOKEN`.
    fn repo(env: &Env) -> worker::Result<ForgeRepo<FetchHttp>> {
        let token = env.secret("GIT_TOKEN").map(|s| s.to_string()).unwrap_or_default();
        let remote = GitRemote::new(
            &var(env, "GIT_API").unwrap_or_default(),
            &var(env, "GIT_REPO").unwrap_or_default(),
            &var(env, "GIT_BRANCH").unwrap_or_default(),
            &token,
        )
        .map_err(rust_err)?;
        Ok(ForgeRepo::new(FetchHttp, remote))
    }

    pub fn open(env: &Env) -> worker::Result<Store> {
        Ok(GitObjects(repo(env)?))
    }

    pub async fn salt(env: &Env) -> worker::Result<Vec<u8>> {
        tc_core::gitsync::load_salt(&repo(env)?).await.map_err(rust_err)
    }

    pub fn server(env: &Env, cryptor: Cryptor, now: fn() -> i64) -> worker::Result<Box<dyn Server>> {
        Ok(Box::new(GitServer::with_cryptor(repo(env)?, cryptor).with_cleanup(now)))
    }

    pub fn missing(env: &Env) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if var(env, "GIT_REPO").is_none() {
            missing.push("GIT_REPO");
        }
        if env.secret("GIT_TOKEN").is_err() {
            missing.push("GIT_TOKEN");
        }
        missing
    }
}

pub use imp::{missing, open, salt, server};

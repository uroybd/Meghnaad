//! A git branch reached through a forge's API (GitHub, Gitea, Forgejo: `tc_core::forge`), for the phone: the requests
//! are sent here, blocking, as the bucket's are (`s3.rs`). The token goes in a header and nowhere else.

use std::time::Duration;
use tc_core::forge::{ForgeRepo, GitRemote, Http, HttpRequest, HttpResponse};
use tc_core::{Error, Result};

pub struct UreqHttp {
    agent: ureq::Agent,
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqHttp {
    pub fn new() -> Self {
        UreqHttp {
            agent: ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(60)))
                .build()
                .into(),
        }
    }
}

impl Http for UreqHttp {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let mut builder = ureq::http::Request::builder().method(request.method).uri(&request.url);
        for (k, v) in &request.headers {
            builder = builder.header(*k, v);
        }
        let built = builder
            .body(request.body.unwrap_or_default())
            .map_err(|e| Error::Store(format!("building a request to the git server: {e}")))?;
        let mut response = self
            .agent
            .run(built)
            .map_err(|e| Error::Store(format!("could not reach the git server: {e}")))?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| Error::Store(format!("reading the git server's answer: {e}")))?;
        Ok(HttpResponse { status, body })
    }
}

/// The branch the settings name, checked (`GitRemote::new`: https, `owner/name`, a branch name, a token).
pub fn open(api: &str, repo: &str, branch: &str, token: &str) -> Result<ForgeRepo<UreqHttp>> {
    Ok(ForgeRepo::new(
        UreqHttp::new(),
        GitRemote::new(api, repo, branch, token)?,
    ))
}

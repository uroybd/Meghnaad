//! What a hooks script may reach beyond computing, on the phone: the network (`ureq`, with the script's timeout, no
//! automatic redirect and a cap on the answer) and its secrets. The runner has already refused what a script may not do
//! (a host it did not declare, plain http, too many requests); this is how the rest is done.

use crate::secrets::Secrets;
use async_trait::async_trait;
use std::io::Read;
use std::rc::Rc;
use std::time::Duration;
use tc_core::lua_hooks::{HttpRequest, HttpResponse, Net};

/// The most of an answer a script is given.
const MAX_RESPONSE: u64 = 1024 * 1024;

pub struct PhoneNet {
    pub secrets: Rc<Secrets>,
}

#[async_trait(?Send)]
impl Net for PhoneNet {
    async fn request(&self, r: HttpRequest) -> Result<HttpResponse, String> {
        // A redirect is the script's to follow (to a host it declared), not the network's to follow for it.
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(Duration::from_secs_f64(r.timeout_secs.clamp(0.1, 60.0))))
            .build()
            .into();
        let mut request = ureq::http::Request::builder().method(r.method.as_str()).uri(&r.url);
        for (k, v) in &r.headers {
            request = request.header(k.as_str(), v.as_str());
        }
        let request = request
            .body(r.body.clone().unwrap_or_default().into_bytes())
            .map_err(|e| format!("the request could not be made: {e}"))?;
        let mut response = agent.run(request).map_err(|e| {
            let text = e.to_string();
            if text.to_ascii_lowercase().contains("timeout") || text.contains("timed out") {
                "the request timed out".to_owned()
            } else {
                format!("the request failed: {text}")
            }
        })?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_ascii_lowercase(), v.to_str().ok()?.to_owned())))
            .collect();
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_RESPONSE + 1)
            .read_to_end(&mut body)
            .map_err(|_| "the answer could not be read".to_owned())?;
        if body.len() as u64 > MAX_RESPONSE {
            return Err("the answer is larger than 1 MB".into());
        }
        Ok(HttpResponse {
            status,
            headers,
            body: String::from_utf8_lossy(&body).into_owned(),
        })
    }

    async fn secret(&self, name: &str) -> Result<Option<String>, String> {
        self.secrets.get(name)
    }
}

//! What a script may reach beyond computing, on this computer: the network (`ureq`, with the script's timeout, no
//! automatic redirect and a cap on the answer) and its secrets, which are environment variables. `secret("JIRA_TOKEN")`
//! reads the variable `JIRA_TOKEN`, by that exact name and no other: the environment is shared with other programs, so
//! nothing is guessed from a prefix. The hosts it may be sent to are the variable `JIRA_TOKEN_HOSTS` (comma-separated);
//! without it the secret goes nowhere. The runner has already refused what a script may not do (a host it did not
//! declare, a secret to a host not its own, plain http, too many requests); this is how the rest is done.

use async_trait::async_trait;
use tc_core::lua_hooks::{HttpRequest, HttpResponse, Net};

pub struct HostNet;

#[async_trait(?Send)]
impl Net for HostNet {
    async fn request(&self, r: HttpRequest) -> Result<HttpResponse, String> {
        tc_native_net::send(r)
    }

    async fn secret(&self, name: &str) -> Result<Option<tc_core::hook_secrets::Secret>, String> {
        let text = |var: &str| match std::env::var(var) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(format!("the environment variable {var} is not text")),
        };
        let Some(value) = text(name)? else { return Ok(None) };
        let hosts_var = format!("{name}_HOSTS");
        let hosts = tc_core::hook_secrets::hosts_from_line(&text(&hosts_var)?.unwrap_or_default())
            .map_err(|why| format!("{hosts_var}: {why}"))?;
        Ok(Some(tc_core::hook_secrets::Secret { value, hosts }))
    }
}

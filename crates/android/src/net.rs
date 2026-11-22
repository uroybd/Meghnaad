//! What a hooks script may reach beyond computing, on the phone: the network (`ureq`, with the script's timeout, no
//! automatic redirect and a cap on the answer) and its secrets. The runner has already refused what a script may not do
//! (a host it did not declare, plain http, too many requests); this is how the rest is done.

use crate::secrets::Secrets;
use async_trait::async_trait;
use std::rc::Rc;
use tc_core::lua_hooks::{HttpRequest, HttpResponse, Net};

pub struct PhoneNet {
    pub secrets: Rc<Secrets>,
}

#[async_trait(?Send)]
impl Net for PhoneNet {
    async fn request(&self, r: HttpRequest) -> Result<HttpResponse, String> {
        tc_native_net::send(r)
    }

    async fn secret(&self, name: &str) -> Result<Option<tc_core::hook_secrets::Secret>, String> {
        self.secrets.get(name)
    }
}

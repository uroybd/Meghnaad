//! The user's hooks, which live on the Worker: this calls them there, one hook call a request (`POST /api/run`), and
//! gives the engine what the script answered. The script itself (and anything secret in it) never comes to the page.
//!
//! The engine knows which hooks the script defines (`set_hooks`), and only calls for those, so a command with no
//! matching hook makes no request. If the Worker can't be reached the hook is skipped with a warning that says so,
//! and the command goes on: a hook that refuses (`refuse("…")`, or an error in the script) still stops it.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use tc_core::dates::Clock;
use tc_core::hooks::{task_from_json, task_to_json, HookHost, Hooked, Reject};
use tc_core::model::Facts;

#[derive(Debug)]
pub struct RemoteHooks {
    base: String,
    defined: BTreeSet<String>,
    clock: Clock,
    dateformat: Option<String>,
}

impl RemoteHooks {
    pub fn new(base: String, defined: BTreeSet<String>, clock: Clock, dateformat: Option<String>) -> Self {
        RemoteHooks {
            base,
            defined,
            clock,
            dateformat,
        }
    }

    /// One call to the script. `Ok(Null)` when there is nothing to do, or the Worker could not be asked.
    async fn call(&self, h: &mut Hooked, name: &str, args: Vec<Value>) -> Result<Value, Reject> {
        if !self.defined.contains(name) {
            return Ok(Value::Null);
        }
        let body = json!({
            "fn": name,
            "args": args,
            "now": self.clock.now,
            "tz": self.clock.tz_offset,
            "week_starts_monday": self.clock.week_starts_monday,
            "dateformat": self.dateformat,
        });
        let sent = crate::server::fetch(
            "POST",
            &format!("{}/api/run", self.base),
            Some(body.to_string().as_bytes()),
        )
        .await;
        let reply = match sent {
            Ok(r) if r.status == 200 => r,
            Ok(r) => {
                h.warn(format!("Hook {name} skipped: the server answered {}.", r.status));
                return Ok(Value::Null);
            }
            Err(e) => {
                h.warn(format!("Hook {name} skipped: {e}."));
                return Ok(Value::Null);
            }
        };
        let answer: Value = match serde_json::from_slice(&reply.body) {
            Ok(a) => a,
            Err(_) => {
                h.warn(format!("Hook {name} skipped: the server's answer could not be read."));
                return Ok(Value::Null);
            }
        };
        for line in answer["say"].as_array().into_iter().flatten() {
            let text = line["text"].as_str().unwrap_or_default().to_owned();
            if line["kind"] == "warn" {
                h.warn(text);
            } else {
                h.say(text);
            }
        }
        if answer["ok"] == true {
            Ok(answer["value"].clone())
        } else {
            Err(answer["error"].as_str().unwrap_or("A hook failed.").to_owned())
        }
    }
}

#[async_trait(?Send)]
impl HookHost for RemoteHooks {
    async fn on_launch(&self, h: &mut Hooked, command: &str) -> Result<(), Reject> {
        self.call(h, "on_launch", vec![json!(command)]).await.map(|_| ())
    }

    async fn on_add(&self, h: &mut Hooked, task: Facts) -> Result<Facts, Reject> {
        let value = self.call(h, "on_add", vec![task_to_json(&task)]).await?;
        if value.is_null() {
            return Ok(task);
        }
        task_from_json(&value, &task)
    }

    async fn on_modify(&self, h: &mut Hooked, old: &Facts, new: Facts) -> Result<Facts, Reject> {
        let value = self
            .call(h, "on_modify", vec![task_to_json(old), task_to_json(&new)])
            .await?;
        if value.is_null() {
            return Ok(new);
        }
        task_from_json(&value, &new)
    }

    async fn on_exit(&self, h: &mut Hooked, changed: &[Facts]) {
        let tasks = Value::Array(changed.iter().map(task_to_json).collect());
        // `on_exit` can only talk: a failure in it is said, not a reason to stop.
        if let Err(message) = self.call(h, "on_exit", vec![tasks]).await {
            h.warn(message);
        }
    }
}

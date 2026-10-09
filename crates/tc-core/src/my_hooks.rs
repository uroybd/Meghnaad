//! Your hooks: the one file to edit. Fill in the placeholders and redeploy; they do nothing as shipped.
//!
//! What a hook receives, may change and may print is in [`crate::hooks`]. Keeping your code here, apart from the
//! engine that runs it, means pulling updates from upstream rarely touches this file, and a change to it is a
//! small patch (see "Keeping your hooks across updates" in `docs/using.md`).

// What the placeholders below use. Unused until you fill one in.
#[allow(unused_imports)]
use crate::hooks::{Hooked, Hooks, Reject};
#[allow(unused_imports)]
use crate::model::Facts;

/// Your hooks. Edit the functions; each one shows what is possible. They do nothing as shipped.
#[derive(Debug, Default)]
pub struct MyHooks;

impl Hooks for MyHooks {
    // Refuse a command, or just say something, before it runs:
    //
    // fn on_launch(&self, h: &mut Hooked, command: &str) -> Result<(), Reject> {
    //     if command.starts_with("purge") {
    //         return Err("No purging from the web.".into());
    //     }
    //     Ok(())
    // }

    // Fix a new task up: here, anything in project Work is also tagged +office.
    //
    // fn on_add(&self, h: &mut Hooked, mut task: Facts) -> Result<Facts, Reject> {
    //     if task.project.as_deref() == Some("Work") {
    //         task.tags.insert("office".into());
    //         h.say("Tagged +office.");
    //     }
    //     Ok(task)
    // }

    // Look at what changed, and refuse or adjust it: here, a task can't be finished while it is blocked.
    //
    // fn on_modify(&self, h: &mut Hooked, old: &Facts, new: Facts) -> Result<Facts, Reject> {
    //     if old.status == "pending" && new.status == "completed" && old.blocked {
    //         return Err("Finish what it depends on first.".into());
    //     }
    //     Ok(new)
    // }

    // Report on what the command did:
    //
    // fn on_exit(&self, h: &mut Hooked, changed: &[Facts]) {
    //     if !changed.is_empty() {
    //         h.say(format!("{} task(s) changed.", changed.len()));
    //     }
    // }
}

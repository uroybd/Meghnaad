#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]
#![cfg(feature = "lua")]
//! Hooks written in Lua, run through the real `execute`: each hook point, the task / tag set / date helpers a script
//! gets, refusing, what a script may and may not change, and the sandbox around it.

use std::sync::Arc;
use taskchampion::Replica;
use tc_core::cli::{execute, load_facts, CliResult, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::lua_hooks::{check, LuaHooks, LuaRunner};
use tc_core::taskrc::Config;
use tc_core::LiveStorage;

type R = Replica<LiveStorage>;

/// 2026-10-12 10:00 UTC, a Monday.
const NOW: i64 = 1_791_799_200;

fn clock() -> Clock {
    Clock::utc(NOW)
}

/// A new moment for every command (the tests share nothing, but tasks made in the same second would be numbered by
/// their random ids, not the order they were made in). The hooks keep seeing `NOW`.
fn later() -> Clock {
    use std::sync::atomic::{AtomicI64, Ordering};
    static STEP: AtomicI64 = AtomicI64::new(0);
    Clock::utc(NOW + STEP.fetch_add(1, Ordering::SeqCst))
}

fn replica() -> R {
    Replica::new(LiveStorage::new())
}

/// Run a command with the script's hooks. Returns what it answered and what the hooks printed.
#[allow(
    clippy::arc_with_non_send_sync,
    reason = "the engine runs on one thread; `Options.hooks` is an `Arc` for every kind of hook, and a Lua VM is not `Send`"
)]
async fn run(r: &mut R, script: &str, line: &str) -> (CliResult, Vec<String>) {
    let hooks = LuaHooks::load(script, clock()).unwrap_or_else(|p| panic!("the script does not load: {p:?}"));
    let mut undo = UndoStack::default();
    let opts = Options {
        confirmed: true,
        hooks: Some(Arc::new(hooks)),
        ..Options::default()
    };
    let done = execute(r, &Config::default(), later(), &split_words(line), opts, &mut undo).await;
    (done.result, done.feedback.into_iter().map(|l| l.text).collect())
}

fn message(r: &CliResult) -> String {
    match r {
        CliResult::Error { message } => message.clone(),
        CliResult::Changed { message, .. } => message.clone(),
        CliResult::Text { lines } => lines.join("\n"),
        other => format!("{other:?}"),
    }
}

async fn task(r: &mut R, description: &str) -> tc_core::model::Facts {
    load_facts(r)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.description == description)
        .unwrap_or_else(|| panic!("no task {description:?}"))
}

#[tokio::test(flavor = "current_thread")]
async fn on_add_changes_a_new_task_with_the_tag_set_and_says_something() {
    let mut r = replica();
    let script = r#"
        function on_add(task)
          if task:in_project("Work") then
            task.tags:add("office")
            say("tagged +office")
          end
        end
    "#;
    let (_, said) = run(&mut r, script, "add Write the report project:Work.Mail +x").await;
    assert_eq!(said, ["tagged +office"]);
    let t = task(&mut r, "Write the report").await;
    assert_eq!(t.tags.iter().cloned().collect::<Vec<_>>(), ["office", "x"]);
    let (_, said) = run(&mut r, script, "add Buy milk project:Home").await;
    assert!(said.is_empty());
    assert!(task(&mut r, "Buy milk").await.tags.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn a_hook_can_change_the_description_project_priority_and_remove_a_tag() {
    let mut r = replica();
    let script = r#"
        function on_add(task)
          task.description = task.description:upper()
          task.project = "Inbox"
          task.priority = "H"
          task.tags:remove("junk")
        end
    "#;
    run(&mut r, script, "add shout +junk +keep").await;
    let t = task(&mut r, "SHOUT").await;
    assert_eq!(t.project.as_deref(), Some("Inbox"));
    assert_eq!(t.priority.as_deref(), Some("H"));
    assert_eq!(t.tags.iter().cloned().collect::<Vec<_>>(), ["keep"]);
}

#[tokio::test(flavor = "current_thread")]
async fn returning_a_task_works_as_well_as_changing_the_one_given() {
    let mut r = replica();
    let script = r#"
        function on_add(task)
          task.description = task.description .. "!"
          return task
        end
    "#;
    run(&mut r, script, "add hello").await;
    task(&mut r, "hello!").await;
}

#[tokio::test(flavor = "current_thread")]
async fn dates_read_as_taskwarrior_reads_them_and_do_arithmetic() {
    let mut r = replica();
    let script = r#"
        function on_add(task)
          if task.due == nil then
            task.due = tomorrow()
          end
          task.wait = task.due - days(2)
          task.scheduled = date("2026-12-25") + hours(8) + minutes(30)
          task.expires = date("now+3d")
          say(tostring(task.due:start_of("day") == task.due), task.due.weekday, task.due.year, task.due.month, task.due.day)
          say(task.due:format("Y-M-D"), tostring(task.due > now()), tostring(task.due - now()))
        end
    "#;
    let (_, said) = run(&mut r, script, "add Plan").await;
    let t = task(&mut r, "Plan").await;
    // tomorrow, at the start of the day: 2026-10-13 00:00 UTC; wait is two days before that
    assert_eq!(t.due, Some(1_791_849_600));
    assert_eq!(t.wait, Some(1_791_849_600 - 2 * 86_400));
    assert_eq!(t.scheduled, Some(1_798_156_800 + 8 * 3600 + 30 * 60));
    assert_eq!(t.until, Some(NOW + 3 * 86_400));
    assert_eq!(said[0], "true 2 2026 10 13", "{said:?}");
    assert!(said[1].starts_with("2026-10-13 true "), "{said:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_task_that_has_a_due_date_keeps_it_and_date_helpers_compare() {
    let mut r = replica();
    let script = r#"
        function on_add(task)
          if task.due ~= nil and task.due < today() + days(3) then
            task.tags:add("soon")
          end
          assert(date_of(2026, 12, 25, 8, 30).hour == 8)
          assert(date("2026-12-25") > date("2026-12-24") and date("2026-12-25") >= date("2026-12-25"))
          assert((date("2026-12-25") - date("2026-12-24")):in_days() == 1)
          assert(tostring(days(2)) ~= nil and duration("3d"):in_hours() == 72)
          assert(date("2026-10-12"):is_weekend() == false and date("2026-10-17"):is_weekend() == true)
        end
    "#;
    run(&mut r, script, "add Pay rent due:2026-10-13").await;
    assert!(task(&mut r, "Pay rent").await.tags.contains("soon"));
    run(&mut r, script, "add Later due:2026-12-30").await;
    assert!(task(&mut r, "Later").await.tags.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn user_defined_attributes_are_fields_of_the_task_and_nil_removes_one() {
    let mut r = replica();
    let script = r#"
        function on_add(task)
          task.jira = "ABC-" .. 123
          task.points = 5
        end
    "#;
    run(&mut r, script, "add Ship it").await;
    let t = task(&mut r, "Ship it").await;
    assert_eq!(t.extra.get("jira").map(String::as_str), Some("ABC-123"));
    assert_eq!(t.extra.get("points").map(String::as_str), Some("5"));
}

#[tokio::test(flavor = "current_thread")]
async fn refusing_stops_the_command_with_the_message_and_nothing_is_saved() {
    let mut r = replica();
    let script = r#"
        function on_launch(command)
          if command:find("purge", 1, true) then refuse("No purging from the web.") end
        end
        function on_add(task)
          if task.description:find("secret", 1, true) then refuse("Nothing secret here.") end
        end
    "#;
    let (res, _) = run(&mut r, script, "add a secret plan").await;
    assert_eq!(message(&res), "Nothing secret here.");
    assert!(load_facts(&mut r).await.unwrap().is_empty());
    let (res, _) = run(&mut r, script, "purge 1").await;
    assert_eq!(message(&res), "No purging from the web.");
}

#[tokio::test(flavor = "current_thread")]
async fn on_modify_sees_the_task_before_and_after_and_can_refuse_a_change() {
    let mut r = replica();
    run(&mut r, "", "add First").await;
    run(&mut r, "", "add Second").await;
    run(&mut r, "", "2 modify depends:1").await;
    let script = r#"
        function on_modify(old, new)
          if old.status == "pending" and new.status == "completed" and old.blocked then
            refuse("Finish what it depends on first.")
          end
          if new.priority == "H" and old.priority ~= "H" then
            new.tags:add("urgent")
            say("marked urgent")
          end
        end
    "#;
    let (res, _) = run(&mut r, script, "2 done").await;
    assert_eq!(message(&res), "Finish what it depends on first.");
    assert_eq!(task(&mut r, "Second").await.status, "pending");
    let (res, said) = run(&mut r, script, "1 modify priority:H").await;
    assert_eq!(said, ["marked urgent"], "{res:?}");
    assert!(task(&mut r, "First").await.tags.contains("urgent"));
}

#[tokio::test(flavor = "current_thread")]
async fn on_exit_is_given_the_tasks_the_command_changed_and_can_only_talk() {
    let mut r = replica();
    let script = r#"
        function on_exit(changed)
          say(#changed .. " changed: " .. (changed[1] and changed[1].description or "-"))
          error("on_exit broke")
        end
    "#;
    let (res, said) = run(&mut r, script, "add Tidy up").await;
    assert!(matches!(res, CliResult::Changed { .. }), "{res:?}");
    assert_eq!(said[0], "1 changed: Tidy up");
    assert!(said[1].contains("on_exit broke"), "{said:?}");
    task(&mut r, "Tidy up").await;
}

#[tokio::test(flavor = "current_thread")]
async fn what_a_hook_may_not_change_is_refused_like_for_a_rust_hook() {
    let mut r = replica();
    let (res, _) = run(
        &mut r,
        r#"function on_add(task) task.status = "completed" end"#,
        "add Sneaky",
    )
    .await;
    assert_eq!(message(&res), "A hook may not change a task's status.");
    let (res, _) = run(
        &mut r,
        r#"function on_add(task) task.description = "" end"#,
        "add Blank",
    )
    .await;
    assert_eq!(message(&res), "A hook left the description empty.");
    let (res, _) = run(&mut r, r"function on_add(task) task.tags = 5 end", "add Odd").await;
    assert!(
        message(&res).contains("task.tags has to be a list of text"),
        "{}",
        message(&res)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_script_that_fails_says_where() {
    let mut r = replica();
    let (res, _) = run(
        &mut r,
        "function on_add(task)\n  local x = nil\n  return x.field\nend",
        "add Boom",
    )
    .await;
    let m = message(&res);
    assert!(m.contains("hooks:3:") || m.contains("attempt to index"), "{m}");
    assert!(load_facts(&mut r).await.unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn hooks_off_runs_none() {
    let mut r = replica();
    let script = r#"function on_add(task) refuse("no") end"#;
    let (res, _) = run(&mut r, script, "rc.hooks:off add Free").await;
    assert!(matches!(res, CliResult::Changed { .. }), "{res:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn the_sandbox_has_no_files_no_process_no_loading_and_stops_a_loop() {
    let runner = LuaRunner::load(
        r"
        function on_launch(command)
          say(type(io), type(os), type(require), type(dofile), type(load), type(debug), type(package))
        end
        function on_add(task)
          while true do end
        end
        ",
        clock(),
    )
    .unwrap();
    let said = runner.call("on_launch", &[serde_json::json!("add x")]).await.unwrap();
    assert_eq!(said.lines[0].text, "nil nil nil nil nil nil nil");
    let t = std::time::Instant::now();
    let failed = runner
        .call(
            "on_add",
            &[tc_core::hooks::task_to_json(&tc_core::model::Facts::default())],
        )
        .await
        .unwrap_err();
    assert!(failed.message.contains("ran for too long"), "{failed:?}");
    assert!(
        t.elapsed().as_secs() < 5,
        "the loop was not stopped quickly: {:?}",
        t.elapsed()
    );
    // And it can be used again afterwards.
    runner.call("on_launch", &[serde_json::json!("add y")]).await.unwrap();
}

#[test]
fn a_script_is_checked_with_the_line_of_what_is_wrong_and_the_hooks_it_defines() {
    assert_eq!(check("").unwrap(), Vec::<&str>::new());
    assert_eq!(
        check("function on_add(task) end\nfunction on_exit(changed) end").unwrap(),
        ["on_add", "on_exit"]
    );
    let p = check("function on_add(task)\n  task.tags:add(\n").unwrap_err();
    assert_eq!(p.line, Some(3), "{p:?}");
    let p = check("local x = 1\nerror('top level')\n").unwrap_err();
    assert_eq!(p.line, Some(2), "{p:?}");
    assert!(p.message.contains("top level"), "{p:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_set_counts_lists_and_iterates_in_order() {
    let runner = LuaRunner::load(
        r#"
        function on_add(task)
          task.tags:add("b", "a", "c")
          local out = {}
          for tag in task.tags:each() do out[#out + 1] = tag end
          say(table.concat(out, ","), #task.tags, task.tags:has_any("x", "a"), task.tags:has_all("a", "z"), tostring(task.tags))
          return task
        end
        "#,
        clock(),
    )
    .unwrap();
    let got = runner
        .call(
            "on_add",
            &[tc_core::hooks::task_to_json(&tc_core::model::Facts::default())],
        )
        .await
        .unwrap();
    assert_eq!(got.lines[0].text, "a,b,c 3 true false a, b, c");
}

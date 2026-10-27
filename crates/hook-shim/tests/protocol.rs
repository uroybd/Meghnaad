#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]
//! `meghnaad-hook` as `task` runs it: the program under the name of a hook, a task on its standard input, and what it
//! prints and exits with. Each test has its own data directory; none of it touches `~/.task`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_meghnaad-hook");

const TASK: &str = r#"{"id":1,"uuid":"11111111-1111-1111-1111-111111111111","status":"pending","description":"Pay rent","entry":"20261027T100000Z","modified":"20261027T100000Z","project":"Work","urgency":3}"#;

/// A data directory with `script` as its `hooks.lua`, and the program linked under `link`.
fn setup(name: &str, script: &str, link: &str) -> (PathBuf, PathBuf) {
    let data = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::remove_dir_all(&data).ok(); // left over from an earlier run, or not there
    std::fs::create_dir_all(data.join("hooks")).unwrap();
    std::fs::write(data.join("hooks.lua"), script).unwrap();
    let exe = data.join("hooks").join(link);
    std::os::unix::fs::symlink(BIN, &exe).unwrap();
    (data, exe)
}

/// What `task` does: the arguments it names itself with, the tasks on standard input.
fn run(data: &Path, exe: &Path, stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(exe);
    cmd.arg("api:2")
        .arg("args:task add Pay rent")
        .arg(format!("rc:{}", data.join("taskrc").display()))
        .arg(format!("data:{}", data.display()))
        .env_remove("MEGHNAAD_HOOKS")
        .env_remove("TASKRC")
        .env_remove("TASKDATA")
        .env("TZ", "UTC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn json_line(o: &Output) -> serde_json::Value {
    let text = out(o);
    let line = text
        .lines()
        .find(|l| l.starts_with('{'))
        .unwrap_or_else(|| panic!("no task in {text:?}"));
    serde_json::from_str(line).unwrap()
}

#[test]
fn on_add_changes_the_task_and_says_something() {
    let (data, exe) = setup(
        "add",
        r#"function on_add(task)
             if task:in_project("Work") then task.tags:add("office"); say("Tagged +office.") end
             task.due = date("2026-12-25")
           end"#,
        "on-add.meghnaad",
    );
    let o = run(&data, &exe, &format!("{TASK}\n"), &[]);
    assert!(o.status.success(), "{}", out(&o));
    assert!(out(&o).contains("Tagged +office."));
    let task = json_line(&o);
    assert_eq!(task["tags"], serde_json::json!(["office"]));
    assert_eq!(task["due"], "20261225T000000Z");
    assert_eq!(task["urgency"], 3, "what the script cannot see goes back as it came");
}

#[test]
fn dates_a_script_writes_are_in_this_computer_s_zone() {
    let (data, exe) = setup(
        "zone",
        r#"function on_add(task) task.due = date("2026-12-25") end"#,
        "on-add.meghnaad",
    );
    let o = run(&data, &exe, &format!("{TASK}\n"), &[("TZ", "Asia/Dhaka")]);
    assert_eq!(
        json_line(&o)["due"],
        "20261224T180000Z",
        "midnight in Dhaka (UTC+6) is 18:00 the day before in UTC"
    );
}

#[test]
fn on_modify_is_given_the_old_and_the_new_task() {
    let (data, exe) = setup(
        "modify",
        r#"function on_modify(old, new)
             if old.description ~= new.description then new.priority = "H" end
           end"#,
        "on-modify.meghnaad",
    );
    let new = TASK.replace("Pay rent", "Pay rent now");
    let o = run(&data, &exe, &format!("{TASK}\n{new}\n"), &[]);
    assert!(o.status.success(), "{}", out(&o));
    let task = json_line(&o);
    assert_eq!(task["description"], "Pay rent now");
    assert_eq!(task["priority"], "H");
}

#[test]
fn refusing_stops_the_command_with_the_message() {
    let (data, exe) = setup(
        "refuse",
        r#"function on_add(task) refuse("not today") end"#,
        "on-add.meghnaad",
    );
    let o = run(&data, &exe, &format!("{TASK}\n"), &[]);
    assert!(!o.status.success());
    assert_eq!(out(&o).trim(), "not today");
}

#[test]
fn on_launch_is_given_the_command_without_task() {
    let (data, exe) = setup(
        "launch",
        r#"function on_launch(command) if command == "add Pay rent" then refuse("saw " .. command) end end"#,
        "on-launch.meghnaad",
    );
    let o = run(&data, &exe, "", &[]);
    assert!(!o.status.success());
    assert_eq!(out(&o).trim(), "saw add Pay rent");
}

#[test]
fn on_exit_is_given_the_changed_tasks_and_cannot_stop_anything() {
    let (data, exe) = setup(
        "exit",
        r#"function on_exit(changed) say("changed " .. #changed) error("broken") end"#,
        "on-exit.meghnaad",
    );
    let o = run(&data, &exe, &format!("{TASK}\n{TASK}\n"), &[]);
    assert!(o.status.success(), "an on-exit hook cannot stop the command");
    assert!(
        out(&o).contains("changed 2") && out(&o).contains("broken"),
        "{}",
        out(&o)
    );
}

#[test]
fn a_secret_is_the_environment_variable_of_that_exact_name() {
    let script = r#"function on_add(task) task.description = "token " .. secret("JIRA_TOKEN") end"#;
    let (data, exe) = setup("secret", script, "on-add.meghnaad");
    let o = run(&data, &exe, &format!("{TASK}\n"), &[("JIRA_TOKEN", "s3cr3t")]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(
        json_line(&o)["description"],
        "token ***",
        "its value is kept out of what comes back"
    );
    // No prefix is tried: another variable that merely ends the same way is not it.
    let o = run(
        &data,
        &exe,
        &format!("{TASK}\n"),
        &[("HOOK_SECRET_JIRA_TOKEN", "x"), ("MY_JIRA_TOKEN", "x")],
    );
    assert!(!o.status.success());
    assert!(out(&o).contains("there is no secret called JIRA_TOKEN"), "{}", out(&o));
}

#[test]
fn a_script_can_call_a_web_service_it_declared() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let seen = std::thread::spawn(move || {
        let (mut conn, _) = server.accept().unwrap();
        let mut request = [0u8; 4096];
        let n = conn.read(&mut request).unwrap();
        let body = r#"{"key":"PAY-7"}"#;
        write!(
            conn,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        String::from_utf8_lossy(&request[..n]).into_owned()
    });
    let script = format!(
        r#"allow_hosts{{"localhost"}}
           function on_add(task)
             local res, err = http.post("http://localhost:{port}/issue", {{
               headers = {{ Authorization = "Bearer " .. secret("API_TOKEN") }},
               json = {{ summary = task.description }},
             }})
             if res == nil then refuse("unreachable: " .. err) end
             task.jira = res.json.key
           end"#
    );
    let (data, exe) = setup("http", &script, "on-add.meghnaad");
    let o = run(&data, &exe, &format!("{TASK}\n"), &[("API_TOKEN", "tok123")]);
    assert!(o.status.success(), "{}", out(&o));
    assert_eq!(json_line(&o)["jira"], "PAY-7");
    let request = seen.join().unwrap().to_ascii_lowercase();
    assert!(request.starts_with("post /issue"), "{request}");
    assert!(request.contains("authorization: bearer tok123"), "{request}");
    assert!(request.contains("pay rent"), "{request}");
}

#[test]
fn a_host_the_script_did_not_declare_is_refused() {
    let (data, exe) = setup(
        "undeclared",
        r#"function on_add(task) http.get("https://example.com/") end"#,
        "on-add.meghnaad",
    );
    let o = run(&data, &exe, &format!("{TASK}\n"), &[]);
    assert!(!o.status.success());
}

#[test]
fn a_hook_the_script_does_not_define_hands_the_task_back() {
    let (data, exe) = setup("undefined", "function on_exit() end", "on-add.meghnaad");
    let o = run(&data, &exe, &format!("{TASK}\n"), &[]);
    assert!(o.status.success());
    assert_eq!(out(&o).trim(), TASK);
}

#[test]
fn a_script_that_does_not_load_stops_the_command_with_its_line() {
    let (data, exe) = setup("broken", "function on_add(task)\n  x = = 1\nend", "on-add.meghnaad");
    let o = run(&data, &exe, &format!("{TASK}\n"), &[]);
    assert!(!o.status.success());
    assert!(out(&o).contains("line 2"), "{}", out(&o));
}

#[test]
fn install_links_the_hooks_the_script_defines_and_uninstall_removes_them() {
    let (data, _) = setup("install", "function on_add(task) end\nfunction on_exit() end", "stale");
    std::fs::remove_file(data.join("hooks").join("stale")).unwrap();
    std::fs::remove_file(data.join("hooks.lua")).unwrap();
    let script = data.join("mine.lua");
    std::fs::write(&script, "function on_add(task) end\nfunction on_exit() end").unwrap();
    let run_cli = |args: &[&str]| {
        Command::new(BIN)
            .args(args)
            .arg("--data")
            .arg(&data)
            .env_remove("MEGHNAAD_HOOKS")
            .output()
            .unwrap()
    };
    let o = run_cli(&["install", script.to_str().unwrap()]);
    assert!(o.status.success(), "{}", out(&o));
    let mut names: Vec<_> = std::fs::read_dir(data.join("hooks"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["on-add.meghnaad", "on-exit.meghnaad"]);
    assert_eq!(
        std::fs::canonicalize(data.join("hooks.lua")).unwrap(),
        std::fs::canonicalize(&script).unwrap()
    );
    assert!(run_cli(&["uninstall"]).status.success());
    assert_eq!(std::fs::read_dir(data.join("hooks")).unwrap().count(), 0);
}

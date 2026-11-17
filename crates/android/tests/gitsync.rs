//! Phones, the `task` CLI and a git repository on a real forge (GitHub, Gitea or Forgejo), which these tests cannot
//! start themselves. Each run uses a branch of its own, so a repository can be used again. Run them with
//!
//! ```text
//! TC_GIT_API=http://127.0.0.1:3917/api/v1 TC_GIT_REPO=tester/tasks TC_GIT_TOKEN=… \
//!   cargo test -p tc-android --test gitsync -- --ignored
//! ```
//!
//! `TC_GIT_API` is `https://api.github.com` for GitHub, or a Gitea/Forgejo's `…/api/v1`. For the CLI test, also
//! `TC_GIT_REMOTE`: the repository's address for `git push` (with the credentials in it, or an SSH one that works without
//! a prompt). It runs `task` 3.5.0 with a taskrc and data in a folder of its own, never anyone's.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tc_android::engine::Engine;
use tc_core::gitsync::{GitRepo, Meta, META};

const NOW: i64 = 1_791_799_200;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is needed"))
}

/// A branch no other run has used.
fn fresh_branch() -> String {
    format!("tc-test-{}", uuid::Uuid::new_v4().simple())
}

fn settings(branch: &str, secret: &str) -> Value {
    json!({
        "tasks": { "kind": "git", "api": var("TC_GIT_API"), "repo": var("TC_GIT_REPO"), "branch": branch,
                   "token": var("TC_GIT_TOKEN") },
        "encryption_secret": secret,
    })
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tc-android-git-{}-{name}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn engine() -> Engine {
    Engine::new(scratch("phone"), &[1; 32])
}

fn run(engine: &mut Engine, line: &str) -> Value {
    serde_json::from_str(&engine.run(&json!({ "line": line, "now": NOW }).to_string())).unwrap()
}

fn descriptions(engine: &mut Engine) -> Vec<String> {
    let text = run(engine, "export")["result"]["text"]
        .as_str()
        .unwrap_or("[]")
        .to_owned();
    let mut found: Vec<String> = serde_json::from_str::<Vec<Value>>(&text)
        .unwrap()
        .iter()
        .map(|t| t["description"].as_str().unwrap().to_owned())
        .collect();
    found.sort();
    found
}

fn branch(branch: &str) -> impl GitRepo {
    tc_android::gitrepo::open(&var("TC_GIT_API"), &var("TC_GIT_REPO"), branch, &var("TC_GIT_TOKEN")).unwrap()
}

#[test]
#[ignore = "needs a git forge and a token (TC_GIT_API, TC_GIT_REPO, TC_GIT_TOKEN)"]
fn phones_share_a_branch() {
    let name = fresh_branch();
    let s = settings(&name, "s3cret").to_string();
    let (mut a, mut b) = (engine(), engine());
    run(&mut a, "add Buy milk");
    a.sync(&s).unwrap();
    b.sync(&s).unwrap();
    assert_eq!(descriptions(&mut b), ["Buy milk"]);
    // Both change; the second to push is told it is behind, pulls, and goes on.
    run(&mut a, "add From a");
    run(&mut b, "add From b");
    a.sync(&s).unwrap();
    b.sync(&s).unwrap();
    a.sync(&s).unwrap();
    assert_eq!(descriptions(&mut a), ["Buy milk", "From a", "From b"]);
    assert_eq!(descriptions(&mut a), descriptions(&mut b));

    // The branch is the CLI's layout.
    let repo = branch(&name);
    let (_, meta) = pollster::block_on(repo.read(META)).unwrap();
    let meta = Meta::parse(&meta.unwrap().bytes).unwrap();
    let (_, files) = pollster::block_on(repo.root()).unwrap();
    let versions = files.iter().filter(|e| e.path.starts_with("v-")).count();
    // "Buy milk", "From a", then "From b" after b was told it was behind and pulled.
    assert_eq!(versions, 3, "{files:?}");
    assert!(files
        .iter()
        .any(|e| e.path.ends_with(&meta.latest_version.as_simple().to_string())));
    // When a file last changed, which a cleanup goes by: just now, and never for one that is not there.
    let changed = pollster::block_on(repo.last_changed(&[META.to_owned(), "nothing-here".to_owned()])).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(changed[0].is_some_and(|t| (now - t).abs() < 600), "{changed:?}");
    assert_eq!(changed[1], None);

    // A wrong secret does not open it.
    assert!(engine().sync(&settings(&name, "not it").to_string()).is_err());
}

#[test]
#[ignore = "needs a git forge and a token (TC_GIT_API, TC_GIT_REPO, TC_GIT_TOKEN)"]
fn settings_files_go_on_the_branch_beside_the_tasks() {
    let name = fresh_branch();
    let mut s = settings(&name, "s3cret");
    // Everything the app shares: the taskrc folder and what it adds up to, settings.json and the hooks.
    s["files"] = json!(["web/config.json", "web/settings.json", "web/hooks.lua"]);
    let (mut a, mut b) = (engine(), engine());
    for (method, path, body) in [
        ("PUT", "/api/settings", &br#"{"homepage":"next"}"#[..]),
        (
            "PUT",
            "/api/config/taskrc",
            b"default.command=next\ninclude themes/dark.theme\n",
        ),
        ("PUT", "/api/config/files/themes/dark.theme", b"color.active=bold\n"),
        ("PUT", "/api/hooks", b"function on_add(t) return t end\n"),
    ] {
        let saved = a.api(method, path, body);
        assert!(saved.contains("200"), "{path}: {saved}");
    }
    a.sync(&s.to_string()).unwrap();
    let mut pulled = b.sync(&s.to_string()).unwrap();
    pulled.sort();
    // config.json is not pulled: b rebuilds it from the taskrc files it pulled, and it comes out the same.
    assert_eq!(
        pulled,
        [
            "web/hooks.lua",
            "web/rc/taskrc",
            "web/rc/themes/dark.theme",
            "web/settings.json"
        ]
    );
    assert!(b.api("GET", "/api/settings", b"").contains("next"));
    assert!(b
        .api("GET", "/api/config/files/themes/dark.theme", b"")
        .contains("color.active=bold"));
    assert!(b.api("GET", "/api/hooks", b"").contains("on_add"));
    assert_eq!(b.api("GET", "/api/config", b""), a.api("GET", "/api/config", b""));
    let repo = branch(&name);
    for path in [
        "web/settings.json",
        "web/rc/taskrc",
        "web/rc/themes/dark.theme",
        "web/config.json",
        "web/hooks.lua",
    ] {
        let (_, file) = pollster::block_on(repo.read(path)).unwrap();
        assert!(file.is_some(), "{path} is not on the branch");
    }

    // Both phones change settings.json; the later change wins on both, and b keeps its own to restore.
    let saved = b.api("PUT", "/api/settings", br#"{"homepage":"from-b"}"#);
    assert!(saved.contains("200"), "{saved}");
    std::thread::sleep(std::time::Duration::from_secs(2));
    let saved = a.api("PUT", "/api/settings", br#"{"homepage":"from-a-later"}"#);
    assert!(saved.contains("200"), "{saved}");
    a.sync(&s.to_string()).unwrap();
    b.sync(&s.to_string()).unwrap();
    assert!(b.api("GET", "/api/settings", b"").contains("from-a-later"));
}

/// The real `task` 3.5.0 and a phone on one branch, both ways, including a snapshot the CLI writes and the phone starts
/// from.
#[test]
#[ignore = "needs a git forge, a token, TC_GIT_REMOTE and the task CLI"]
fn the_task_cli_and_a_phone_share_a_branch() {
    let name = fresh_branch();
    let remote = var("TC_GIT_REMOTE");
    let dir = scratch("cli");
    let rc = dir.join("taskrc");
    std::fs::write(
        &rc,
        format!(
            "data.location={}\nsync.git.local_path={}\nsync.git.branch={name}\nsync.git.remote={remote}\n\
             sync.encryption_secret=s3cret\nconfirmation=off\nverbose=nothing\n",
            dir.join("data").display(),
            dir.join("clone").display()
        ),
    )
    .unwrap();
    let task = |args: &[&str]| -> String {
        let out = std::process::Command::new("task")
            .env("TASKRC", &rc)
            .env("TASKDATA", dir.join("data"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "task {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let data = task(&["_get", "rc.data.location"]);
    assert!(
        Path::new(data.trim()).starts_with(&dir),
        "the CLI's data is {data}: not running anything"
    );
    assert!(task(&["--version"]).starts_with("3.5"), "this needs task 3.5.0");

    let s = settings(&name, "s3cret").to_string();
    // The phone starts the branch; the CLI joins it.
    let mut phone = engine();
    run(&mut phone, "add From the phone");
    phone.sync(&s).unwrap();
    task(&["sync"]);
    task(&["add", "From the CLI"]);
    task(&["sync"]);
    phone.sync(&s).unwrap();
    assert_eq!(descriptions(&mut phone), ["From the CLI", "From the phone"]);

    // Changes go both ways.
    run(&mut phone, "1 done");
    phone.sync(&s).unwrap();
    task(&["sync"]);
    let pending = task(&[
        "rc.report.all.columns=description",
        "rc.report.all.labels=D",
        "status:pending",
        "all",
    ]);
    assert!(
        !pending.contains("From the CLI") || !pending.contains("From the phone"),
        "{pending}"
    );

    // Enough versions that the CLI writes a snapshot; a new phone starts from it.
    for i in 0..105 {
        task(&["add", &format!("n{i}")]);
        task(&["sync"]);
    }
    let (_, snapshot) = pollster::block_on(branch(&name).read("snapshot")).unwrap();
    assert!(snapshot.is_some(), "the CLI wrote no snapshot");
    let mut late = engine();
    late.sync(&s).unwrap();
    phone.sync(&s).unwrap();
    assert_eq!(descriptions(&mut late), descriptions(&mut phone));
    assert_eq!(descriptions(&mut late).len(), 107);
}

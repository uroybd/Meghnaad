//! Phones and a TaskChampion sync server: the engine syncs through the real client against a server that keeps the protocol
//! (`/v1/client/…`, `X-Client-Id`, a single chain of versions, snapshots on request), kept in this process.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tc_android::engine::Engine;

const NIL: &str = "00000000-0000-0000-0000-000000000000";
const CLIENT: &str = "3f2b8d1e-9c4a-4b7e-8f21-5a6d0c9e1b34";

#[derive(Default)]
struct Client {
    /// `(version, parent, sealed body)`, oldest first.
    versions: Vec<(String, String, Vec<u8>)>,
    snapshot: Option<(String, Vec<u8>)>,
}

#[derive(Default)]
struct Server {
    clients: BTreeMap<String, Client>,
    /// What `X-Snapshot-Request` says after a version is added.
    snapshot_request: Option<&'static str>,
    next: u128,
    /// Requests with no `X-Client-Id`.
    anonymous: usize,
}

/// A server on a local port. Returns its address and the shared state.
fn serve() -> (String, Arc<Mutex<Server>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let state = Arc::new(Mutex::new(Server::default()));
    let shared = state.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let state = shared.clone();
            std::thread::spawn(move || handle(stream, &state));
        }
    });
    (url, state)
}

fn handle(stream: std::net::TcpStream, state: &Mutex<Server>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut out = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = line.split_whitespace();
        let (method, path) = (parts.next().unwrap().to_owned(), parts.next().unwrap().to_owned());
        let mut headers = BTreeMap::new();
        loop {
            let mut h = String::new();
            reader.read_line(&mut h).unwrap();
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                headers.insert(k.to_ascii_lowercase(), v.trim().to_owned());
            }
        }
        let mut body = vec![0; headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0)];
        reader.read_exact(&mut body).unwrap();

        let mut server = state.lock().unwrap();
        let (status, extra, payload) = answer(&mut server, &method, &path, &headers, body);
        drop(server);
        let head = format!(
            "HTTP/1.1 {status} X\r\n{extra}Content-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            payload.len()
        );
        if out.write_all(head.as_bytes()).is_err() || out.write_all(&payload).is_err() {
            return;
        }
    }
}

fn answer(
    server: &mut Server,
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: Vec<u8>,
) -> (u16, String, Vec<u8>) {
    let Some(client_id) = headers.get("x-client-id") else {
        server.anonymous += 1;
        return (400, String::new(), b"no client id".to_vec());
    };
    let Some(rest) = path.strip_prefix("/v1/client/") else {
        return (404, String::new(), Vec::new());
    };
    let request = server.snapshot_request;
    server.next += 1;
    let fresh = format!("{:032x}", server.next);
    let fresh = format!(
        "{}-{}-{}-{}-{}",
        &fresh[..8],
        &fresh[8..12],
        &fresh[12..16],
        &fresh[16..20],
        &fresh[20..]
    );
    let client = server.clients.entry(client_id.clone()).or_default();
    match (method, rest) {
        ("POST", r) if r.starts_with("add-version/") => {
            let parent = &r["add-version/".len()..];
            let latest = client.versions.last().map_or(NIL, |v| v.0.as_str()).to_owned();
            if parent != latest {
                return (409, format!("X-Parent-Version-Id: {latest}\r\n"), Vec::new());
            }
            assert_eq!(
                headers.get("content-type").map(String::as_str),
                Some("application/vnd.taskchampion.history-segment")
            );
            client.versions.push((fresh.clone(), parent.to_owned(), body));
            let ask = request.map_or(String::new(), |r| format!("X-Snapshot-Request: {r}\r\n"));
            (200, format!("X-Version-Id: {fresh}\r\n{ask}"), Vec::new())
        }
        ("GET", r) if r.starts_with("get-child-version/") => {
            let parent = &r["get-child-version/".len()..];
            match client.versions.iter().find(|v| v.1 == parent) {
                Some((id, parent, body)) => (
                    200,
                    format!(
                        "X-Version-Id: {id}\r\nX-Parent-Version-Id: {parent}\r\nContent-Type: application/vnd.taskchampion.history-segment\r\n"
                    ),
                    body.clone(),
                ),
                None => (404, String::new(), Vec::new()),
            }
        }
        ("POST", r) if r.starts_with("add-snapshot/") => {
            client.snapshot = Some((r["add-snapshot/".len()..].to_owned(), body));
            (200, String::new(), Vec::new())
        }
        ("GET", "snapshot") => match &client.snapshot {
            Some((id, body)) => (
                200,
                format!("X-Version-Id: {id}\r\nContent-Type: application/vnd.taskchampion.snapshot\r\n"),
                body.clone(),
            ),
            None => (404, String::new(), Vec::new()),
        },
        _ => (404, String::new(), Vec::new()),
    }
}

fn settings(url: &str, secret: &str) -> String {
    json!({
        "tasks": { "kind": "server", "url": url, "client_id": CLIENT },
        "encryption_secret": secret,
    })
    .to_string()
}

fn settings_with(url: &str, secret: &str, switch: &str) -> String {
    let mut s: Value = serde_json::from_str(&settings(url, secret)).unwrap();
    s["switch"] = json!(switch);
    s.to_string()
}

const NOW: i64 = 1_791_799_200;

/// An engine keeping its settings in a folder of its own.
fn engine() -> Engine {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Engine::new(
        std::env::temp_dir().join(format!("tc-android-server-{}-{n}", std::process::id())),
        &[1; 32],
    )
}

fn run(engine: &mut Engine, line: &str) -> Value {
    let answer = engine.run(&json!({ "line": line, "now": NOW }).to_string());
    serde_json::from_str(&answer).unwrap()
}

fn descriptions(engine: &mut Engine) -> Vec<String> {
    let answer = run(engine, "export");
    let text = answer["result"]["text"].as_str().unwrap_or("[]").to_owned();
    let mut found: Vec<String> = serde_json::from_str::<Vec<Value>>(&text)
        .unwrap()
        .iter()
        .map(|t| t["description"].as_str().unwrap().to_owned())
        .collect();
    found.sort();
    found
}

#[test]
fn two_phones_converge_through_the_server() {
    let (url, state) = serve();
    let (mut a, mut b) = (engine(), engine());
    run(&mut a, "add Buy milk");
    a.sync(&settings(&url, "s3cret")).unwrap();
    b.sync(&settings(&url, "s3cret")).unwrap();
    assert_eq!(descriptions(&mut b), ["Buy milk"]);
    run(&mut b, "add Call Sam");
    run(&mut a, "add Write report");
    b.sync(&settings(&url, "s3cret")).unwrap();
    a.sync(&settings(&url, "s3cret")).unwrap();
    b.sync(&settings(&url, "s3cret")).unwrap();
    assert_eq!(descriptions(&mut a), ["Buy milk", "Call Sam", "Write report"]);
    assert_eq!(descriptions(&mut b), descriptions(&mut a));
    assert_eq!(state.lock().unwrap().anonymous, 0);
    // What the server holds is sealed: no task text in it.
    let server = state.lock().unwrap();
    let held: Vec<u8> = server.clients[CLIENT]
        .versions
        .iter()
        .flat_map(|v| v.2.clone())
        .collect();
    assert!(!String::from_utf8_lossy(&held).contains("Buy milk"));
}

#[test]
fn a_snapshot_the_server_asks_for_starts_a_new_phone() {
    let (url, state) = serve();
    state.lock().unwrap().snapshot_request = Some("urgency=high");
    let mut a = engine();
    run(&mut a, "add Buy milk");
    a.sync(&settings(&url, "s3cret")).unwrap();
    assert!(state.lock().unwrap().clients[CLIENT].snapshot.is_some());
    // The new phone starts from the snapshot, and goes on from there.
    let mut b = engine();
    b.sync(&settings(&url, "s3cret")).unwrap();
    assert_eq!(descriptions(&mut b), ["Buy milk"]);
}

#[test]
fn a_wrong_secret_or_a_wrong_address_is_an_error_not_a_crash() {
    let (url, _state) = serve();
    let mut a = engine();
    run(&mut a, "add Buy milk");
    a.sync(&settings(&url, "s3cret")).unwrap();
    let mut b = engine();
    let wrong = b.sync(&settings(&url, "other")).unwrap_err();
    assert!(!wrong.is_empty());
    // The same phone, with the right secret, is fine.
    b.sync(&settings(&url, "s3cret")).unwrap();
    assert_eq!(descriptions(&mut b), ["Buy milk"]);

    // Nothing there at all (a phone that has not synced anywhere yet has no other place to be asked about).
    let mut b = engine();
    let unreachable = b.sync(&settings("http://127.0.0.1:1", "s3cret")).unwrap_err();
    assert!(unreachable.contains("could not reach the sync server"), "{unreachable}");
    // Not an address of the right kind, or not a client id.
    assert!(b
        .sync(&settings("http://example.com", "s"))
        .unwrap_err()
        .contains("https://"));
    let mut bad_id: Value = serde_json::from_str(&settings(&url, "s")).unwrap();
    bad_id["tasks"]["client_id"] = json!("mine");
    assert!(b.sync(&bad_id.to_string()).unwrap_err().contains("client id"));
}

#[test]
fn something_that_is_not_a_sync_server_says_so_when_there_is_something_to_send() {
    // A server that does not know the paths answers 404 with a page.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0; 4096];
            // The request itself is of no interest to this canned answer.
            let _request_len = stream.read(&mut buf).unwrap_or(0);
            let page = "<html>Not found</html>";
            write!(
                stream,
                "HTTP/1.1 404 X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                page.len()
            )
            .ok();
        }
    });
    let mut a = engine();
    run(&mut a, "add Buy milk");
    let why = a.sync(&settings(&url, "s")).unwrap_err();
    assert!(why.contains("404") && why.contains("check the address"), "{why}");
}

#[test]
fn settings_that_share_files_need_a_bucket_to_keep_them_in() {
    let (url, _state) = serve();
    let mut a = engine();
    let mut s: Value = serde_json::from_str(&settings(&url, "s3cret")).unwrap();
    s["files"] = json!(["web/hooks.lua"]);
    let why = a.sync(&s.to_string()).unwrap_err();
    assert!(why.contains("needs a bucket"), "{why}");
}

#[test]
fn a_phone_asks_before_it_goes_to_another_server_and_then_keeps_or_lets_go_of_its_tasks() {
    let (one, _) = serve();
    let (two, _) = serve();
    let (mut a, mut other) = (engine(), engine());
    run(&mut a, "add On the phone");
    a.sync(&settings(&one, "s3cret")).unwrap();

    // Someone else already keeps tasks on the second server.
    run(&mut other, "add Already there");
    other.sync(&settings(&two, "s3cret")).unwrap();

    // Without an answer nothing happens, and the phone still works where it was.
    let ask = a.sync(&settings(&two, "s3cret")).unwrap_err();
    assert!(ask.starts_with("backend-changed"), "{ask}");
    assert!(ask.contains(&one) && ask.contains(&two), "{ask}");
    assert_eq!(descriptions(&mut a), ["On the phone"]);
    a.sync(&settings(&one, "s3cret")).unwrap();

    // A switch that does not work leaves the phone as it was.
    let failed = a
        .sync(&settings_with("http://127.0.0.1:1", "s3cret", "replace"))
        .unwrap_err();
    assert!(failed.contains("could not reach"), "{failed}");
    assert_eq!(descriptions(&mut a), ["On the phone"]);

    // Merging: both kinds of task are on the phone and on the server.
    a.sync(&settings_with(&two, "s3cret", "merge")).unwrap();
    assert_eq!(descriptions(&mut a), ["Already there", "On the phone"]);
    other.sync(&settings(&two, "s3cret")).unwrap();
    assert_eq!(descriptions(&mut other), ["Already there", "On the phone"]);
    // And it is a phone of the second server now: no question again.
    a.sync(&settings(&two, "s3cret")).unwrap();

    // Replacing: the phone has what the server has, and what it had alone is let go.
    let (three, _) = serve();
    let mut third = engine();
    run(&mut third, "add Only on three");
    third.sync(&settings(&three, "s3cret")).unwrap();
    a.sync(&settings_with(&three, "s3cret", "replace")).unwrap();
    assert_eq!(descriptions(&mut a), ["Only on three"]);
}

#[test]
fn a_merge_onto_an_empty_server_makes_the_phones_tasks_its_first_history() {
    let (one, _) = serve();
    let (two, _) = serve();
    let mut a = engine();
    run(&mut a, "add Buy milk project:Home +errand");
    run(&mut a, "add Call Sam due:2026-12-01");
    a.sync(&settings(&one, "s3cret")).unwrap();
    a.sync(&settings_with(&two, "s3cret", "merge")).unwrap();
    // A new phone gets them, with what they carried.
    let mut b = engine();
    b.sync(&settings(&two, "s3cret")).unwrap();
    assert_eq!(descriptions(&mut b), ["Buy milk", "Call Sam"]);
    let milk = run(&mut b, "project:Home export");
    let text = milk["result"]["text"].as_str().unwrap();
    assert!(text.contains("errand"), "{text}");
}

#[test]
fn a_phone_with_no_tasks_goes_to_another_server_without_asking() {
    let (one, _) = serve();
    let (two, _) = serve();
    let mut a = engine();
    a.sync(&settings(&one, "s3cret")).unwrap();
    a.sync(&settings(&two, "s3cret")).unwrap();
}

/// Against the real `taskchampion-sync-server`, which these tests cannot start themselves: run it (for instance
/// `taskchampion-sync-server --listen 127.0.0.1:8099 --data-dir <empty folder> --snapshot-versions 2`) and run this with
/// `TC_SYNC_SERVER=http://127.0.0.1:8099 cargo test -p tc-android --test syncserver -- --ignored`.
#[test]
#[ignore = "needs a running taskchampion-sync-server (TC_SYNC_SERVER)"]
fn the_real_server_and_phones_agree() {
    let url = std::env::var("TC_SYNC_SERVER").expect("TC_SYNC_SERVER is the server's address");
    // A client of its own for each run, since the server keeps what it is given.
    let id = uuid::Uuid::new_v4().to_string();
    let settings = |secret: &str| {
        json!({ "tasks": { "kind": "server", "url": url, "client_id": id }, "encryption_secret": secret }).to_string()
    };
    let (mut a, mut b) = (engine(), engine());
    run(&mut a, "add Buy milk");
    a.sync(&settings("s3cret")).unwrap();
    b.sync(&settings("s3cret")).unwrap();
    run(&mut b, "add Call Sam");
    for _ in 0..4 {
        run(&mut a, &format!("add more {}", uuid::Uuid::new_v4()));
        a.sync(&settings("s3cret")).unwrap();
    }
    b.sync(&settings("s3cret")).unwrap();
    a.sync(&settings("s3cret")).unwrap();
    assert_eq!(descriptions(&mut a), descriptions(&mut b));
    assert!(descriptions(&mut a).contains(&"Call Sam".to_owned()));
    // A phone that starts now begins from a snapshot or from the whole history: either way it has everything.
    let mut c = engine();
    c.sync(&settings("s3cret")).unwrap();
    assert_eq!(descriptions(&mut c), descriptions(&mut a));
    // And a wrong secret does not open it.
    let mut d = engine();
    assert!(d.sync(&settings("not the secret")).is_err());
}

/// The real `task` CLI and a phone, on one real server. `TC_CLI_TASKRC` is a taskrc made for this (its `data.location`
/// next to it, and `sync.server.url`, `sync.server.client_id` and `sync.encryption_secret` the same as the three
/// variables below), because this runs `task add` and must never touch anyone's own data: it refuses a taskrc whose
/// data is not in the folder the taskrc is in.
#[test]
#[ignore = "needs a running taskchampion-sync-server, the task CLI and a taskrc made for it"]
fn the_task_cli_and_a_phone_agree() {
    let var = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("{n} is needed"));
    let (url, id, secret) = (var("TC_SYNC_SERVER"), var("TC_SYNC_CLIENT_ID"), var("TC_SYNC_SECRET"));
    let rc = std::path::PathBuf::from(var("TC_CLI_TASKRC"));
    let task = |args: &[&str]| {
        let out = std::process::Command::new("task")
            .env("TASKRC", &rc)
            .args(["rc.confirmation=off", "rc.verbose=nothing"])
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
        std::path::Path::new(data.trim()).starts_with(rc.parent().unwrap()),
        "the CLI's data is {data}, not next to {rc:?}: not running anything"
    );
    let settings =
        json!({ "tasks": { "kind": "server", "url": url, "client_id": id }, "encryption_secret": secret }).to_string();

    task(&["add", "From the CLI"]);
    task(&["sync"]);
    let mut phone = engine();
    phone.sync(&settings).unwrap();
    assert_eq!(descriptions(&mut phone), ["From the CLI"]);

    run(&mut phone, "add From the phone");
    phone.sync(&settings).unwrap();
    task(&["sync"]);
    let listed = task(&["rc.report.all.columns=description", "rc.report.all.labels=D", "all"]);
    assert!(
        listed.contains("From the phone") && listed.contains("From the CLI"),
        "{listed}"
    );
}

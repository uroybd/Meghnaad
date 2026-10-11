//! Two phones and a bucket: the engine syncs through the real S3 client against a bucket that answers like S3 and R2 do
//! (signed requests, conditional puts, a paged listing), kept in this process.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tc_android::engine::Engine;

#[derive(Default)]
struct Bucket {
    objects: BTreeMap<String, (Vec<u8>, u64)>,
    next_tag: u64,
    /// Every request's `Authorization` header, to see that each one was signed.
    unsigned: usize,
}

/// A bucket named `tasks` on a local port. Returns the endpoint and the shared state.
fn serve() -> (String, Arc<Mutex<Bucket>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let state = Arc::new(Mutex::new(Bucket::default()));
    let shared = state.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let state = shared.clone();
            std::thread::spawn(move || handle(stream, &state));
        }
    });
    (endpoint, state)
}

fn handle(stream: std::net::TcpStream, state: &Mutex<Bucket>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut out = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = line.split_whitespace();
        let (method, target) = (parts.next().unwrap().to_owned(), parts.next().unwrap().to_owned());
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

        let (path, query) = target.split_once('?').unwrap_or((&target, ""));
        let key = path.strip_prefix("/tasks/").unwrap_or("").to_owned();
        let mut bucket = state.lock().unwrap();
        if !headers
            .get("authorization")
            .is_some_and(|a| a.starts_with("AWS4-HMAC-SHA256 Credential=AK/"))
        {
            bucket.unsigned += 1;
        }
        let (status, extra, payload): (u16, String, Vec<u8>) = match method.as_str() {
            "GET" if key.is_empty() => {
                let prefix = query
                    .split('&')
                    .find_map(|p| p.strip_prefix("prefix="))
                    .unwrap_or("")
                    .replace("%2F", "/");
                let mut xml = String::from("<ListBucketResult><IsTruncated>false</IsTruncated>");
                for k in bucket.objects.keys().filter(|k| k.starts_with(&prefix)) {
                    xml += &format!(
                        "<Contents><Key>{k}</Key><LastModified>2026-10-11T20:33:15.000Z</LastModified></Contents>"
                    );
                }
                xml += "</ListBucketResult>";
                (200, String::new(), xml.into_bytes())
            }
            "GET" => match bucket.objects.get(&key) {
                Some((v, tag)) => (200, format!("ETag: \"t{tag}\"\r\n"), v.clone()),
                None => (404, String::new(), b"<Error><Code>NoSuchKey</Code></Error>".to_vec()),
            },
            "PUT" => {
                let current = bucket.objects.get(&key).map(|(_, t)| format!("\"t{t}\""));
                let refused = match (headers.get("if-none-match"), headers.get("if-match")) {
                    (Some(_), _) => current.is_some(),
                    (_, Some(tag)) => current.as_ref() != Some(tag),
                    _ => false,
                };
                if refused {
                    (
                        412,
                        String::new(),
                        b"<Error><Code>PreconditionFailed</Code></Error>".to_vec(),
                    )
                } else {
                    bucket.next_tag += 1;
                    let tag = bucket.next_tag;
                    bucket.objects.insert(key, (body, tag));
                    (200, format!("ETag: \"t{tag}\"\r\n"), Vec::new())
                }
            }
            "DELETE" => {
                bucket.objects.remove(&key);
                (204, String::new(), Vec::new())
            }
            _ => (405, String::new(), Vec::new()),
        };
        drop(bucket);
        let head = format!(
            "HTTP/1.1 {status} X\r\n{extra}Content-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            payload.len()
        );
        if out.write_all(head.as_bytes()).is_err() || out.write_all(&payload).is_err() {
            return;
        }
    }
}

fn settings(endpoint: &str, secret: &str) -> String {
    json!({
        "endpoint": endpoint,
        "bucket": "tasks",
        "access_key": "AK",
        "secret_key": "SK",
        "encryption_secret": secret,
    })
    .to_string()
}

const NOW: i64 = 1_791_799_200;

/// An engine keeping its settings in a folder of its own.
fn engine() -> Engine {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Engine::new(
        std::env::temp_dir().join(format!("tc-android-sync-{}-{n}", std::process::id())),
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
fn two_phones_converge_through_the_bucket() {
    let (endpoint, bucket) = serve();
    let s = settings(&endpoint, "a passphrase");
    let (mut a, mut b) = (engine(), engine());

    // The first phone adds a task and gets it up; the second pulls it.
    assert_eq!(run(&mut a, "add Buy milk")["wrote"], true);
    a.sync(&s).unwrap();
    b.sync(&s).unwrap();
    assert_eq!(descriptions(&mut b), ["Buy milk"]);

    // Each adds one while offline, and after both have synced, both have both.
    run(&mut a, "add Call the bank");
    run(&mut b, "add Water the plants");
    b.sync(&s).unwrap();
    a.sync(&s).unwrap();
    b.sync(&s).unwrap();
    let want = ["Buy milk", "Call the bank", "Water the plants"];
    assert_eq!(descriptions(&mut a), want);
    assert_eq!(descriptions(&mut b), want);

    // What reached the bucket is sealed, and every request was signed.
    let bucket = bucket.lock().unwrap();
    assert_eq!(bucket.unsigned, 0);
    assert!(bucket.objects.len() > 3);
    for (value, _) in bucket.objects.values() {
        assert!(!String::from_utf8_lossy(value).contains("Buy milk"));
    }
}

#[test]
fn a_replica_saved_and_restored_goes_on_syncing() {
    let (endpoint, _bucket) = serve();
    let s = settings(&endpoint, "a passphrase");
    let mut a = engine();
    run(&mut a, "add Pay rent");
    a.sync(&s).unwrap();
    let saved = a.save().unwrap();

    let mut again = engine();
    again.restore(&saved).unwrap();
    assert_eq!(descriptions(&mut again), ["Pay rent"]);
    run(&mut again, "add Book the dentist");
    again.sync(&s).unwrap();
    let mut b = engine();
    b.sync(&s).unwrap();
    assert_eq!(descriptions(&mut b), ["Book the dentist", "Pay rent"]);
    assert!(engine().restore(b"not a replica").is_err());
}

#[test]
fn a_wrong_secret_or_an_unreachable_bucket_is_an_error_not_a_crash() {
    let (endpoint, _bucket) = serve();
    let mut a = engine();
    run(&mut a, "add Secret plans");
    a.sync(&settings(&endpoint, "right")).unwrap();

    let mut b = engine();
    let wrong = b.sync(&settings(&endpoint, "wrong")).unwrap_err();
    assert!(!wrong.is_empty());
    // Nothing of the bucket got into the second replica.
    assert!(descriptions(&mut b).is_empty());
    assert!(b.sync("{").unwrap_err().contains("invalid sync settings"));
    let gone = b.sync(&settings("http://127.0.0.1:1", "right")).unwrap_err();
    assert!(gone.contains("could not reach the bucket"), "{gone}");
    // And with the right secret it works.
    b.sync(&settings(&endpoint, "right")).unwrap();
    assert_eq!(descriptions(&mut b), ["Secret plans"]);
}

#[test]
fn the_jni_calls_answer_in_json() {
    let mut engine = engine();
    let call = |e: &mut Engine, op: &str, arg: &str| -> Value {
        serde_json::from_str(&tc_android::jni::call(e, op, arg)).unwrap()
    };
    assert_eq!(call(&mut engine, "changes", ""), json!({ "changes": 0 }));
    assert!(call(&mut engine, "config", "nonsense")["error"].is_string());
    assert!(call(&mut engine, "bogus", "")["error"]
        .as_str()
        .unwrap()
        .contains("unknown call"));
    let added = call(
        &mut engine,
        "run",
        &json!({ "line": "add One", "now": NOW }).to_string(),
    );
    assert_eq!(added["wrote"], true);
    assert_eq!(call(&mut engine, "changes", "")["changes"], 1);
    let imported = call(
        &mut engine,
        "import",
        &json!({ "text": "[]", "apply": false, "tz": 0, "now": NOW }).to_string(),
    );
    assert!(imported.get("error").is_none(), "{imported}");
}

fn api(engine: &mut Engine, method: &str, path: &str, body: &str) -> (u64, Value) {
    let answer: Value = serde_json::from_str(&engine.api(method, path, body.as_bytes())).unwrap();
    let body = answer["body"].as_str().unwrap();
    (
        answer["status"].as_u64().unwrap(),
        serde_json::from_str(body).unwrap_or(Value::String(body.to_owned())),
    )
}

#[test]
fn the_settings_routes_of_the_web_app_are_answered_from_files() {
    let mut engine = engine();
    // Nothing saved yet: the defaults, and `{}` for the app's own settings.
    let (status, config) = api(&mut engine, "GET", "/api/config", "");
    assert_eq!(status, 200);
    assert_eq!(config["has_previous"], false);
    assert_eq!(api(&mut engine, "GET", "/api/settings", "").1, json!({}));

    // A taskrc is kept as the allowlisted subset, and what was dropped is named.
    let rc = "uda.estimate.type=numeric\nuda.estimate.label=Est\nsync.encryption_secret=hunter2\nreport.mine.columns=id,description\nreport.mine.filter=status:pending\n";
    let (status, summary) = api(&mut engine, "PUT", "/api/config/taskrc", rc);
    assert_eq!(status, 200);
    assert_eq!(summary["udas"], 1);
    assert_eq!(summary["blocked"], json!(["sync.encryption_secret"]));
    assert!(!summary.to_string().contains("hunter2"));
    let (_, text) = api(&mut engine, "GET", "/api/config/taskrc", "");
    assert!(text.as_str().unwrap().contains("uda.estimate.type"));
    assert!(!text.as_str().unwrap().contains("hunter2"));

    // The second save keeps the first for a restore, and restoring swaps back.
    api(&mut engine, "PUT", "/api/config/taskrc", "uda.size.type=string\n");
    assert_eq!(api(&mut engine, "GET", "/api/config", "").1["has_previous"], true);
    assert_eq!(api(&mut engine, "POST", "/api/config/taskrc/restore", "").1["udas"], 1);

    // What a `config` command saves can't be a credential.
    let bad = json!({ "version": 1, "config": { "settings": { "sync.aws.secret_access_key": "x" } } }).to_string();
    assert_eq!(api(&mut engine, "PUT", "/api/config/engine", &bad).0, 400);

    // The app's settings are stored as written, if they are valid.
    assert_eq!(api(&mut engine, "PUT", "/api/settings", "not json").0, 400);
    assert_eq!(api(&mut engine, "PUT", "/api/settings", "{\"theme\":\"dark\"}").0, 200);
    assert_eq!(api(&mut engine, "GET", "/api/settings", "").1["theme"], "dark");

    // Hooks: a script that does not load is refused; one that does is kept and its hooks named.
    let broken = api(
        &mut engine,
        "PUT",
        "/api/hooks",
        "function on_add(task)\n  task.tags:add(\n",
    );
    assert_eq!(broken.0, 400);
    assert!(broken.1["error"].as_str().unwrap().contains("not saved"));
    assert_eq!(
        api(&mut engine, "PUT", "/api/hooks", "function on_add(task) end").1["defined"],
        json!(["on_add"])
    );
    assert_eq!(
        api(&mut engine, "GET", "/api/hooks/defined", "").1["defined"],
        json!(["on_add"])
    );
    assert_eq!(
        api(&mut engine, "GET", "/api/hooks", "").1["source"],
        "function on_add(task) end"
    );
    assert_eq!(api(&mut engine, "POST", "/api/hooks/check", "x =").1["ok"], false);

    // A route the app does not have is a 404, not a crash.
    assert_eq!(api(&mut engine, "GET", "/api/tc/pull", "").0, 404);
}

#[test]
fn hooks_run_on_the_phone_and_secrets_are_listed_by_name_only() {
    let mut engine = engine();
    let script = r#"
        function on_add(task)
          if task:in_project("Work") then task.tags:add("office") say("Tagged +office.") end
        end
        function on_modify(old, new)
          if new.priority == "H" then refuse("No high priority today.") end
        end
    "#;
    assert_eq!(api(&mut engine, "PUT", "/api/hooks", script).0, 200);
    let added = run(&mut engine, "add Write the report project:Work.Mail");
    assert_eq!(added["wrote"], true);
    assert!(added["feedback"].to_string().contains("Tagged +office."), "{added}");
    assert!(descriptions(&mut engine).contains(&"Write the report".to_owned()));
    let tagged = run(&mut engine, "+office count");
    assert!(tagged.to_string().contains('1'), "{tagged}");

    // A hook that refuses stops the command.
    let refused = run(&mut engine, "1 modify priority:H");
    assert_eq!(refused["wrote"], false);
    assert!(refused.to_string().contains("No high priority today."), "{refused}");

    // Removing the script stops the hooks at the next command.
    assert_eq!(api(&mut engine, "PUT", "/api/hooks", "").0, 200);
    assert_eq!(run(&mut engine, "1 modify priority:H")["wrote"], true);

    // Secrets: the names come back, never the values.
    assert_eq!(api(&mut engine, "PUT", "/api/secrets/jira", "tok-123").0, 200);
    assert_eq!(api(&mut engine, "PUT", "/api/secrets/9bad", "x").0, 400);
    let (_, names) = api(&mut engine, "GET", "/api/secrets", "");
    assert_eq!(names["names"], json!(["jira"]));
    assert!(!names.to_string().contains("tok-123"));
    assert_eq!(api(&mut engine, "DELETE", "/api/secrets/jira", "").0, 200);
    assert_eq!(api(&mut engine, "GET", "/api/secrets", "").1["names"], json!([]));
}

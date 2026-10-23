//! The taskrc as a folder, through the settings routes: files kept as written, `include` followed, sync and
//! credential lines never stored or served, one version to go back to, and `config` changes written into the main
//! taskrc only.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use serde_json::{json, Value};
use tc_core::store::{MemStore, ObjectStore};
use tc_core::webapi::{WebSettings, CONFIG_KEY, RC_PREFIX};

struct Api {
    web: WebSettings<MemStore>,
    store: MemStore,
}

impl Api {
    fn new() -> Api {
        let store = MemStore::new();
        Api {
            web: WebSettings::new(store.clone()),
            store,
        }
    }

    async fn call(&self, method: &str, path: &str, body: &[u8]) -> (u16, String) {
        let r = self.web.handle(method, path, body).await.expect("a settings route");
        (r.status, String::from_utf8(r.body).unwrap())
    }

    async fn ok(&self, method: &str, path: &str, body: &str) -> String {
        let (status, text) = self.call(method, path, body.as_bytes()).await;
        assert_eq!(status, 200, "{method} {path}: {text}");
        text
    }

    async fn put(&self, path: &str, body: &str) -> Value {
        serde_json::from_str(&self.ok("PUT", path, body).await).unwrap()
    }

    async fn every_stored_byte(&self) -> String {
        let mut all = String::new();
        for name in self.store.names() {
            all.push_str(&name);
            all.push('\n');
            all.push_str(&String::from_utf8_lossy(&self.store.get(&name).await.unwrap().unwrap()));
            all.push('\n');
        }
        all
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_first_save_leaves_nothing_to_restore() {
    let api = Api::new();
    api.put("/api/config/taskrc", "bulk=3\n").await;
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert_eq!(config["has_previous"], false);
    assert_eq!(api.call("POST", "/api/config/taskrc/restore", b"").await.0, 400);
    api.put("/api/config/taskrc", "bulk=4\n").await;
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert_eq!(config["has_previous"], true);
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_comes_back_as_it_was_written() {
    let api = Api::new();
    let text = "# my taskrc\r\n\r\nbulk=3   # three\r\n  default.project =  P1\r\nweekstart=Monday";
    let summary = api.put("/api/config/taskrc", text).await;
    assert_eq!(summary["text"], text);
    assert_eq!(api.ok("GET", "/api/config/taskrc", "").await, text);
    // And the settings in it are the ones in force.
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert_eq!(config["config"]["settings"]["bulk"], "3");
    assert_eq!(config["config"]["settings"]["weekstart"], "Monday");
}

#[tokio::test(flavor = "current_thread")]
async fn sync_and_credential_lines_are_never_stored_or_served() {
    let api = Api::new();
    let text = "bulk=3\nsync.encryption_secret=hunter2\n# sync.server.client_id = abc-123\n\
                bulk=4 # sync.server.origin=https://example.com\nuda.x.api_key=zzz\nweekstart=Monday\n";
    let summary = api.put("/api/config/taskrc", text).await;
    assert_eq!(summary["text"], "bulk=3\nweekstart=Monday\n");
    let blocked = summary["blocked"].to_string();
    assert!(blocked.contains("sync.encryption_secret"), "{blocked}");
    for secret in ["hunter2", "abc-123", "example.com", "zzz"] {
        assert!(!summary.to_string().contains(secret), "{secret} in the answer");
        assert!(!api.every_stored_byte().await.contains(secret), "{secret} stored");
        assert!(!api.ok("GET", "/api/config/taskrc", "").await.contains(secret));
        assert!(!api.ok("GET", "/api/config", "").await.contains(secret));
    }
    // The same through a file in a folder, an import, and a config change from the engine.
    let one = api
        .put("/api/config/files/holidays/x.rc", "token=hunter2\nholiday.a.name=A\n")
        .await;
    assert_eq!(one["text"], "holiday.a.name=A\n");
    let import = json!({ "files": [{ "path": "y.rc", "text": "password=hunter2\n" }] });
    let done = api.put_post("/api/config/files/import", &import.to_string()).await;
    assert_eq!(done["blocked"], json!(["password"]));
    assert!(!api.every_stored_byte().await.contains("hunter2"));
    let engine = json!({ "settings": { "sync.encryption_secret": "hunter2" } });
    let (status, body) = api
        .call("PUT", "/api/config/engine", engine.to_string().as_bytes())
        .await;
    assert_eq!(status, 400, "{body}");
    let engine = json!({ "contexts": { "w": { "name": "w", "rc": { "sync.server.origin": "x" } } } });
    let (status, _) = api
        .call("PUT", "/api/config/engine", engine.to_string().as_bytes())
        .await;
    assert_eq!(status, 400, "a context's own setting is checked too");
}

impl Api {
    async fn put_post(&self, path: &str, body: &str) -> Value {
        serde_json::from_str(&self.ok("POST", path, body).await).unwrap()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn what_the_app_cannot_use_is_commented_out_not_lost() {
    let api = Api::new();
    let summary = api
        .put("/api/config/taskrc", "bulk=3\nsome.unknown.key=1\nnot a setting\n")
        .await;
    assert_eq!(
        summary["text"],
        "bulk=3\n# [unsupported] some.unknown.key=1\n# [unsupported] not a setting\n"
    );
    assert_eq!(summary["commented"].as_array().unwrap().len(), 2);
    assert_eq!(summary["commented"][0]["line"], 2);
}

#[tokio::test(flavor = "current_thread")]
async fn a_folder_with_includes_adds_up_and_files_can_be_listed_edited_and_deleted() {
    let api = Api::new();
    api.put(
        "/api/config/files/holidays/en.rc",
        "holiday.towel.name=Towel day\nholiday.towel.date=20260525\n",
    )
    .await;
    api.put("/api/config/taskrc", "calendar.holidays=full\ninclude holidays/en.rc\n")
        .await;
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert_eq!(config["config"]["settings"]["holiday.towel.name"], "Towel day");

    let list: Value = serde_json::from_str(&api.ok("GET", "/api/config/files", "").await).unwrap();
    let paths: Vec<&str> = list["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["holidays/en.rc", "taskrc"]);
    assert_eq!(list["files"][1]["includes"][0]["found"], "holidays/en.rc");
    assert_eq!(list["problems"], json!([]));

    // A name with a space, written the way a page sends it.
    api.put("/api/config/files/my%20notes.rc", "bulk=2\n").await;
    assert_eq!(api.ok("GET", "/api/config/files/my%20notes.rc", "").await, "bulk=2\n");
    let list: Value = serde_json::from_str(&api.ok("GET", "/api/config/files", "").await).unwrap();
    assert_eq!(list["files"][1]["path"], "my notes.rc");
    assert_eq!(list["files"][1]["used"], false, "nothing includes it");

    // Deleting a file that is included leaves a problem, not a crash.
    api.ok("DELETE", "/api/config/files/holidays/en.rc", "").await;
    let list: Value = serde_json::from_str(&api.ok("GET", "/api/config/files", "").await).unwrap();
    assert!(list["problems"][0]["message"]
        .as_str()
        .unwrap()
        .contains("no such file"));
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert!(config["config"]["settings"].get("holiday.towel.name").is_none());

    assert_eq!(api.call("DELETE", "/api/config/files/taskrc", b"").await.0, 400);
    assert_eq!(api.call("DELETE", "/api/config/files/nothere.rc", b"").await.0, 404);
    for bad in [
        "/api/config/files/..%2Fx",
        "/api/config/files/a%5Cb",
        "/api/config/files/%2Fetc",
    ] {
        assert_eq!(api.call("PUT", bad, b"bulk=1").await.0, 400, "{bad}");
    }
    assert_eq!(api.call("PATCH", "/api/config/files/a.rc", b"").await.0, 405);
}

#[tokio::test(flavor = "current_thread")]
async fn a_folder_is_imported_in_one_step_and_one_restore_undoes_it() {
    let api = Api::new();
    api.put("/api/config/taskrc", "bulk=1\n").await;
    let folder = json!({ "mode": "replace", "files": [
        { "path": "taskrc", "text": "bulk=5\ninclude lists/a.rc\n" },
        { "path": "lists/a.rc", "text": "default.project=A # kept\n" },
    ]});
    let done = api.put_post("/api/config/files/import", &folder.to_string()).await;
    assert_eq!(done["text"], "bulk=5\ninclude lists/a.rc\n");
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert_eq!(config["config"]["settings"]["default.project"], "A");
    assert_eq!(config["has_previous"], true);

    // Replacing needs a main taskrc, and a bad path refuses the whole import.
    let no_root = json!({ "mode": "replace", "files": [{ "path": "a.rc", "text": "bulk=1\n" }] });
    assert_eq!(
        api.call("POST", "/api/config/files/import", no_root.to_string().as_bytes())
            .await
            .0,
        400
    );
    let bad = json!({ "files": [{ "path": "ok.rc", "text": "bulk=1\n" }, { "path": "../x", "text": "" }] });
    assert_eq!(
        api.call("POST", "/api/config/files/import", bad.to_string().as_bytes())
            .await
            .0,
        400
    );
    assert!(
        api.call("GET", "/api/config/files/ok.rc", b"").await.0 == 404,
        "nothing of a refused import is kept"
    );

    // Back to before, and forward again.
    let back: Value = serde_json::from_str(&api.ok("POST", "/api/config/taskrc/restore", "").await).unwrap();
    assert_eq!(back["text"], "bulk=1\n");
    assert_eq!(api.call("GET", "/api/config/files/lists/a.rc", b"").await.0, 404);
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    assert_eq!(config["config"]["settings"]["bulk"], "1");
    assert!(config["config"]["settings"].get("default.project").is_none());
    let fwd: Value = serde_json::from_str(&api.ok("POST", "/api/config/taskrc/restore", "").await).unwrap();
    assert_eq!(fwd["text"], "bulk=5\ninclude lists/a.rc\n");
}

#[tokio::test(flavor = "current_thread")]
async fn config_changes_go_to_the_main_taskrc_and_keep_its_comments() {
    let api = Api::new();
    api.put("/api/config/files/a.rc", "default.project=A\nbulk=5\n").await;
    api.put("/api/config/taskrc", "# mine\nbulk=3   # three\ninclude a.rc\n")
        .await;
    let config: Value = serde_json::from_str(&api.ok("GET", "/api/config", "").await).unwrap();
    let mut cfg = config["config"].clone();
    cfg["settings"]["bulk"] = json!("7");
    cfg["settings"]["weekstart"] = json!("Monday");
    let done = api.put("/api/config/engine", &cfg.to_string()).await;
    assert_eq!(
        api.ok("GET", "/api/config/taskrc", "").await,
        "# mine\nbulk=7   # three\ninclude a.rc\nweekstart=Monday\n"
    );
    assert_eq!(
        api.ok("GET", "/api/config/files/a.rc", "").await,
        "default.project=A\nbulk=5\n"
    );
    // `bulk=5` in the included file comes later than `bulk=7`, so it still wins; the answer says what is in force.
    assert_eq!(done["config"]["settings"]["bulk"], "5");
    assert_eq!(done["config"]["settings"]["weekstart"], "Monday");
}

#[tokio::test(flavor = "current_thread")]
async fn settings_saved_before_the_folder_existed_are_still_there() {
    let api = Api::new();
    // What an earlier version wrote: only the settings, no files.
    let bytes = tc_core::webapi::encode(&tc_core::taskrc::parse("bulk=6\nweekstart=Monday\n").config).unwrap();
    api.store.put(CONFIG_KEY, &bytes).await.unwrap();
    assert_eq!(
        api.ok("GET", "/api/config/taskrc", "").await,
        "bulk=6\nweekstart=Monday\n"
    );
    assert!(
        api.store.list(RC_PREFIX).await.unwrap().is_empty(),
        "reading writes nothing"
    );
    api.put("/api/config/taskrc", "# now mine\nbulk=6\nweekstart=Monday\n")
        .await;
    assert_eq!(api.store.list(RC_PREFIX).await.unwrap(), ["web/rc/taskrc"]);
}

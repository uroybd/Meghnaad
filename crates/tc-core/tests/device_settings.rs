//! Each device's own settings (spec 0027) through the settings routes: listed, read, written, renamed and deleted as
//! `web/devices/<id>.json`, with the same checks as `settings.json` and IDs that are file names.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]

use serde_json::{json, Value};
use tc_core::store::{MemStore, ObjectStore};
use tc_core::webapi::{device_id_problem, device_key, WebSettings};

async fn call(web: &WebSettings<MemStore>, method: &str, path: &str, body: &str) -> (u16, Value) {
    let r = web
        .handle(method, path, body.as_bytes())
        .await
        .expect("a settings route");
    (r.status, serde_json::from_slice(&r.body).unwrap_or(Value::Null))
}

#[tokio::test(flavor = "current_thread")]
async fn a_device_is_written_listed_read_renamed_and_deleted() {
    let store = MemStore::new();
    let web = WebSettings::new(store.clone());
    assert_eq!(
        call(&web, "GET", "/api/settings/devices", "").await,
        (200, json!({ "devices": [] }))
    );
    assert_eq!(call(&web, "GET", "/api/settings/devices/phone", "").await.0, 404);

    let mine = r#"{"version":2,"deviceId":"phone","homepage":{"report":"next"}}"#;
    assert_eq!(call(&web, "PUT", "/api/settings/devices/phone", mine).await.0, 200);
    assert_eq!(store.get(&device_key("phone")).await.unwrap().unwrap(), mine.as_bytes());
    call(&web, "PUT", "/api/settings/devices/laptop", "{}").await;
    assert_eq!(
        call(&web, "GET", "/api/settings/devices", "").await.1,
        json!({ "devices": ["laptop", "phone"] })
    );
    assert_eq!(
        call(&web, "GET", "/api/settings/devices/phone", "").await.1["homepage"],
        json!({ "report": "next" })
    );

    // A rename writes the new file and takes the old one away.
    let (status, _) = call(&web, "PUT", "/api/settings/devices/pixel?from=phone", mine).await;
    assert_eq!(status, 200);
    assert!(store.get(&device_key("phone")).await.unwrap().is_none());
    assert!(store.get(&device_key("pixel")).await.unwrap().is_some());

    assert_eq!(call(&web, "DELETE", "/api/settings/devices/laptop", "").await.0, 200);
    assert_eq!(
        call(&web, "GET", "/api/settings/devices", "").await.1,
        json!({ "devices": ["pixel"] })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_bad_id_or_file_is_refused_and_nothing_is_written() {
    let store = MemStore::new();
    let web = WebSettings::new(store.clone());
    for path in ["/api/settings/devices/a.b", "/api/settings/devices/../settings"] {
        assert_eq!(call(&web, "PUT", path, "{}").await.0, 400, "{path}");
    }
    assert_eq!(call(&web, "PUT", "/api/settings/devices/ok", "[1]").await.0, 400);
    assert_eq!(call(&web, "PUT", "/api/settings/devices/ok", "not json").await.0, 400);
    let big = format!("{{\"x\":\"{}\"}}", "a".repeat(70 * 1024));
    assert_eq!(call(&web, "PUT", "/api/settings/devices/ok", &big).await.0, 413);
    assert_eq!(
        call(&web, "PUT", "/api/settings/devices/ok?from=a%2Fb", "{}").await.0,
        400
    );
    assert!(store.list("web/devices/").await.unwrap().is_empty());
}

#[test]
fn an_id_is_a_file_name() {
    assert_eq!(device_id_problem("laptop-2_b"), None);
    assert!(device_id_problem("").is_some());
    assert!(device_id_problem("my laptop").is_some());
    assert!(device_id_problem(&"a".repeat(65)).is_some());
}

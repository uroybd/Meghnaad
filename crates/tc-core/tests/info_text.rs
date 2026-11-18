//! `task info`'s text (`helpers::call("info_text", ..)`) against what the real `task` 3.5.0 printed for the same tasks,
//! with `rc.color=off rc.verbose=nothing`, on a scratch taskrc and data directory. The fixtures under
//! `fixtures/info_text/` are that output, unedited. Each task is given as the engine's `info` row writes it, with its
//! history, the open tasks around it, and the moment `task` ran (in a zone six hours east of UTC).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use serde_json::{json, Value};
use tc_core::helpers::call;
use tc_core::taskrc::parse;

/// The scratch taskrc the fixtures were made with.
const RC: &str = "confirmation=off
uda.estimate.type=duration
uda.estimate.label=Estimate
uda.review.type=date
uda.review.label=Reviewed
uda.size.type=string
urgency.user.tag.errand.coefficient=2.5
";

/// When the tasks were added, and the zone `task` ran in. (A task's urgency is the one `info` printed: an `export` a few
/// seconds later has the due date a little nearer.)
const ENTRY: i64 = 1_795_022_317;
const TZ: i64 = 21_600;

const MILK: &str = "8f3f4cbb-c9b2-4c8e-beca-9583d257635e";
const DEP_ONE: &str = "1e257d29-494c-4825-8ba2-b4c7064c3a98";
const DEP_TWO: &str = "09f1d061-64ee-4596-9d73-f4ccb44ff2e5";
const BLOCKED: &str = "186700c4-84b3-42da-b5a6-d289235ec49d";
const PARENT: &str = "84aeea39-1f76-474f-96b0-bfca23e42da6";
const MILK_TEXT: &str =
    "Buy milk and a long description that should wrap somewhere past the width of the value column for sure";

fn set(prop: &str, value: &str, date: bool) -> Value {
    json!({ "kind": "set", "prop": prop, "value": value, "date": date })
}

fn info(task: &Value, history: &Value, blocked_by: &Value, blocking: &Value, now: i64) -> String {
    let cfg = serde_json::to_value(parse(RC).config).unwrap();
    let args = json!([task, history, blocked_by, blocking, cfg, now, TZ]);
    let answer: Value = serde_json::from_str(&call("info_text", &args.to_string())).unwrap();
    answer
        .as_str()
        .unwrap_or_else(|| panic!("not text: {answer}"))
        .to_owned()
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/info_text/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn a_task_with_everything_reads_as_task_info_prints_it() {
    let task = json!({
        "uuid": MILK, "status": "pending", "description": MILK_TEXT, "project": "home", "priority": "H",
        "tags": ["errand", "shop"], "annotations": [{ "entry": ENTRY, "text": "first note here" }],
        "entry": ENTRY, "modified": ENTRY, "start": ENTRY, "due": 1_795_179_600, "wait": 1_795_024_800,
        "scheduled": 1_794_938_400, "depends": [DEP_TWO, DEP_ONE], "blocked": true, "blocking": true,
        "extra": { "estimate": "PT2H", "review": "1793469600", "size": "large" },
        "urgency": 28.1678, "id": 2,
        "virtual_tags": ["ACTIVE", "ANNOTATED", "BLOCKED", "BLOCKING", "DUE", "MONTH", "PRIORITY", "PROJECT",
            "QUARTER", "SCHEDULED", "TAGGED", "UDA", "WAITING", "WEEK", "YEAR"],
        "orphans": []
    });
    let history = json!([{ "at": ENTRY, "changes": [
        { "kind": "tag_added", "prop": "errand" },
        set("description", MILK_TEXT, false),
        set("due", "1795179600", true),
        set("entry", "1795022317", true),
        set("estimate", "PT2H", false),
        set("priority", "H", false),
        set("project", "home", false),
        set("review", "1793469600", true),
        set("scheduled", "1794938400", true),
        set("size", "large", false),
        { "kind": "tag_added", "prop": "shop" },
        set("wait", "1795024800", true),
        set("status", "pending", false),
        { "kind": "dep_added", "prop": DEP_TWO },
        { "kind": "dep_added", "prop": DEP_ONE },
        { "kind": "note_added", "prop": "annotation_1795022317", "value": "first note here" },
        set("start", "1795022317", true),
    ]}]);
    let text = info(
        &task,
        &history,
        &json!([{ "id": 1, "description": "Dep one" }]),
        &json!([{ "id": 3, "description": "Blocked on milk" }]),
        ENTRY + 6,
    );
    assert_eq!(text, fixture("task2.txt"));
}

#[test]
fn a_blocked_task_and_a_blocking_one_list_each_other() {
    let blocked = json!({
        "uuid": BLOCKED, "status": "pending", "description": "Blocked on milk", "entry": ENTRY, "modified": ENTRY,
        "depends": [MILK], "blocked": true, "urgency": -5.0, "id": 3, "virtual_tags": ["BLOCKED", "PENDING"]
    });
    let history = json!([{ "at": ENTRY, "changes": [
        { "kind": "dep_added", "prop": MILK },
        set("description", "Blocked on milk", false),
        set("entry", "1795022317", true),
        set("status", "pending", false),
    ]}]);
    let text = info(
        &blocked,
        &history,
        &json!([{ "id": 2, "description": MILK_TEXT }]),
        &json!([]),
        ENTRY,
    );
    assert_eq!(text, fixture("task3.txt"));

    let blocking = json!({
        "uuid": DEP_ONE, "status": "pending", "description": "Dep one", "project": "home.kitchen",
        "entry": ENTRY, "modified": ENTRY, "blocking": true, "urgency": 9.0, "id": 1,
        "virtual_tags": ["BLOCKING", "PENDING", "PROJECT", "READY", "UNBLOCKED"]
    });
    let history = json!([{ "at": ENTRY, "changes": [
        set("description", "Dep one", false),
        set("entry", "1795022317", true),
        set("project", "home.kitchen", false),
        set("status", "pending", false),
    ]}]);
    let text = info(
        &blocking,
        &history,
        &json!([]),
        &json!([{ "id": 2, "description": MILK_TEXT }]),
        ENTRY,
    );
    assert_eq!(text, fixture("task1.txt"));
}

#[test]
fn a_recurring_parent_and_its_instance_show_the_recurrence() {
    let parent = json!({
        "uuid": PARENT, "status": "recurring", "description": "Repeat me", "entry": ENTRY, "modified": ENTRY,
        "due": 1_795_543_200, "recur": "weekly", "rtype": "periodic", "mask": "-", "urgency": 6.0444, "id": 4,
        "virtual_tags": ["MONTH", "PARENT", "QUARTER", "TEMPLATE", "UNBLOCKED", "YEAR"]
    });
    let history = json!([{ "at": ENTRY, "changes": [
        set("description", "Repeat me", false),
        set("due", "1795543200", true),
        set("entry", "1795022317", true),
        set("recur", "weekly", false),
        set("rtype", "periodic", false),
        set("status", "recurring", false),
        set("mask", "-", false),
    ]}]);
    assert_eq!(
        info(&parent, &history, &json!([]), &json!([]), ENTRY + 6),
        fixture("task4.txt")
    );

    // The fixture is the first lines of what `task` printed, down to the urgency.
    let instance = json!({
        "uuid": "ad174c1f-b02f-4cc3-a319-cdf1ddfe0b1e", "status": "pending", "description": "Repeat me",
        "entry": ENTRY, "modified": ENTRY, "due": 1_795_543_200, "recur": "weekly", "rtype": "periodic",
        "parent": PARENT, "imask": 0, "urgency": 6.0444, "id": 5,
        "virtual_tags": ["CHILD", "INSTANCE", "LATEST", "MONTH", "PENDING", "QUARTER", "READY", "UNBLOCKED", "YEAR"]
    });
    let text = info(&instance, &json!(null), &json!([]), &json!([]), ENTRY + 26);
    assert!(text.starts_with(&fixture("task5.txt")), "{text}");
}

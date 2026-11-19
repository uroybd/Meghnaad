//! What `tc_core::forge` asks of GitHub and of Gitea/Forgejo, and how it reads the answers, against a recording `Http`:
//! one request per commit, the conflict checks each forge makes, and the token only ever in a header.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;
use tc_core::forge::{Forge, ForgeRepo, GitRemote, Http, HttpRequest, HttpResponse};
use tc_core::gitsync::{Change, Committed, GitRepo};

const TOKEN: &str = "ghp_SECRET_TOKEN_123";

/// Answers each request with the next of `answers`, and keeps what was asked.
#[derive(Clone, Default)]
struct Recorder {
    asked: Rc<RefCell<Vec<HttpRequest>>>,
    answers: Rc<RefCell<Vec<(u16, Value)>>>,
}

impl Recorder {
    fn answering(answers: Vec<(u16, Value)>) -> Self {
        let r = Recorder::default();
        *r.answers.borrow_mut() = answers;
        r
    }

    fn body(&self, i: usize) -> Value {
        serde_json::from_slice(self.asked.borrow()[i].body.as_deref().unwrap()).unwrap()
    }
}

impl Http for Recorder {
    async fn send(&self, request: HttpRequest) -> tc_core::Result<HttpResponse> {
        self.asked.borrow_mut().push(request);
        let (status, body) = self.answers.borrow_mut().remove(0);
        Ok(HttpResponse {
            status,
            body: serde_json::to_vec(&body).unwrap(),
        })
    }
}

fn github(r: &Recorder) -> ForgeRepo<Recorder> {
    ForgeRepo::new(r.clone(), GitRemote::new("", "alice/tasks", "", TOKEN).unwrap())
}

fn gitea(r: &Recorder) -> ForgeRepo<Recorder> {
    ForgeRepo::new(
        r.clone(),
        GitRemote::new("https://git.example.com/api/v1/", "alice/tasks", "work", TOKEN).unwrap(),
    )
}

fn changes() -> Vec<Change> {
    vec![
        Change::Create {
            path: "v-a-b".into(),
            bytes: b"sealed".to_vec(),
        },
        Change::Update {
            path: "meta".into(),
            bytes: b"{}".to_vec(),
            was: "blob1".into(),
        },
        Change::Delete {
            path: "v-old".into(),
            was: "blob2".into(),
        },
    ]
}

/// No request carries the token anywhere but its `authorization` header.
fn token_only_in_headers(r: &Recorder) {
    for req in r.asked.borrow().iter() {
        assert!(!req.url.contains(TOKEN), "{}", req.url);
        let body = String::from_utf8_lossy(req.body.as_deref().unwrap_or_default()).into_owned();
        assert!(!body.contains(TOKEN), "{body}");
        let carrying: Vec<&str> = req
            .headers
            .iter()
            .filter(|(_, v)| v.contains(TOKEN))
            .map(|(k, _)| *k)
            .collect();
        assert_eq!(carrying, ["authorization"]);
    }
}

#[test]
fn remotes_are_checked_and_defaulted() {
    let r = GitRemote::new(" ", "alice/tasks", "", "t").unwrap();
    assert_eq!(
        (r.api.as_str(), r.branch.as_str(), r.forge()),
        ("https://api.github.com", "taskchampion", Forge::GitHub)
    );
    assert_eq!(
        GitRemote::new("https://git.example.com/api/v1", "a/b", "x", "t")
            .unwrap()
            .forge(),
        Forge::Gitea
    );
    // The token goes with every request: only over TLS, but to this machine for testing.
    assert!(GitRemote::new("http://git.example.com/api/v1", "a/b", "", "t").is_err());
    assert!(GitRemote::new("http://127.0.0.1:3000/api/v1", "a/b", "", "t").is_ok());
    for repo in ["tasks", "a/b/c", "../x", "a/"] {
        assert!(GitRemote::new("", repo, "", "t").is_err(), "{repo}");
    }
    for branch in ["a..b", "/x", "x/", "-x", "x y"] {
        assert!(GitRemote::new("", "a/b", branch, "t").is_err(), "{branch}");
    }
    assert!(GitRemote::new("", "a/b", "", " ").is_err());
    let e = GitRemote::new("ftp://x", "a/b", "", TOKEN).err().unwrap().to_string();
    assert!(!e.contains(TOKEN));
}

#[tokio::test]
async fn a_github_commit_is_one_mutation_on_the_head_it_was_worked_out_from() {
    let r = Recorder::answering(vec![(
        200,
        json!({ "data": { "createCommitOnBranch": { "commit": { "oid": "new-head" } } } }),
    )]);
    let done = github(&r)
        .commit(Some("old-head"), "add version", &changes())
        .await
        .unwrap();
    assert_eq!(done, Committed::Done("new-head".into()));
    let asked = r.asked.borrow()[0].clone();
    assert_eq!(
        (asked.method, asked.url.as_str()),
        ("POST", "https://api.github.com/graphql")
    );
    let input = &r.body(0)["variables"]["input"];
    assert_eq!(input["expectedHeadOid"], "old-head");
    assert_eq!(
        input["branch"],
        json!({ "repositoryNameWithOwner": "alice/tasks", "branchName": "taskchampion" })
    );
    assert_eq!(input["message"]["headline"], "add version");
    assert_eq!(
        input["fileChanges"]["additions"],
        json!([{ "path": "v-a-b", "contents": "c2VhbGVk" }, { "path": "meta", "contents": "e30=" }])
    );
    assert_eq!(input["fileChanges"]["deletions"], json!([{ "path": "v-old" }]));
    token_only_in_headers(&r);
}

#[tokio::test]
async fn a_github_branch_that_moved_is_moved() {
    for error in [
        json!({ "type": "STALE_DATA", "message": "Expected branch to point to \"x\" but it did not. Pull and try again." }),
        json!({ "message": "Expected branch to point to \"x\" but it did not." }),
    ] {
        let r = Recorder::answering(vec![(200, json!({ "data": null, "errors": [error] }))]);
        assert_eq!(
            github(&r).commit(Some("h"), "m", &changes()).await.unwrap(),
            Committed::Moved
        );
    }
    // Any other refusal is an error, and says why.
    let r = Recorder::answering(vec![(
        200,
        json!({ "errors": [{ "message": "Resource not accessible" }] }),
    )]);
    let e = github(&r)
        .commit(Some("h"), "m", &changes())
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("Resource not accessible"), "{e}");
}

#[tokio::test]
async fn github_makes_a_missing_branch_from_the_default_one() {
    let r = Recorder::answering(vec![
        (
            200,
            json!({ "data": { "repository": { "id": "R1", "defaultBranchRef": { "target": { "oid": "main-head" } }, "ref": null } } }),
        ),
        (
            200,
            json!({ "data": { "createRef": { "ref": { "name": "refs/heads/taskchampion" } } } }),
        ),
    ]);
    assert_eq!(
        github(&r).commit(None, "init", &changes()).await.unwrap(),
        Committed::Moved
    );
    let input = &r.body(1)["variables"]["input"];
    assert_eq!(
        input,
        &json!({ "repositoryId": "R1", "name": "refs/heads/taskchampion", "oid": "main-head" })
    );
    // An empty repository has nothing to branch from: it says what to do.
    let r = Recorder::answering(vec![(
        200,
        json!({ "data": { "repository": { "id": "R1", "defaultBranchRef": null, "ref": null } } }),
    )]);
    let e = github(&r)
        .commit(None, "init", &changes())
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("no commits yet"), "{e}");
}

#[tokio::test]
async fn github_reads_the_head_and_a_file_together_and_fetches_binary_ones() {
    let r = Recorder::answering(vec![(
        200,
        json!({ "data": { "repository": { "ref": { "target": { "oid": "h1", "file": {
            "oid": "b1", "type": "blob", "object": { "text": "{\"a\":1}", "isBinary": false, "isTruncated": false } } } } } } }),
    )]);
    let (head, blob) = github(&r).read("meta").await.unwrap();
    assert_eq!(head.as_deref(), Some("h1"));
    assert_eq!(blob.unwrap().bytes, b"{\"a\":1}");
    assert_eq!(r.body(0)["variables"]["path"], "meta");

    let r = Recorder::answering(vec![
        (
            200,
            json!({ "data": { "repository": { "ref": { "target": { "oid": "h1", "file": {
                "oid": "b2", "type": "blob", "object": { "text": null, "isBinary": true, "isTruncated": false } } } } } } }),
        ),
        (200, json!({ "content": "AAEC\n/w==\n", "encoding": "base64" })),
    ]);
    let (_, blob) = github(&r).read("web/hook-secrets.enc").await.unwrap();
    assert_eq!(blob.unwrap().bytes, [0, 1, 2, 255]);
    assert_eq!(
        r.asked.borrow()[1].url,
        "https://api.github.com/repos/alice/tasks/git/blobs/b2"
    );

    // No such branch, and no such file.
    let r = Recorder::answering(vec![(200, json!({ "data": { "repository": { "ref": null } } }))]);
    assert_eq!(github(&r).read("meta").await.unwrap(), (None, None));
    let r = Recorder::answering(vec![(
        200,
        json!({ "data": { "repository": { "ref": { "target": { "oid": "h1", "file": null } } } } }),
    )]);
    assert_eq!(github(&r).read("meta").await.unwrap(), (Some("h1".into()), None));
    token_only_in_headers(&r);

    // What GitHub really answers for a file that isn't there: the data, and an error about that one field.
    let r = Recorder::answering(vec![(
        200,
        json!({
            "data": { "repository": { "ref": { "target": { "oid": "h1", "file": null } } } },
            "errors": [{ "type": "NOT_FOUND", "path": ["repository", "ref", "target", "file"],
                "locations": [{ "line": 1, "column": 120 }], "message": "Could not resolve file for path 'snapshot'." }]
        }),
    )]);
    assert_eq!(github(&r).read("snapshot").await.unwrap(), (Some("h1".into()), None));

    // Any other error is still one.
    let r = Recorder::answering(vec![(
        200,
        json!({ "data": null, "errors": [{ "type": "NOT_FOUND", "path": ["repository"],
            "message": "Could not resolve to a Repository with the name 'alice/tasks'." }] }),
    )]);
    let err = github(&r).read("meta").await.unwrap_err().to_string();
    assert!(err.contains("Could not resolve to a Repository"), "{err}");
}

#[tokio::test]
async fn github_dates_many_files_in_one_query() {
    let r = Recorder::answering(vec![(
        200,
        json!({ "data": { "repository": { "ref": { "target": {
            "h0": { "nodes": [{ "committedDate": "2026-01-02T03:04:05Z" }] },
            "h1": { "nodes": [] } } } } } }),
    )]);
    let dates = github(&r)
        .last_changed(&["v-a-b".to_owned(), "v-\"odd\"".to_owned()])
        .await
        .unwrap();
    assert_eq!(dates, [Some(1_767_323_045), None]);
    assert_eq!(r.asked.borrow().len(), 1);
    let query = r.body(0)["query"].as_str().unwrap().to_owned();
    assert!(query.contains(r#"h1:history(path:"v-\"odd\"",first:1)"#), "{query}");
}

#[tokio::test]
async fn a_gitea_commit_is_one_request_that_checks_each_file() {
    let r = Recorder::answering(vec![(201, json!({ "commit": { "sha": "c9" } }))]);
    assert_eq!(
        gitea(&r).commit(Some(""), "add version", &changes()).await.unwrap(),
        Committed::Done("c9".into())
    );
    let asked = r.asked.borrow()[0].clone();
    assert_eq!(asked.url, "https://git.example.com/api/v1/repos/alice/tasks/contents");
    let body = r.body(0);
    assert_eq!(body["branch"], "work");
    assert_eq!(
        body["author"],
        json!({ "name": "taskchampion", "email": "taskchampion@local" })
    );
    assert_eq!(
        body["files"],
        json!([
            { "operation": "create", "path": "v-a-b", "content": "c2VhbGVk" },
            { "operation": "update", "path": "meta", "content": "e30=", "sha": "blob1" },
            { "operation": "delete", "path": "v-old", "sha": "blob2" },
        ])
    );
    token_only_in_headers(&r);
}

#[tokio::test]
async fn gitea_conflicts_are_moved_and_a_missing_branch_is_made() {
    for (status, message) in [
        (422, "sha does not match [given: x, expected: y]"),
        (422, "repository file already exists [path: meta]"),
        (500, "repository file does not exist [path: v-old]"),
    ] {
        let r = Recorder::answering(vec![(status, json!({ "message": message }))]);
        assert_eq!(
            gitea(&r).commit(Some(""), "m", &changes()).await.unwrap(),
            Committed::Moved,
            "{message}"
        );
    }
    let r = Recorder::answering(vec![
        (404, json!({ "message": "branch does not exist [name: work]" })),
        (201, json!({ "commit": { "sha": "c1" } })),
    ]);
    assert_eq!(
        gitea(&r).commit(Some(""), "init", &changes()).await.unwrap(),
        Committed::Done("c1".into())
    );
    assert_eq!(r.body(1)["new_branch"], "work");
    assert!(r.body(1).get("branch").is_none());
    // A repository that is not there (or not visible) is an error, not a race to retry.
    let r = Recorder::answering(vec![(404, json!({ "message": "repo does not exist" }))]);
    assert!(gitea(&r).commit(Some(""), "m", &changes()).await.is_err());
}

#[tokio::test]
async fn gitea_lists_the_root_by_the_heads_commit() {
    let r = Recorder::answering(vec![
        (200, json!({ "name": "work", "commit": { "id": "c7" } })),
        (
            200,
            json!({ "sha": "c7", "truncated": false, "tree": [
                { "path": "meta", "type": "blob", "sha": "b1" },
                { "path": "web", "type": "tree", "sha": "t1" } ] }),
        ),
    ]);
    let (head, entries) = gitea(&r).root().await.unwrap();
    assert_eq!(head.as_deref(), Some("c7"));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "meta");
    assert_eq!(
        r.asked.borrow()[0].url,
        "https://git.example.com/api/v1/repos/alice/tasks/branches/work"
    );
    assert!(r.asked.borrow()[1].url.contains("/git/trees/c7?"));
}

#[tokio::test]
async fn refusals_say_why_and_never_carry_the_token() {
    let forges: [fn(&Recorder) -> ForgeRepo<Recorder>; 2] = [github, gitea];
    for repo in forges {
        let r = Recorder::answering(vec![(401, json!({ "message": "Bad credentials" }))]);
        let e = repo(&r).blob("b1").await.unwrap_err().to_string();
        assert!(e.contains("401") && e.contains("token was refused"), "{e}");
        assert!(!e.contains(TOKEN), "{e}");
        token_only_in_headers(&r);
    }
}

#[test]
fn github_enterprise_has_its_graphql_beside_its_rest() {
    let r = Recorder::answering(vec![(200, json!({ "data": { "repository": { "ref": null } } }))]);
    let repo = ForgeRepo::new(
        r.clone(),
        GitRemote::new("https://ghe.example.com/api/v3", "a/b", "", "t").unwrap(),
    );
    assert_eq!(futures_executor(repo.read("meta")).unwrap(), (None, None));
    assert_eq!(r.asked.borrow()[0].url, "https://ghe.example.com/api/graphql");
}

/// Run a future to completion on this thread (the recorder never waits).
fn futures_executor<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(f)
}

#[tokio::test]
async fn github_reads_a_folder_two_levels_deep_in_one_query_and_deeper_ones_after() {
    let blob = |text: Option<&str>| match text {
        Some(t) => json!({ "text": t, "isBinary": false, "isTruncated": false }),
        None => json!({ "text": null, "isBinary": true, "isTruncated": false }),
    };
    let first = json!({ "data": { "repository": { "ref": { "target": { "file": { "type": "tree", "object": { "entries": [
        { "name": "taskrc", "type": "blob", "oid": "a1", "object": blob(Some("include themes/dark.rc\n")) },
        { "name": "icon.png", "type": "blob", "oid": "b2", "object": blob(None) },
        { "name": "themes", "type": "tree", "oid": "c3", "object": { "entries": [
            { "name": "dark.rc", "type": "blob", "oid": "d4", "object": blob(Some("color.due=red\n")) },
            { "name": "more", "type": "tree", "oid": "e5", "object": { "entries": [
                { "name": "deep", "type": "tree", "oid": "f6", "object": {} }
            ] } }
        ] } }
    ] } } } } } } });
    let second = json!({ "data": { "repository": { "ref": { "name": "main" }, "t0": { "entries": [
        { "name": "x.rc", "type": "blob", "oid": "a7", "object": blob(Some("bulk=2\n")) }
    ] } } } });
    let png = json!({ "content": "aGk=" });
    let r = Recorder::answering(vec![(200, first), (200, second), (200, png)]);
    let mut files = github(&r).read_dir("web/rc").await.unwrap().unwrap();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let got: Vec<(&str, &str, &[u8])> = files
        .iter()
        .map(|(p, b)| (p.as_str(), b.id.as_str(), b.bytes.as_slice()))
        .collect();
    assert_eq!(
        got,
        [
            ("web/rc/icon.png", "b2", b"hi".as_slice()),
            ("web/rc/taskrc", "a1", b"include themes/dark.rc\n".as_slice()),
            ("web/rc/themes/dark.rc", "d4", b"color.due=red\n".as_slice()),
            ("web/rc/themes/more/deep/x.rc", "a7", b"bulk=2\n".as_slice()),
        ]
    );
    // Three requests: the folder and two levels, the folder too deep for it, and the binary file by its blob.
    assert_eq!(r.asked.borrow().len(), 3);
    assert_eq!(r.body(0)["variables"]["path"], "web/rc");
    assert!(r.body(1)["query"].as_str().unwrap().contains("t0:object(oid:\"f6\")"));
    assert!(r.asked.borrow()[2].url.ends_with("git/blobs/b2"));
}

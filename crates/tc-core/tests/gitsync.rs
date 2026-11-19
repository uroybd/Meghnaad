//! The git sync server (`tc_core::gitsync`): TaskChampion's own `gitsync` tests, ported to an in-memory branch, and a
//! branch written by the real `task` 3.5.0 (`tests/fixtures/gitsync-cli`, made with `sync.git.remote` and the secret
//! `s3cret`) read back.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]

use taskchampion::server::{AddVersionResult, GetVersionResult, SnapshotUrgency};
use taskchampion::{Operations, Replica, Server, Status};
use tc_core::crypto::Cryptor;
use tc_core::gitsync::{load_salt, Change, Committed, GitObjects, GitRepo, GitServer, MemRepo, Meta, META, SNAPSHOT};
use tc_core::{names, LiveStorage, ObjectStore};
use uuid::Uuid;

const NIL: Uuid = Uuid::nil();
const DAY: i64 = 24 * 3600;

/// A fixed key, so the tests do not spend PBKDF2's 600,000 rounds each time.
fn cryptor() -> Cryptor {
    Cryptor::from_key(&[7; 32])
}

async fn server(repo: &MemRepo) -> GitServer<MemRepo> {
    load_salt(repo).await.unwrap();
    GitServer::with_cryptor(repo.clone(), cryptor())
}

fn meta(repo: &MemRepo) -> Meta {
    Meta::parse(&repo.file(META).unwrap()).unwrap()
}

async fn add(s: &mut GitServer<MemRepo>, parent: Uuid, body: &[u8]) -> (Uuid, SnapshotUrgency) {
    match s.add_version(parent, body.to_vec()).await.unwrap() {
        (AddVersionResult::Ok(v), u) => (v, u),
        (AddVersionResult::ExpectedParentVersion(e), _) => panic!("expected parent {e}"),
    }
}

#[tokio::test]
async fn init_writes_meta_once() {
    let repo = MemRepo::new();
    let salt = load_salt(&repo).await.unwrap();
    assert_eq!(salt.len(), 16);
    assert_eq!(meta(&repo).latest_version, NIL);
    assert_eq!(repo.messages(), ["init taskchampion repo"]);
    // A second replica finds it and takes the same salt.
    assert_eq!(load_salt(&repo).await.unwrap(), salt);
    assert_eq!(repo.messages().len(), 1);
    // The file is the CLI's: simple uuid, base64 salt.
    let raw: serde_json::Value = serde_json::from_slice(&repo.file(META).unwrap()).unwrap();
    assert_eq!(raw["latest_version"], "00000000000000000000000000000000");
}

#[tokio::test]
async fn meta_round_trips() {
    let m = Meta {
        latest_version: Uuid::new_v4(),
        salt: vec![1, 2, 3],
    };
    assert_eq!(Meta::parse(&m.to_bytes().unwrap()).unwrap(), m);
}

#[tokio::test]
async fn add_from_nil_base() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    let (v, _) = add(&mut s, NIL, b"1234").await;
    assert!(repo.paths().contains(&names::version_name(NIL, v)));
    assert_eq!(meta(&repo).latest_version, v);
    // One commit holds the version and `meta`.
    assert_eq!(repo.messages().last().unwrap(), "add version");
}

#[tokio::test]
async fn any_parent_while_empty_then_only_the_latest() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    let parent = Uuid::new_v4();
    let (v, _) = add(&mut s, parent, b"1234").await;
    match s.add_version(parent, b"1234".to_vec()).await.unwrap().0 {
        AddVersionResult::ExpectedParentVersion(e) => assert_eq!(e, v),
        AddVersionResult::Ok(_) => panic!("a stale parent was taken"),
    }
}

#[tokio::test]
async fn second_writer_is_told_the_latest() {
    let repo = MemRepo::new();
    let mut s1 = server(&repo).await;
    let mut s2 = server(&repo).await;
    let (v1, _) = add(&mut s1, NIL, b"v1").await;
    match s2.add_version(NIL, b"v2".to_vec()).await.unwrap().0 {
        AddVersionResult::ExpectedParentVersion(e) => assert_eq!(e, v1),
        AddVersionResult::Ok(_) => panic!("both were taken"),
    }
}

#[tokio::test]
async fn a_moved_branch_is_tried_again_when_only_settings_changed() {
    // GitHub refuses a commit whenever the branch moved, even for a settings file: the version still goes in.
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    repo.lose_next(2);
    let (v, _) = add(&mut s, NIL, b"v1").await;
    assert_eq!(meta(&repo).latest_version, v);
    // But not forever.
    repo.lose_next(100);
    assert!(s.add_version(v, b"v2".to_vec()).await.is_err());
}

#[tokio::test]
async fn file_checking_forges_refuse_a_stale_meta() {
    // Gitea and Forgejo check `meta`'s blob, not the head.
    let repo = MemRepo::checking_files();
    let mut s = server(&repo).await;
    let (v1, _) = add(&mut s, NIL, b"v1").await;
    let (v2, _) = add(&mut s, v1, b"v2").await;
    let mut other = server(&repo).await;
    match other.add_version(v1, b"x".to_vec()).await.unwrap().0 {
        AddVersionResult::ExpectedParentVersion(e) => assert_eq!(e, v2),
        AddVersionResult::Ok(_) => panic!("a stale parent was taken"),
    }
}

#[tokio::test]
async fn get_child_version() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    let parent = Uuid::new_v4();
    assert_eq!(
        s.get_child_version(parent).await.unwrap(),
        GetVersionResult::NoSuchVersion
    );
    let (v, _) = add(&mut s, parent, b"1234").await;
    let mut other = server(&repo).await;
    assert_eq!(
        other.get_child_version(parent).await.unwrap(),
        GetVersionResult::Version {
            version_id: v,
            parent_version_id: parent,
            history_segment: b"1234".to_vec()
        }
    );
    assert_eq!(
        other.get_child_version(v).await.unwrap(),
        GetVersionResult::NoSuchVersion
    );
}

#[tokio::test]
async fn a_corrupt_version_is_an_error() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    let (v, _) = add(&mut s, NIL, b"good").await;
    repo.insert(&names::version_name(NIL, v), b"garbage");
    assert!(server(&repo).await.get_child_version(NIL).await.is_err());
}

#[tokio::test]
async fn snapshots() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    assert_eq!(s.get_snapshot().await.unwrap(), None);
    let v1 = Uuid::new_v4();
    s.add_snapshot(v1, b"first".to_vec()).await.unwrap();
    assert_eq!(s.get_snapshot().await.unwrap(), Some((v1, b"first".to_vec())));
    // The next overwrites it.
    let v2 = Uuid::new_v4();
    s.add_snapshot(v2, b"second".to_vec()).await.unwrap();
    let mut other = server(&repo).await;
    assert_eq!(other.get_snapshot().await.unwrap(), Some((v2, b"second".to_vec())));
    // The file is the CLI's: `{"version_id": simple, "payload": base64}`.
    let raw: serde_json::Value = serde_json::from_slice(&repo.file(SNAPSHOT).unwrap()).unwrap();
    assert_eq!(raw["version_id"], v2.as_simple().to_string());
    assert!(raw["payload"].is_string());
}

#[tokio::test]
async fn a_snapshot_that_cannot_be_stored_does_not_fail_the_sync() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    repo.lose_next(100);
    s.add_snapshot(Uuid::new_v4(), b"snap".to_vec()).await.unwrap();
    repo.lose_next(0);
    assert_eq!(s.get_snapshot().await.unwrap(), None);
}

/// Three versions a long time ago, then a snapshot of the last now.
async fn old_chain(repo: &MemRepo, s: &mut GitServer<MemRepo>) -> [Uuid; 3] {
    repo.set_now(1_000 * DAY);
    let (v1, _) = add(s, NIL, b"v1").await;
    let (v2, _) = add(s, v1, b"v2").await;
    let (v3, _) = add(s, v2, b"v3").await;
    [v1, v2, v3]
}

#[tokio::test]
async fn cleanup_after_a_snapshot_removes_old_covered_versions_in_one_commit() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await.with_cleanup(|| 1_200 * DAY);
    let [v1, v2, v3] = old_chain(&repo, &mut s).await;
    let commits = repo.messages().len();
    s.add_snapshot(v3, b"full state".to_vec()).await.unwrap();
    for (p, c) in [(NIL, v1), (v1, v2), (v2, v3)] {
        assert!(!repo.paths().contains(&names::version_name(p, c)));
    }
    assert!(repo.paths().contains(&SNAPSHOT.to_owned()));
    assert_eq!(
        &repo.messages()[commits..],
        ["add snapshot", "cleanup: remove version files covered by snapshot"]
    );
}

#[tokio::test]
async fn cleanup_keeps_recent_versions() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await.with_cleanup(|| 1_100 * DAY);
    let [_, _, v3] = old_chain(&repo, &mut s).await;
    s.add_snapshot(v3, b"full state".to_vec()).await.unwrap();
    assert_eq!(repo.paths().iter().filter(|p| p.starts_with("v-")).count(), 3);
}

#[tokio::test]
async fn cleanup_keeps_versions_after_the_snapshot() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await.with_cleanup(|| 1_200 * DAY);
    let [v1, v2, v3] = old_chain(&repo, &mut s).await;
    s.add_snapshot(v2, b"state at v2".to_vec()).await.unwrap();
    let left: Vec<String> = repo.paths().into_iter().filter(|p| p.starts_with("v-")).collect();
    assert_eq!(left, [names::version_name(v2, v3)]);
    // A replica catching up from the snapshot still finds the rest.
    let mut other = server(&repo).await;
    assert_eq!(other.get_snapshot().await.unwrap().unwrap().0, v2);
    assert!(matches!(
        other.get_child_version(v2).await.unwrap(),
        GetVersionResult::Version { .. }
    ));
    let _ = v1;
}

#[tokio::test]
async fn urgency_counts_versions_since_the_snapshot() {
    let repo = MemRepo::new();
    let mut s = server(&repo).await;
    let mut at = NIL;
    let mut urgencies = Vec::new();
    for i in 0..101 {
        let (v, u) = add(&mut s, at, format!("v{i}").as_bytes()).await;
        urgencies.push(u);
        at = v;
    }
    assert_eq!(urgencies[49], SnapshotUrgency::None);
    assert_eq!(urgencies[50], SnapshotUrgency::Low);
    assert_eq!(urgencies[99], SnapshotUrgency::Low);
    assert_eq!(urgencies[100], SnapshotUrgency::High);
    // After a snapshot the count starts again.
    s.add_snapshot(at, b"state".to_vec()).await.unwrap();
    assert_eq!(add(&mut s, at, b"next").await.1, SnapshotUrgency::None);
}

#[tokio::test]
async fn settings_files_share_the_branch() {
    let repo = MemRepo::checking_files();
    let mut s = server(&repo).await;
    let files = GitObjects(repo.clone());
    files.put("web/config.json", b"{}").await.unwrap();
    files.put("web/rc/themes/dark.theme", b"color=on").await.unwrap();
    files.put("web/rc/taskrc", b"x=1").await.unwrap();
    assert_eq!(files.get("web/config.json").await.unwrap().unwrap(), b"{}");
    let mut listed = files.list("web/rc/").await.unwrap();
    listed.sort();
    assert_eq!(listed, ["web/rc/taskrc", "web/rc/themes/dark.theme"]);
    // The whole folder at once, by the files' blob ids.
    let mut all = files.get_all("web/rc/").await.unwrap();
    all.sort();
    assert_eq!(
        all,
        [
            ("web/rc/taskrc".to_owned(), b"x=1".to_vec()),
            ("web/rc/themes/dark.theme".to_owned(), b"color=on".to_vec())
        ]
    );
    // An unchanged put makes no commit.
    let commits = repo.messages().len();
    files.put("web/config.json", b"{}").await.unwrap();
    assert_eq!(repo.messages().len(), commits);
    files.del("web/rc/taskrc").await.unwrap();
    files.del("web/rc/taskrc").await.unwrap();
    assert_eq!(files.get("web/rc/taskrc").await.unwrap(), None);
    // Conditional writes, as on a bucket.
    assert!(!files
        .compare_and_swap("web/config.json", Some(b"[]"), b"1")
        .await
        .unwrap());
    assert!(files
        .compare_and_swap("web/config.json", Some(b"{}"), b"1")
        .await
        .unwrap());
    let (_, tag) = files.get_tagged("web/config.json").await.unwrap().unwrap();
    assert!(files.swap_tagged("web/config.json", Some(&tag), b"2").await.unwrap());
    assert!(!files.swap_tagged("web/config.json", Some(&tag), b"3").await.unwrap());
    // Tasks are unaffected.
    add(&mut s, NIL, b"v1").await;
}

#[tokio::test]
async fn a_settings_file_is_dated_by_its_last_commit() {
    // A phone keeps the newer of two copies changed on both sides, so the date is the commit's, not 0.
    let repo = MemRepo::new();
    let files = GitObjects(repo.clone());
    repo.set_now(1_000);
    files.put("web/settings.json", b"{}").await.unwrap();
    files.put("web/rc/taskrc", b"x=1").await.unwrap();
    repo.set_now(2_000);
    files.put("web/settings.json", b"{\"a\":1}").await.unwrap();
    let mut dated = files.list_dated("web/").await.unwrap();
    dated.sort();
    // Everything under the prefix, as a bucket lists it.
    assert_eq!(
        dated,
        [
            ("web/rc/taskrc".to_owned(), 1_000),
            ("web/settings.json".to_owned(), 2_000)
        ]
    );
    assert_eq!(
        files.list_dated("web/rc/").await.unwrap(),
        [("web/rc/taskrc".to_owned(), 1_000)]
    );
    assert_eq!(files.list_dated("web/nothing").await.unwrap(), []);
}

#[tokio::test]
async fn first_commit_on_a_new_branch() {
    // A forge that has the repository but not the branch: the first commit makes it and says `Moved`.
    struct NoBranchYet(MemRepo, std::cell::Cell<bool>);
    impl GitRepo for NoBranchYet {
        async fn read(&self, path: &str) -> tc_core::Result<(Option<String>, Option<tc_core::gitsync::Blob>)> {
            if self.1.get() {
                Ok(self.0.read(path).await?)
            } else {
                Ok((None, None))
            }
        }
        async fn root(&self) -> tc_core::Result<(Option<String>, Vec<tc_core::gitsync::Entry>)> {
            self.0.root().await
        }
        async fn tree(&self, dir: &str) -> tc_core::Result<Vec<tc_core::gitsync::Entry>> {
            self.0.tree(dir).await
        }
        async fn blob(&self, id: &str) -> tc_core::Result<Vec<u8>> {
            self.0.blob(id).await
        }
        async fn commit(&self, head: Option<&str>, m: &str, c: &[Change]) -> tc_core::Result<Committed> {
            if head.is_none() && !self.1.get() {
                self.1.set(true);
                return Ok(Committed::Moved);
            }
            self.0.commit(head, m, c).await
        }
        async fn last_changed(&self, paths: &[String]) -> tc_core::Result<Vec<Option<i64>>> {
            self.0.last_changed(paths).await
        }
    }
    let inner = MemRepo::new();
    let repo = NoBranchYet(inner.clone(), std::cell::Cell::new(false));
    assert_eq!(load_salt(&repo).await.unwrap().len(), 16);
    assert_eq!(inner.messages(), ["init taskchampion repo"]);
}

fn cli_branch() -> MemRepo {
    let repo = MemRepo::new();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/gitsync-cli");
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        repo.insert(
            entry.file_name().to_str().unwrap(),
            &std::fs::read(entry.path()).unwrap(),
        );
    }
    repo
}

#[tokio::test]
async fn reads_a_branch_the_cli_wrote() {
    let repo = cli_branch();
    let mut s = GitServer::new(repo.clone(), b"s3cret").await.unwrap();
    let key = tc_core::gitsync::load_cryptor(&repo, b"s3cret").await.unwrap();
    let (version, snapshot) = s.get_snapshot().await.unwrap().unwrap();
    assert_eq!(version.as_simple().to_string(), "d4aa23830e1f4a63b7e1bedd7d9227a0");
    assert!(!snapshot.is_empty());
    // The versions after it, up to `meta`'s latest.
    let mut at = version;
    let mut seen = 0;
    while let GetVersionResult::Version { version_id, .. } = s.get_child_version(at).await.unwrap() {
        at = version_id;
        seen += 1;
    }
    assert_eq!(seen, 5);
    assert_eq!(at, meta(&repo).latest_version);

    // A replica built from it has every task the CLI added (two, 105 numbered ones, and one from a second replica).
    let mut replica = Replica::new(LiveStorage::new());
    let mut s: Box<dyn Server> = Box::new(GitServer::with_cryptor(repo.clone(), key));
    replica.sync(&mut s, false).await.unwrap();
    let pending = replica
        .all_tasks()
        .await
        .unwrap()
        .values()
        .filter(|t| t.get_status() == Status::Pending)
        .count();
    assert_eq!(pending, 108);

    // And what it adds lands where the CLI looks.
    let mut ops = Operations::new();
    let mut t = replica.create_task(Uuid::new_v4(), &mut ops).await.unwrap();
    t.set_description("from the web".into(), &mut ops).unwrap();
    replica.commit_operations(ops).await.unwrap();
    replica.sync(&mut s, false).await.unwrap();
    let latest = meta(&repo).latest_version;
    assert!(repo.paths().contains(&names::version_name(at, latest)));
}

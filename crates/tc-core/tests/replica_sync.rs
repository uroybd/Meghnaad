//! Two independent TaskChampion replicas syncing through our `CloudServer` over one shared
//! in-memory bucket.

use taskchampion::storage::inmemory::InMemoryStorage;
use taskchampion::{Operations, Replica, Server, Status};
use tc_core::{names, CloudServer, MemStore, ObjectStore};

type R = Replica<InMemoryStorage>;

async fn server(store: &MemStore) -> Box<dyn Server> {
    Box::new(CloudServer::new(store.clone(), b"hunter2").await.unwrap())
}

fn replica() -> R {
    Replica::new(InMemoryStorage::new())
}

async fn add_task(r: &mut R, description: &str) -> uuid::Uuid {
    let mut ops = Operations::new();
    let uuid = uuid::Uuid::new_v4();
    let mut t = r.create_task(uuid, &mut ops).await.unwrap();
    t.set_status(Status::Pending, &mut ops).unwrap();
    t.set_description(description.into(), &mut ops).unwrap();
    r.commit_operations(ops).await.unwrap();
    uuid
}

#[tokio::test]
async fn tasks_propagate_between_replicas() {
    let store = MemStore::new();
    let (mut a, mut b) = (replica(), replica());

    let id = add_task(&mut a, "buy milk").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut server(&store).await, true).await.unwrap();

    let t = b.get_task(id).await.unwrap().expect("task synced to b");
    assert_eq!(t.get_description(), "buy milk");

    // The bucket now has the layout the `task` CLI expects.
    let names = store.names();
    assert!(names.contains(&"salt".to_string()));
    assert!(names.contains(&"latest".to_string()));
    assert_eq!(
        names.iter().filter(|n| names::parse_version_name(n).is_some()).count(),
        1
    );
}

#[tokio::test]
async fn concurrent_edits_converge() {
    let store = MemStore::new();
    let (mut a, mut b) = (replica(), replica());
    let id = add_task(&mut a, "shared").await;
    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut server(&store).await, true).await.unwrap();

    // Divergent offline edits on both replicas.
    let mut ops = Operations::new();
    let mut t = a.get_task(id).await.unwrap().unwrap();
    t.set_description("from a".into(), &mut ops).unwrap();
    a.commit_operations(ops).await.unwrap();
    add_task(&mut b, "only on b").await;

    a.sync(&mut server(&store).await, true).await.unwrap();
    b.sync(&mut server(&store).await, true).await.unwrap();
    a.sync(&mut server(&store).await, true).await.unwrap();

    assert_eq!(a.all_tasks().await.unwrap().len(), 2);
    assert_eq!(b.all_tasks().await.unwrap().len(), 2);
    assert_eq!(b.get_task(id).await.unwrap().unwrap().get_description(), "from a");
}

#[tokio::test]
async fn stale_parent_is_rejected_and_leaves_no_orphan() {
    use taskchampion::server::AddVersionResult;
    let store = MemStore::new();
    let mut s = CloudServer::new(store.clone(), b"hunter2").await.unwrap();

    let (res, _) = s.add_version(uuid::Uuid::nil(), b"[]".to_vec()).await.unwrap();
    let AddVersionResult::Ok(v1) = res else {
        panic!("first add must succeed")
    };

    // A second writer that still thinks the DB is empty must be told about v1.
    let (res, _) = s.add_version(uuid::Uuid::nil(), b"[]".to_vec()).await.unwrap();
    assert_eq!(res, AddVersionResult::ExpectedParentVersion(v1));
    let versions = store.list("v-").await.unwrap();
    assert_eq!(versions.len(), 1, "no stray version object: {versions:?}");
}

#[tokio::test]
async fn wrong_secret_cannot_read() {
    let store = MemStore::new();
    let mut a = replica();
    add_task(&mut a, "secret stuff").await;
    a.sync(&mut server(&store).await, true).await.unwrap();

    let mut wrong: Box<dyn Server> = Box::new(CloudServer::new(store.clone(), b"not-the-secret").await.unwrap());
    assert!(replica().sync(&mut wrong, true).await.is_err());
}

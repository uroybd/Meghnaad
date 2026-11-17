//! A [`GitRepo`] over a forge's web API, so a host that cannot run `git` (the Worker, the phone) can still keep tasks on
//! a branch. Two kinds of forge, told apart by the API's address:
//!
//! - **GitHub** (`https://api.github.com`, or GitHub Enterprise's `…/api/v3`): reads and commits through GraphQL, where
//!   one `createCommitOnBranch` makes a commit of several files and refuses it if the branch moved (`expectedHeadOid`);
//!   blobs and folders through REST.
//! - **Gitea and Forgejo** (an address ending in `/api/v1`): one `POST /contents` makes a commit of several files and
//!   refuses it if a file it updates or deletes is not the blob it names.
//!
//! What goes over the network is the host's: it hands an [`Http`] in. This module only says what to ask and reads the
//! answers. The token goes in a header and nowhere else.

use crate::error::{Error, Result};
use crate::gitsync::{Blob, Change, Committed, Entry, GitRepo};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use taskchampion::chrono::DateTime;

/// A request for the host to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<Vec<u8>>,
}

/// What came back: the status and the body. A network failure is an `Err` from [`Http::send`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// How a host sends a request.
#[allow(async_fn_in_trait)] // Workers futures are !Send, so no Send bound is wanted.
pub trait Http {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse>;
}

/// Which API a forge speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    /// Gitea and Forgejo, which share theirs.
    Gitea,
}

/// Where the branch is: the forge's API, `owner/name`, the branch, and the token that may read and write it.
#[derive(Clone)]
pub struct GitRemote {
    pub api: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
}

/// The branch the CLI's manual suggests, and so the default here too.
pub const DEFAULT_BRANCH: &str = "taskchampion";
pub const DEFAULT_API: &str = "https://api.github.com";

impl GitRemote {
    /// Check the parts and settle the defaults. The token is never part of an error.
    pub fn new(api: &str, repo: &str, branch: &str, token: &str) -> Result<GitRemote> {
        let api = match api.trim().trim_end_matches('/') {
            "" => DEFAULT_API,
            a => a,
        };
        // The token travels with every request, so only over TLS; plain http only to this machine, for testing.
        let local = api
            .strip_prefix("http://")
            .and_then(|h| h.split(['/', ':']).next())
            .is_some_and(|h| matches!(h, "localhost" | "127.0.0.1"));
        if !(api.starts_with("https://") || local) {
            return Err(Error::Invalid("the git API address must start with https://".into()));
        }
        let repo = repo.trim().trim_matches('/');
        let ok_part = |p: &str| {
            !p.is_empty() && p != "." && p != ".." && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        };
        if !matches!(repo.split_once('/'), Some((o, n)) if ok_part(o) && ok_part(n)) {
            return Err(Error::Invalid("the repository is written owner/name".into()));
        }
        let branch = match branch.trim() {
            "" => DEFAULT_BRANCH,
            b => b,
        };
        let branch_ok = branch.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
            && !branch.starts_with(['/', '.', '-'])
            && !branch.ends_with(['/', '.'])
            && !branch.contains("..")
            && !branch.contains("//");
        if !branch_ok {
            return Err(Error::Invalid(format!(
                "`{branch}` is not a branch name this app takes"
            )));
        }
        if token.trim().is_empty() {
            return Err(Error::Invalid("a token is needed to reach the repository".into()));
        }
        Ok(GitRemote {
            api: api.to_owned(),
            repo: repo.to_owned(),
            branch: branch.to_owned(),
            token: token.trim().to_owned(),
        })
    }

    pub fn forge(&self) -> Forge {
        if self.api.ends_with("/api/v1") {
            Forge::Gitea
        } else {
            Forge::GitHub
        }
    }

    fn owner_name(&self) -> (&str, &str) {
        self.repo.split_once('/').unwrap_or((&self.repo, ""))
    }

    /// GitHub's GraphQL address: `…/graphql` beside `api.github.com`, `…/api/graphql` for Enterprise (`…/api/v3`).
    fn graphql_url(&self) -> String {
        match self.api.strip_suffix("/api/v3") {
            Some(host) => format!("{host}/api/graphql"),
            None => format!("{}/graphql", self.api),
        }
    }
}

/// Escape a path for a URL, keeping its slashes.
fn path_part(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn bad_answer(what: &str) -> Error {
    Error::Store(format!("the git server's answer to {what} was not understood"))
}

/// The status and, if the forge said, why: never the request, which may carry the token.
fn failed(what: &str, res: &HttpResponse) -> Error {
    let why = serde_json::from_slice::<Value>(&res.body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default();
    let hint = match res.status {
        401 => " (the token was refused)",
        403 => " (the token may not do this, or a rate limit was reached)",
        404 => " (no such repository, or the token cannot see it)",
        _ => "",
    };
    Error::Store(
        format!("{what}: the git server answered {}{hint} {why}", res.status)
            .trim_end()
            .to_owned(),
    )
}

/// Base64 as forges send it, which may be broken into lines.
fn unbase64(text: &str) -> Result<Vec<u8>> {
    let joined: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    STANDARD.decode(joined).map_err(|_| bad_answer("a file's contents"))
}

fn parse_time(text: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(text).ok().map(|t| t.timestamp())
}

/// A [`GitRepo`] over a forge's API.
pub struct ForgeRepo<H> {
    pub http: H,
    pub remote: GitRemote,
}

impl<H: Http> ForgeRepo<H> {
    pub fn new(http: H, remote: GitRemote) -> Self {
        ForgeRepo { http, remote }
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        match self.remote.forge() {
            Forge::GitHub => vec![
                ("authorization", format!("Bearer {}", self.remote.token)),
                ("accept", "application/vnd.github+json".into()),
                ("x-github-api-version", "2022-11-28".into()),
                ("user-agent", "meghnaad".into()),
            ],
            Forge::Gitea => vec![
                ("authorization", format!("token {}", self.remote.token)),
                ("accept", "application/json".into()),
            ],
        }
    }

    async fn get(&self, url: String) -> Result<HttpResponse> {
        self.http
            .send(HttpRequest {
                method: "GET",
                url,
                headers: self.headers(),
                body: None,
            })
            .await
    }

    async fn send_json(&self, method: &'static str, url: String, body: &Value) -> Result<HttpResponse> {
        let mut headers = self.headers();
        headers.push(("content-type", "application/json".into()));
        self.http
            .send(HttpRequest {
                method,
                url,
                headers,
                body: Some(serde_json::to_vec(body).map_err(|e| Error::Store(e.to_string()))?),
            })
            .await
    }

    /// A REST address under the repository.
    fn rest(&self, rest: &str) -> String {
        format!("{}/repos/{}/{rest}", self.remote.api, self.remote.repo)
    }

    /// GitHub: run a GraphQL query or mutation. `Ok(Err(errors))` when GitHub answered with errors.
    async fn graphql(&self, query: &str, variables: Value) -> Result<std::result::Result<Value, Vec<Value>>> {
        let res = self
            .send_json(
                "POST",
                self.remote.graphql_url(),
                &json!({ "query": query, "variables": variables }),
            )
            .await?;
        if res.status != 200 {
            return Err(failed("GitHub", &res));
        }
        let mut v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a query"))?;
        if let Some(Value::Array(errors)) = v.get_mut("errors").map(Value::take) {
            if !errors.is_empty() {
                return Ok(Err(errors));
            }
        }
        Ok(Ok(v.get_mut("data").map_or(Value::Null, Value::take)))
    }

    /// GitHub: a query whose errors are failures.
    async fn query(&self, query: &str, mut variables: Value) -> Result<Value> {
        let (owner, name) = self.remote.owner_name();
        if let Value::Object(m) = &mut variables {
            m.insert("owner".into(), owner.into());
            m.insert("name".into(), name.into());
            m.insert("ref".into(), format!("refs/heads/{}", self.remote.branch).into());
        }
        match self.graphql(query, variables).await? {
            Ok(data) => Ok(data),
            Err(errors) => Err(Error::Store(format!(
                "GitHub: {}",
                errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("; ")
            ))),
        }
    }

    /// GitHub: a folder's files by the folder's tree id, however deep, by path from the root.
    async fn github_tree(&self, tree: &str, dir: &str) -> Result<Vec<Entry>> {
        let res = self.get(self.rest(&format!("git/trees/{tree}?recursive=1"))).await?;
        if res.status != 200 {
            return Err(failed("listing a folder", &res));
        }
        let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a listing"))?;
        Ok(tree_entries(&v, dir))
    }

    /// Gitea: one page of a tree (by branch or tree id): its entries and whether there are more.
    async fn gitea_tree_page(&self, tree: &str, recursive: bool, page: usize) -> Result<Option<(Value, bool)>> {
        let res = self
            .get(self.rest(&format!(
                "git/trees/{}?per_page=1000&page={page}{}",
                path_part(tree),
                if recursive { "&recursive=true" } else { "" }
            )))
            .await?;
        // A branch that does not exist is "sha not found".
        if res.status == 404 || res.status == 400 {
            return Ok(None);
        }
        if res.status != 200 {
            return Err(failed("listing the branch", &res));
        }
        let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a listing"))?;
        let more = v.get("truncated").and_then(Value::as_bool).unwrap_or(false);
        Ok(Some((v, more)))
    }

    /// Gitea: the commit the branch points at, `None` if there is no such branch.
    async fn gitea_head(&self) -> Result<Option<String>> {
        let res = self
            .get(self.rest(&format!("branches/{}", path_part(&self.remote.branch))))
            .await?;
        if res.status == 404 {
            return Ok(None);
        }
        if res.status != 200 {
            return Err(failed("reading the branch", &res));
        }
        let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a branch"))?;
        Ok(v.pointer("/commit/id").and_then(Value::as_str).map(str::to_owned))
    }

    /// Gitea: every page of a tree. `None` if there is no such tree (or branch).
    async fn gitea_tree(&self, tree: &str, recursive: bool, dir: &str) -> Result<Option<Vec<Entry>>> {
        let mut entries = Vec::new();
        for page in 1.. {
            let Some((v, more)) = self.gitea_tree_page(tree, recursive, page).await? else {
                return Ok(None);
            };
            let found = tree_entries(&v, dir);
            let empty = v.get("tree").and_then(Value::as_array).is_none_or(Vec::is_empty);
            entries.extend(found);
            if !more || empty || page >= 100 {
                break;
            }
        }
        Ok(Some(entries))
    }

    async fn gitea_commit(&self, message: &str, changes: &[Change], new_branch: bool) -> Result<Committed> {
        let files: Vec<Value> = changes
            .iter()
            .map(|c| match c {
                Change::Create { path, bytes } => {
                    json!({ "operation": "create", "path": path, "content": STANDARD.encode(bytes) })
                }
                Change::Update { path, bytes, was } => {
                    json!({ "operation": "update", "path": path, "content": STANDARD.encode(bytes), "sha": was })
                }
                Change::Delete { path, was } => json!({ "operation": "delete", "path": path, "sha": was }),
            })
            .collect();
        let who = json!({ "name": "taskchampion", "email": "taskchampion@local" });
        let mut body = json!({ "message": message, "author": who, "committer": who, "files": files });
        let branch_key = if new_branch { "new_branch" } else { "branch" };
        body[branch_key] = self.remote.branch.clone().into();
        let res = self.send_json("POST", self.rest("contents"), &body).await?;
        match res.status {
            200 | 201 => {
                let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a commit"))?;
                let sha = v
                    .pointer("/commit/sha")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad_answer("a commit"))?;
                Ok(Committed::Done(sha.to_owned()))
            }
            404 if !new_branch && String::from_utf8_lossy(&res.body).contains("branch does not exist") => {
                // The first commit makes the branch, from the repository's default one.
                Box::pin(self.gitea_commit(message, changes, true)).await
            }
            // A file is not the blob named ("sha does not match"), already exists, or is gone; or the branch was made
            // meanwhile. Somebody else got there first.
            409 | 422 => Ok(Committed::Moved),
            500 if String::from_utf8_lossy(&res.body).contains("does not exist") => Ok(Committed::Moved),
            _ => Err(failed("committing", &res)),
        }
    }

    /// GitHub: make the branch from the default branch's head, for the first commit. `Moved` either way: read again.
    async fn github_create_branch(&self) -> Result<Committed> {
        let data = self
            .query(
                "query($owner:String!,$name:String!,$ref:String!){repository(owner:$owner,name:$name){id \
                 defaultBranchRef{target{oid}} ref(qualifiedName:$ref){name}}}",
                json!({}),
            )
            .await?;
        let id = data.pointer("/repository/id").and_then(Value::as_str);
        let oid = data
            .pointer("/repository/defaultBranchRef/target/oid")
            .and_then(Value::as_str);
        let (Some(id), Some(oid)) = (id, oid) else {
            return Err(Error::Store(
                "the repository has no commits yet: add a first one (a README) and sync again".into(),
            ));
        };
        // Made by somebody else meanwhile is as good.
        self.graphql(
            "mutation($input:CreateRefInput!){createRef(input:$input){ref{name}}}",
            json!({ "input": { "repositoryId": id, "name": format!("refs/heads/{}", self.remote.branch), "oid": oid } }),
        )
        .await?
        .ok();
        Ok(Committed::Moved)
    }
}

/// The files in a tree listing (`tree: [{path, type, sha}]`), by path from the root.
fn tree_entries(v: &Value, dir: &str) -> Vec<Entry> {
    v.get("tree")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|e| e.get("type").and_then(Value::as_str) == Some("blob"))
                .filter_map(|e| {
                    let path = e.get("path").and_then(Value::as_str)?;
                    let id = e.get("sha").and_then(Value::as_str)?;
                    Some(Entry {
                        path: if dir.is_empty() {
                            path.to_owned()
                        } else {
                            format!("{dir}/{path}")
                        },
                        id: id.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// GitHub's head and one file in it: `target { oid … on Commit { file(path:) { oid object { … on Blob { text isBinary
/// isTruncated } } } } }`. The text is used when it is all there; otherwise the blob is fetched.
const GH_READ: &str = "query($owner:String!,$name:String!,$ref:String!,$path:String!){repository(owner:$owner,name:$name)\
{ref(qualifiedName:$ref){target{oid ... on Commit{file(path:$path){oid type object{... on Blob{text isBinary isTruncated}}}}}}}}";

const GH_ROOT: &str = "query($owner:String!,$name:String!,$ref:String!){repository(owner:$owner,name:$name)\
{ref(qualifiedName:$ref){target{oid ... on Commit{tree{entries{name type oid}}}}}}}";

const GH_COMMIT: &str = "mutation($input:CreateCommitOnBranchInput!){createCommitOnBranch(input:$input){commit{oid}}}";

impl<H: Http> GitRepo for ForgeRepo<H> {
    async fn read(&self, path: &str) -> Result<(Option<String>, Option<Blob>)> {
        match self.remote.forge() {
            Forge::GitHub => {
                let data = self.query(GH_READ, json!({ "path": path })).await?;
                let Some(target) = data.pointer("/repository/ref/target") else {
                    return Ok((None, None));
                };
                let head = target.get("oid").and_then(Value::as_str).map(str::to_owned);
                let Some(file) = target.get("file").filter(|f| !f.is_null()) else {
                    return Ok((head, None));
                };
                if file.get("type").and_then(Value::as_str) != Some("blob") {
                    return Ok((head, None));
                }
                let id = file
                    .get("oid")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad_answer("a read"))?;
                let object = file.get("object");
                let whole_text = object.filter(|o| {
                    o.get("isBinary").and_then(Value::as_bool) == Some(false)
                        && o.get("isTruncated").and_then(Value::as_bool) == Some(false)
                });
                let bytes = match whole_text.and_then(|o| o.get("text")).and_then(Value::as_str) {
                    Some(text) => text.as_bytes().to_vec(),
                    None => self.blob(id).await?,
                };
                Ok((
                    head,
                    Some(Blob {
                        id: id.to_owned(),
                        bytes,
                    }),
                ))
            }
            Forge::Gitea => {
                let res = self
                    .get(self.rest(&format!(
                        "contents/{}?ref={}",
                        path_part(path),
                        path_part(&self.remote.branch)
                    )))
                    .await?;
                // Gitea's commits check files, not the head, so the head is only "the branch is there" (a missing
                // branch is found when committing).
                let head = Some(String::new());
                if res.status == 404 {
                    return Ok((head, None));
                }
                if res.status != 200 {
                    return Err(failed("reading a file", &res));
                }
                let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a read"))?;
                if v.get("type").and_then(Value::as_str) != Some("file") {
                    return Ok((head, None));
                }
                let id = v
                    .get("sha")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad_answer("a read"))?;
                let bytes = match v.get("content").and_then(Value::as_str) {
                    Some(c) => unbase64(c)?,
                    None => self.blob(id).await?,
                };
                Ok((
                    head,
                    Some(Blob {
                        id: id.to_owned(),
                        bytes,
                    }),
                ))
            }
        }
    }

    async fn root(&self) -> Result<(Option<String>, Vec<Entry>)> {
        match self.remote.forge() {
            Forge::GitHub => {
                let data = self.query(GH_ROOT, json!({})).await?;
                let Some(target) = data.pointer("/repository/ref/target") else {
                    return Ok((None, Vec::new()));
                };
                let head = target.get("oid").and_then(Value::as_str).map(str::to_owned);
                let entries = target
                    .pointer("/tree/entries")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter(|e| e.get("type").and_then(Value::as_str) == Some("blob"))
                            .filter_map(|e| {
                                Some(Entry {
                                    path: e.get("name").and_then(Value::as_str)?.to_owned(),
                                    id: e.get("oid").and_then(Value::as_str)?.to_owned(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Ok((head, entries))
            }
            Forge::Gitea => {
                // By the head's commit id: the trees endpoint does not find every branch by name.
                let Some(head) = self.gitea_head().await? else {
                    return Ok((None, Vec::new()));
                };
                let entries = self.gitea_tree(&head, false, "").await?.unwrap_or_default();
                Ok((Some(head), entries))
            }
        }
    }

    async fn tree(&self, dir: &str) -> Result<Vec<Entry>> {
        match self.remote.forge() {
            Forge::GitHub => {
                let data = self.query(GH_READ, json!({ "path": dir })).await?;
                let file = data.pointer("/repository/ref/target/file");
                let tree = file
                    .filter(|f| f.get("type").and_then(Value::as_str) == Some("tree"))
                    .and_then(|f| f.get("oid"))
                    .and_then(Value::as_str);
                match tree {
                    Some(id) => self.github_tree(id, dir).await,
                    None => Ok(Vec::new()),
                }
            }
            Forge::Gitea => {
                // The folder's tree id, from a listing of the folder it is in; then the whole of it.
                let (parent, name) = dir.rsplit_once('/').unwrap_or(("", dir));
                let res = self
                    .get(self.rest(&format!(
                        "contents/{}?ref={}",
                        path_part(parent),
                        path_part(&self.remote.branch)
                    )))
                    .await?;
                if res.status == 404 {
                    return Ok(Vec::new());
                }
                if res.status != 200 {
                    return Err(failed("listing a folder", &res));
                }
                let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a listing"))?;
                let tree = v.as_array().and_then(|items| {
                    items.iter().find(|e| {
                        e.get("name").and_then(Value::as_str) == Some(name)
                            && e.get("type").and_then(Value::as_str) == Some("dir")
                    })
                });
                let Some(id) = tree.and_then(|t| t.get("sha")).and_then(Value::as_str) else {
                    return Ok(Vec::new());
                };
                Ok(self.gitea_tree(id, true, dir).await?.unwrap_or_default())
            }
        }
    }

    async fn blob(&self, id: &str) -> Result<Vec<u8>> {
        let res = self.get(self.rest(&format!("git/blobs/{}", path_part(id)))).await?;
        if res.status != 200 {
            return Err(failed("reading a file", &res));
        }
        let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a file"))?;
        let content = v
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| bad_answer("a file"))?;
        unbase64(content)
    }

    async fn commit(&self, head: Option<&str>, message: &str, changes: &[Change]) -> Result<Committed> {
        match self.remote.forge() {
            Forge::Gitea => self.gitea_commit(message, changes, false).await,
            Forge::GitHub => {
                let Some(head) = head else {
                    return self.github_create_branch().await;
                };
                let mut additions = Vec::new();
                let mut deletions = Vec::new();
                for c in changes {
                    match c {
                        Change::Create { path, bytes } | Change::Update { path, bytes, .. } => {
                            additions.push(json!({ "path": path, "contents": STANDARD.encode(bytes) }));
                        }
                        Change::Delete { path, .. } => deletions.push(json!({ "path": path })),
                    }
                }
                let input = json!({
                    "branch": { "repositoryNameWithOwner": self.remote.repo, "branchName": self.remote.branch },
                    "expectedHeadOid": head,
                    "message": { "headline": message },
                    "fileChanges": { "additions": additions, "deletions": deletions },
                });
                match self.graphql(GH_COMMIT, json!({ "input": input })).await? {
                    Ok(data) => data
                        .pointer("/createCommitOnBranch/commit/oid")
                        .and_then(Value::as_str)
                        .map(|oid| Committed::Done(oid.to_owned()))
                        .ok_or_else(|| bad_answer("a commit")),
                    Err(errors) => {
                        let moved = errors.iter().any(|e| {
                            e.get("type").and_then(Value::as_str) == Some("STALE_DATA")
                                || e.get("message")
                                    .and_then(Value::as_str)
                                    .is_some_and(|m| m.contains("Expected branch to point to"))
                        });
                        if moved {
                            Ok(Committed::Moved)
                        } else {
                            Err(Error::Store(format!(
                                "GitHub refused the commit: {}",
                                errors
                                    .iter()
                                    .filter_map(|e| e.get("message").and_then(Value::as_str))
                                    .collect::<Vec<_>>()
                                    .join("; ")
                            )))
                        }
                    }
                }
            }
        }
    }

    async fn last_changed(&self, paths: &[String]) -> Result<Vec<Option<i64>>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        match self.remote.forge() {
            Forge::GitHub => {
                // One query, a `history` for each path under an alias of its own.
                let mut fields = String::new();
                for (i, p) in paths.iter().enumerate() {
                    let quoted = serde_json::to_string(p).map_err(|e| Error::Store(e.to_string()))?;
                    fields.push_str(&format!(
                        "h{i}:history(path:{quoted},first:1){{nodes{{committedDate}}}} "
                    ));
                }
                let query = format!(
                    "query($owner:String!,$name:String!,$ref:String!){{repository(owner:$owner,name:$name)\
                     {{ref(qualifiedName:$ref){{target{{... on Commit{{{fields}}}}}}}}}}}"
                );
                let data = self.query(&query, json!({})).await?;
                let target = data.pointer("/repository/ref/target");
                Ok((0..paths.len())
                    .map(|i| {
                        target
                            .and_then(|t| t.pointer(&format!("/h{i}/nodes/0/committedDate")))
                            .and_then(Value::as_str)
                            .and_then(parse_time)
                    })
                    .collect())
            }
            Forge::Gitea => {
                let mut out = Vec::with_capacity(paths.len());
                for p in paths {
                    let res = self
                        .get(self.rest(&format!(
                            "commits?sha={}&path={}&limit=1&stat=false&verification=false&files=false",
                            path_part(&self.remote.branch),
                            path_part(p)
                        )))
                        .await?;
                    if res.status != 200 {
                        out.push(None);
                        continue;
                    }
                    let v: Value = serde_json::from_slice(&res.body).map_err(|_| bad_answer("a history"))?;
                    out.push(
                        v.pointer("/0/commit/committer/date")
                            .and_then(Value::as_str)
                            .and_then(parse_time),
                    );
                }
                Ok(out)
            }
        }
    }

    fn history_batch(&self) -> usize {
        match self.remote.forge() {
            Forge::GitHub => 100,
            Forge::Gitea => 20,
        }
    }
}

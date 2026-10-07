#!/usr/bin/env bash
# Fill the LOCAL dev bucket with demo data so the UI has something to show.
# Needs `npm run dev` (wrangler) running with DEV_AUTH_BYPASS. Safe: only talks to localhost.
set -euo pipefail
API=${API:-http://127.0.0.1:8787}
cli()  { curl -sf -X POST "$API/api/cli" -H 'content-type: application/json' -d "$1"; }
line() { cli "$(python3 -c 'import json,sys; print(json.dumps({"line": sys.argv[1]}))' "$1")"; }
args() { cli "$(python3 -c 'import json,sys; print(json.dumps({"args": sys.argv[1:]}))' "$@")"; }
uuid_of() { python3 -c 'import sys,json; print(json.load(sys.stdin)["result"]["tasks"][0]["uuid"])'; }

curl -sf "$API/api/health" >/dev/null || { echo "API not reachable at $API" >&2; exit 1; }
n=$(curl -sf "$API/api/health" | python3 -c 'import sys,json; print(json.load(sys.stdin)["tasks"])')
[ "$n" = 0 ] || { echo "bucket already has $n tasks; not seeding (reset: rm -rf .wrangler/state/v3/r2)" >&2; exit 1; }

today=$(date +%F)

# 1. A first taskrc defines two UDAs; some tasks use both.
curl -sf -X PUT "$API/api/config/taskrc" --data-binary @- >/dev/null <<RC
uda.estimate.type=string
uda.estimate.label=Size
uda.estimate.values=big,medium,small
uda.points.type=numeric
uda.points.label=Points
RC

line "add Write quarterly report project:Work priority:H due:tomorrow +writing estimate:big" >/dev/null
review=$(line "add Review pull requests project:Work priority:M due:${today}T16:00 +code estimate:medium points:3" | uuid_of)
line "add Buy milk and eggs project:Home.Kitchen due:${today}T17:30 +errand estimate:small" >/dev/null
line "add Fix the leaky tap project:Home priority:L due:-2d +fixme estimate:medium" >/dev/null
plan=$(line "add Plan the holiday project:Home +planning wait:3d" | uuid_of)
args add "description:Book flights" project:Home "depends:$plan" due:2026-12-20 >/dev/null
line "add Email the accountant project:Work +errand estimate:small" >/dev/null
line "add Water the plants due:${today}" >/dev/null
old=$(line "add Renew passport project:Home" | uuid_of)

args "$review" start >/dev/null
args "$review" annotate "waiting on CI before merging" >/dev/null
args "$review" annotate "ask Sam about the migration" >/dev/null
args "$old" done >/dev/null

# 2. The final taskrc: `points` is no longer defined, so it becomes an orphan on the review task
#    (shown, read-only). Fake secrets are included on purpose: they must be blocked.
curl -sf -X PUT "$API/api/config/taskrc" --data-binary @- <<RC | python3 -m json.tool
# ---- demo taskrc ----
uda.estimate.type=string
uda.estimate.label=Size
uda.estimate.values=big,medium,small

report.work.description=Work tasks by size
report.work.columns=id,priority,estimate,due.relative,description
report.work.labels=ID,P,Size,Due,Task
report.work.sort=estimate-,due+
report.work.filter=project:Work status:pending

report.today.description=Due today or overdue
report.today.columns=id,start.active,due,project,description.truncated
report.today.labels=ID,A,Due,Project,Task
report.today.sort=due+,urgency-
report.today.filter=status:pending (+TODAY or +OVERDUE)

context.work.read=project:Work
context.work.write=project:Work

urgency.user.tag.errand.coefficient=1.5
default.command=next

# time tracking: `start`/`stop` add "Started task"/"Stopped task" notes, and the UI shows time tracked
journal.time=on

# fake credentials: these must be blocked, never stored
sync.encryption_secret=DEMO-not-a-real-secret
sync.aws.access_key_id=AKIADEMODEMODEMO
sync.aws.secret_access_key=demo-demo-demo-demo
sync.aws.bucket=demo-bucket
RC
echo
echo "Seeded. Open the UI and try: next, work, today, list +errand, info <id>"

# Hooks

Taskwarrior runs your scripts when a command starts, when a task is added or changed, and when it ends. A browser can't
run `~/.task/hooks/on-add.*`, so here a hook is a function in a **Lua script** that you write in **Settings → Hooks**.
The script lives in your bucket (as `web/hooks.lua`, beside your settings), not in the app: change it and the next
command uses it, with no rebuild or deploy. It runs on the server, behind your sign-in, so a script can later hold
secrets that never reach the page. Your desktop's own hooks are separate: they only fire for commands run there, and these
only for commands run here, unless you run this same script on the desktop too ([below](#the-same-script-with-the-real-task)).

A hook is a request to the server, and a command waits for each one in turn (`add` asks for `on_launch`, `on_add` and `on_exit`),
as `task` waits for its scripts. A new task does not hold the page up for that: the **+** and the full form wait a second as
before, so a refusal comes back to the form it was typed in; after that they close, and a **Creating…** notice stays until the
task is made, when the usual notice (or what a hook said) takes its place. If a hook then refuses it, the one-line add opens
again with what you typed.

Define any of four functions, each at most once:

| Hook | Runs | It can |
| --- | --- | --- |
| `on_launch(command)` | before a command does anything; `command` is the line | refuse it |
| `on_add(task)` | for each new task, before it is saved | change the task, or refuse |
| `on_modify(old, new)` | for each task `modify`, `done`, `delete`, `start`, `stop`, `annotate`… changes, before it is saved | change `new`, or refuse |
| `on_exit(changed)` | after the command, with the list of tasks it changed | only talk |

```lua
-- Anything in project Work (or below it) is also tagged +office.
function on_add(task)
  if task:in_project("Work") then
    task.tags:add("office")
    say("Tagged +office.")
  end
end

-- A task can't be finished while it is blocked, and a due date is never left in the past.
function on_modify(old, new)
  if old.status == "pending" and new.status == "completed" and old.blocked then
    refuse("Finish what it depends on first.")
  end
  if new.due ~= nil and new.due < today() then
    new.due = tomorrow()
  end
end
```

**A task** is a table: `description`, `project`, `priority`, `status`, `uuid`, `blocked`, `blocking`, `recur`,
`annotations` (a list of `{entry, text}`), the dates, and your UDAs as plain fields (`task.estimate = "big"`; set one to
`nil` to remove it). Change the task you are given, or return one. What a hook may change is what `modify` may change;
`uuid`, `status`, annotations and the recurrence fields can be read but not changed, and changing them is an error.
The change is applied in the same undo step as the command, and a refused command saves nothing. (`old` in `on_modify` is
for reading; changes to it are ignored.)

**Tags** (and `task.depends`, a set of task ids as text) are a set: `task.tags:has("work")`, `:add("a", "b")`,
`:remove("x")`, `:has_any("a", "b")`, `:has_all(…)`, `:list()`, `#task.tags`, and `for t in task.tags:each() do … end`.

**Dates** (`entry`, `modified`, `start`, `due`, `wait`, `scheduled`, and `finished` and `expires` for `end` and `until`,
which are keywords in Lua) are values you can calculate with. They are written and read as Taskwarrior does:
`date("friday")`, `date("2026-12-25")`, `date("eom")`, `date("now+3d")`, `now()`, `today()`, `tomorrow()`. Add or take away
a duration (`days(2)`, `hours(3)`, `minutes(30)`, `weeks(1)`, `duration("3d")`), compare (`d1 < d2`), take one from another
(`d2 - d1` is a duration, with `:in_days()`, `:in_hours()` and so on), and read the parts (`d.year`, `.month`, `.day`,
`.hour`, `.min`, `.sec`, `.weekday` from 0 for Sunday). `d:format("Y-M-D")` uses Taskwarrior's date-format letters, and
`d:start_of("week")` takes `day`, `week`, `month` or `year`. They follow your time zone and `dateformat`.

**Helpers**: `task:in_project("Work")` (also true for `Work.Mail`), `:is_pending()`, `:is_completed()`, `:is_deleted()`,
`:is_recurring()`, `:is_waiting()`, `:is_overdue()`. `say("…")` and `warn("…")` (and `print`) show under the command's
result in the Console, or as a toast elsewhere. `refuse("why")`, or any error in the script, stops the command with that
message; an error in `on_exit` is shown but cannot stop anything.

**The network and secrets.** A script runs on the server, so it can reach other services, to hosts it declares at the top
and over https only (`http://` is accepted only for `localhost`, for testing):

```lua
allow_hosts{"acme.atlassian.net"}

function on_add(task)
  local res, err = http.post("https://acme.atlassian.net/rest/api/3/issue", {
    headers = { Authorization = "Basic " .. secret("jira") },
    json = { fields = { summary = task.description } },
  })
  if res == nil then
    warn("Jira unreachable: " .. err)   -- the network failed: carry on without it
    return
  end
  task.jira = res.json.key              -- a UDA named jira
  say("Created " .. res.json.key)
end
```

`http.get`, `post`, `put`, `patch` and `delete` (or `http.request(method, url, opts)`) take `opts` of `headers = { Name =
"value" }`, `json = <table>` (sent as JSON) or `body = "text"`, and `timeout = seconds` (up to 10). The answer is `{ status, ok,
headers, body, json }` (`json` when the body is JSON; header names are lowercase), or `nil` and a reason when the request
could not be made or answered, which is the one failure a script is expected to handle. A request to a host the script did
not declare, to a plain `http://` address, or a sixth request in one call, is an error that ends the hook with a message.
`http.urlencode(text)` makes text safe to put in an address. Only the hooks you define are called, and each request waits
at most the timeout; a slow service slows the command, so give it a short one.

`secret("name")` is a value kept apart from the script, so the script holds no token. Set secrets in **Settings → Hooks →
Secrets**: a secret is sealed in your bucket with the same key as your tasks, can be replaced but never shown again, and is
hidden from anything a hook prints, fails with or returns (it is replaced by `***`). Or set a Worker secret called
`HOOK_SECRET_<NAME>` (`wrangler secret put HOOK_SECRET_JIRA`, the dashboard, or your deploy workflow); that is read the same
way, wins over one kept in the app, and never touches the bucket. Each call is logged as `hook http METHOD host -> status`
(never the path, which may hold a token). A script that needs more than five requests, or a secret that is missing, fails
the hook with a message that says so.

The editor highlights Lua and marks a mistake with its line as you type, using the same Lua that will run the script. A
script that does not load cannot be saved, so a saved script always runs; the one before it is kept. The language is
Lua 5.4 with `string`, `table` and `math` only: no files, no clock but the command's, and a script that runs too long (a
few million steps) or takes too much memory is stopped with a message.

Notes: `hooks=off` (or `rc.hooks:off` for one command, or `config hooks off`) turns them all off. `undo` doesn't run hooks,
as in Taskwarrior. The housekeeping before a report does, as there: `on_add` for each recurring instance it generates, and
`on_modify` for the parent whose mask changed and for a task it expires (a series that has ended, a task past its
`until`). A hook that refuses one of these stops the command with its message and writes nothing, where Taskwarrior can
leave the instances made before the refusal. A command that asks a question first runs `on_launch` once per round.

Each hook that runs is a request to the server, so only the hooks your script defines are called, and a command with
none of them is as fast as before. If the server can't be reached the hook is skipped with a warning that says so, and
the command carries on (your own `refuse` or error still stops it). The time a hook spends waiting on the network is not
CPU time, but it is time your command waits.

## The same script with the real `task`

`meghnaad-hook` runs the script for the `task` on your computer, with the same Lua, the same library (`task:in_project`,
`date("friday")`, `say`, `refuse`) and the same `http`, so a script written in Settings → Hooks works there unchanged and the
other way round. `task` runs an executable per hook from `<data>/hooks/`; `meghnaad-hook` is that executable, linked under
the names `task` looks for.

First get the program, once, in any of three ways. None needs a clone of this repository.

- **Homebrew** (macOS and Linux): `brew install uroybd/tap/meghnaad-hook`, and `brew upgrade` for a newer one. The
  links `install` makes (below) point at Homebrew's `opt` path, which stays the same program through an upgrade, so nothing
  needs installing again.
- **Prebuilt** (no Rust): download `meghnaad-hook-<version>-<platform>.tar.gz` from the
  [latest release](https://github.com/uroybd/Meghnaad/releases/latest) (Linux or macOS, x86_64 or arm64) and put the
  `meghnaad-hook` inside where you keep programs, for example `~/.local/bin`. On macOS, if it is blocked for coming from the
  internet, `xattr -d com.apple.quarantine ~/.local/bin/meghnaad-hook` (or fetch it with `curl`, which does not mark it).
- **From source** (needs Rust): `cargo install --git https://github.com/uroybd/Meghnaad tc-hook --locked`, which puts it in
  `~/.cargo/bin`. Add `--tag v<version>` for a release rather than the latest commit.

```sh
meghnaad-hook check ~/hooks.lua                # which hooks the script defines, or the line that is wrong
meghnaad-hook install ~/hooks.lua              # ~/.task/hooks.lua -> your script; ~/.task/hooks/on-add.meghnaad -> the program, …
```

To try it, save the first example above (the one that tags Work tasks) as `~/hooks.lua`, run the two commands, then:

```console
$ task add Write report project:Work
Created task 1.
Tagged +office.
The project 'Work' has changed.  Project 'Work' is 0% complete (1 task remaining).
$ task diag | grep -A4 Hooks         # Active: on-add.meghnaad (executable) (symlink)
```

From then on, edit the script and the next `task` command uses it; nothing needs installing again unless the script
defines a hook it did not before (or stops defining one). The `meghnaad-hook` links point at the program's own path, so
reinstall after you move it or build a new one somewhere else (a Homebrew upgrade is not one: see above).

- **Where things are.** The data directory is `--data DIR`, else `$TASKDATA`, else `~/.task`. The script is
  `$MEGHNAAD_HOOKS`, else `<data>/hooks.lua`; `install SCRIPT` makes that a link to SCRIPT, so the script can live in a
  dotfiles repository. `install` links only the hooks the script defines (a `task` command starts one process per hook file,
  so an undefined one would be wasted) and removes the links of ones it no longer defines: run it again after adding a
  function. `meghnaad-hook uninstall` removes the `*.meghnaad` links (and only those, so your own hook executables stay;
  `hooks.lua` stays too). Links go in `<data>/hooks`. The taskrc's `hooks.location` is not read (it may be in an
  `include`d file), so if you set one, give that directory to `install` and `uninstall` as `--hooks DIR`.
  `meghnaad-hook check` with no argument checks the script it would use. A hook that finds no script at that place prints
  `meghnaad-hook: there is no script at …` and lets the command carry on.
- **What it does.** `on-add` and `on-modify` give the script the task and print it back; what the script cannot change
  (uuid, status, annotations, recurrence) is refused as in the web app, and anything it did not touch, `id` and `urgency`
  included, goes back as `task` wrote it. `refuse("why")`, an error in the script, or a script that does not load stops the
  command and shows the message (`task` exits 4). `say` and `warn` print under the command's result; a `warn` starts with
  `Warning:`. `on-exit` can only talk. `on-launch` is given the command line without `task`. `hooks=off` in the taskrc turns
  them off, as for any hook.
- **Dates** are read and written in this computer's time zone and your taskrc's `dateformat`, `date.iso` and `weekstart`.
  `rc.` overrides on the command line and `include`d rc files are not seen.
- **The network** works as on the server: `allow_hosts{…}` at the top, https only (`http://localhost` for testing), at most
  five requests a call and ten seconds each, redirects are the script's to follow.
- **Secrets** are environment variables: `secret("JIRA_TOKEN")` is the variable `JIRA_TOKEN`, by that exact name and no
  other (the environment is shared with every other program, so no prefix is added or guessed), and a missing one is an
  error that stops the hook. Its value is replaced by `***` in anything the script prints, fails with or returns. Set them
  where `task` is started from (your shell profile, or `launchctl setenv` for a GUI); the Worker's secrets and the app's are
  separate.
- **Differences.** `task.blocked` and `task.blocking` are always false (`task` does not send them and a hook cannot ask it,
  as it holds the data). A date UDA reads and writes as seconds, as everywhere in the script.

# Hooks

Taskwarrior runs your scripts when a command starts, when a task is added or changed, and when it ends. A browser can't
run `~/.task/hooks/on-add.*`, so here a hook is a function in a **Lua script** you write in **Settings → Hooks**.

The script lives in your bucket (as `web/hooks.lua`, beside your settings), not in the app, so a change is used by the
next command with no rebuild or deploy. It runs on the server behind your sign-in, so it can hold secrets that never
reach the page. Your desktop's own hooks are separate: they fire only for commands run there, unless you run this same
script on the desktop too ([below](#the-same-script-with-the-real-task)).

A command waits for each hook in turn, as `task` waits for its scripts (`add` asks for `on_launch`, `on_add` and
`on_exit`). A new task doesn't hold the page up for that: **+** and the full form wait a second, so a refusal comes back
to the form it was typed in, then they close and a **Creating…** notice stays until the task is made. If a hook refuses
it then, the one-line add opens again with what you typed.

## The four hooks

Define any of them, each at most once:

| Hook | Runs | It can |
| --- | --- | --- |
| `on_launch(command)` | before a command does anything; `command` is the line | refuse it |
| `on_add(task)` | for each new task, before it is saved | change the task, or refuse |
| `on_modify(old, new)` | for each task `modify`, `done`, `delete`, `start`, `stop`, `annotate`… changes, before it is saved | change `new`, or refuse |
| `on_exit(changed)` | after the command, with the tasks it changed | only talk |

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

## Tasks, tags and dates

**A task** is a table: `description`, `project`, `priority`, `status`, `uuid`, `blocked`, `blocking`, `recur`,
`annotations` (a list of `{entry, text}`), the dates, and your UDAs as plain fields (`task.estimate = "big"`; set one to
`nil` to remove it). Change the task you are given, or return one. A hook may change what `modify` may change: `uuid`,
`status`, annotations and the recurrence fields can be read but not changed, and trying is an error. The change joins
the command's undo step, and a refused command saves nothing. In `on_modify`, `old` is read-only.

**Tags** (and `task.depends`, a set of task ids as text) are a set: `task.tags:has("work")`, `:add("a", "b")`,
`:remove("x")`, `:has_any("a", "b")`, `:has_all(…)`, `:list()`, `#task.tags`, and `for t in task.tags:each() do … end`.

**Dates** (`entry`, `modified`, `start`, `due`, `wait`, `scheduled`, and `finished` and `expires` for `end` and `until`,
which are Lua keywords) can be calculated with:

- Make one: `date("friday")`, `date("2026-12-25")`, `date("eom")`, `date("now+3d")`, `now()`, `today()`, `tomorrow()`.
- Add or subtract a duration: `days(2)`, `hours(3)`, `minutes(30)`, `weeks(1)`, `duration("3d")`.
- Compare (`d1 < d2`) or subtract (`d2 - d1` is a duration, with `:in_days()`, `:in_hours()` and so on).
- Read parts: `d.year`, `.month`, `.day`, `.hour`, `.min`, `.sec`, `.weekday` (0 is Sunday).
- Format with `d:format("Y-M-D")` (Taskwarrior's letters) or round down with `d:start_of("week")` (`day`, `week`,
  `month`, `year`).

They follow your time zone and `dateformat`.

## Helpers and messages

- `task:in_project("Work")` (also true for `Work.Mail`), `:is_pending()`, `:is_completed()`, `:is_deleted()`,
  `:is_recurring()`, `:is_waiting()`, `:is_overdue()`.
- `say("…")`, `warn("…")` and `print` show under the command's result in the Console, or as a toast elsewhere.
- `refuse("why")`, or any error in the script, stops the command with that message. An error in `on_exit` is shown but
  stops nothing.

## The network and secrets

A script can reach other services, to hosts it declares at the top, over https only (`http://` only for `localhost`, to
test):

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
"value" }`, `json = <table>` or `body = "text"`, and `timeout = seconds` (up to 10). The answer is `{ status, ok, headers,
body, json }` (`json` when the body is JSON; header names are lowercase), or `nil` and a reason when the request couldn't
be made or answered, the one failure a script is expected to handle. A host not declared, a plain `http://` address, or a
sixth request in one call is an error that ends the hook. `http.urlencode(text)` makes text safe for an address. A slow
service slows the command, so give it a short timeout.

`secret("name")` is a value kept apart from the script. Set secrets in **Settings → Hooks → Secrets**: each is sealed in
your bucket with the same key as your tasks, can be replaced but never shown again, and is replaced by `***` in anything
a hook prints, fails with or returns. Or set a Worker secret called `HOOK_SECRET_<NAME>` (`wrangler secret put
HOOK_SECRET_JIRA`, the dashboard, or your deploy workflow): it is read the same way, wins over one kept in the app, and
never touches the bucket. Each call is logged as `hook http METHOD host -> status`, never the path, which may hold a
token. A missing secret fails the hook with a message that says so.

## The editor and the sandbox

The editor highlights Lua and marks a mistake with its line as you type, using the same Lua that runs the script. A
script that doesn't load can't be saved, so a saved script always runs, and the one before it is kept. The language is
Lua 5.4 with `string`, `table` and `math` only: no files, no clock but the command's. A script that runs too long (a few
million steps) or uses too much memory is stopped with a message.

## Good to know

- `hooks=off` (or `rc.hooks:off` for one command, or `config hooks off`) turns them all off.
- `undo` doesn't run hooks, as in Taskwarrior.
- The housekeeping before a report runs hooks as there: `on_add` for each recurring instance it makes, and `on_modify`
  for the parent whose mask changed and for a task it expires. If a hook refuses one of these, the command stops with its
  message and writes nothing, where Taskwarrior can leave the instances made before the refusal.
- A command that asks a question first runs `on_launch` once per round.
- Only the hooks your script defines are called, so a command with none is as fast as before. If the server can't be
  reached, the hook is skipped with a warning and the command carries on (your own `refuse` or error still stops it).

## The same script with the real `task`

`meghnaad-hook` runs the script for the `task` on your computer, with the same Lua, library and `http`, so a script
written in Settings → Hooks works there unchanged, and the other way round. `task` runs an executable per hook from
`<data>/hooks/`; `meghnaad-hook` is that executable, linked under the names `task` looks for.

Get the program once, any of three ways (none needs a clone of this repository):

### Homebrew

For macOS and Linux: `brew install uroybd/tap/meghnaad-hook`, and `brew upgrade` for a newer one. The links
`install` makes point at Homebrew's `opt` path, which survives an upgrade, so nothing needs installing again.

### Prebuilt

No Rust needed: download `meghnaad-hook-<version>-<platform>.tar.gz` from the
[latest release](https://github.com/uroybd/Meghnaad/releases/latest) (Linux or macOS, x86_64 or arm64) and put
`meghnaad-hook` where you keep programs, for example `~/.local/bin`. On macOS, if it is blocked for coming from the
internet, run `xattr -d com.apple.quarantine ~/.local/bin/meghnaad-hook` (or fetch it with `curl`).

### From source

Needs Rust: `cargo install --git https://github.com/uroybd/Meghnaad tc-hook --locked`, which puts it in
`~/.cargo/bin`. Add `--tag v<version>` for a release instead of the latest commit.

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

From then on, edit the script and the next `task` command uses it. Run `install` again only when the script starts or
stops defining a hook, or after you move the program or build a new one somewhere else (a Homebrew upgrade isn't one).

### Where things are

The data directory is `--data DIR`, else `$TASKDATA`, else `~/.task`. The script is
`$MEGHNAAD_HOOKS`, else `<data>/hooks.lua`; `install SCRIPT` makes that a link to SCRIPT, so the script can live in a
dotfiles repository. `install` links only the hooks the script defines (each hook file costs a process per `task`
command) and removes the links of ones it no longer defines.

### Removing and other places

`meghnaad-hook uninstall` removes the `*.meghnaad` links, and only those; your own hook
executables and `hooks.lua` stay. Links go in `<data>/hooks`. The taskrc's `hooks.location` is not read (it may be in an
included file), so if you set one, pass that directory to `install` and `uninstall` as `--hooks DIR`.
`meghnaad-hook check` with no argument checks the script it would use. If there is no script, a hook prints
`meghnaad-hook: there is no script at …` and the command carries on.

### What it does

`on-add` and `on-modify` give the script the task and print it back. What the script can't change
(uuid, status, annotations, recurrence) is refused as in the web app, and anything it didn't touch, `id` and `urgency`
included, goes back as `task` wrote it. `refuse("why")`, an error, or a script that doesn't load stops the command and
shows the message (`task` exits 4). `say` and `warn` print under the command's result; a `warn` starts with
`Warning:`. `on-exit` can only talk. `on-launch` gets the command line without `task`. `hooks=off` in the taskrc turns
them off.

### Dates

Dates use this computer's time zone and your taskrc's `dateformat`, `date.iso` and `weekstart`. `rc.` overrides on
the command line and included rc files are not seen.

### The network

The network works as on the server: `allow_hosts{…}` at the top, https only (`http://localhost` for testing), at
most five requests a call and ten seconds each; following redirects is the script's job.

### Secrets

Secrets are environment variables: `secret("JIRA_TOKEN")` is the variable `JIRA_TOKEN`, by that exact name (nothing
is prefixed or guessed, because the environment is shared with every program). A missing one is an error that stops
the hook, and its value is replaced by `***` in anything the script prints, fails with or returns. Set them where
`task` starts from (your shell profile, or `launchctl setenv` for a GUI). The Worker's and the app's secrets are separate.

### Differences

`task.blocked` and `task.blocking` are always false (`task` doesn't send them, and a hook can't ask,
since `task` holds the data). A date UDA reads and writes as seconds, as everywhere in the script.


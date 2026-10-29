# Different from the desktop

Where this app is not the `task` you know:

## Numeric ids are this app's own

Pending tasks are numbered by creation time, so they won't match your laptop's.
Use uuid prefixes where it matters.

## `undo`

Covers the last few commands made while this tab is open. Closing or reloading the tab forgets them.

## Hooks are Lua

, not scripts. A browser can't run local programs, so hooks are functions in a script you edit in
Settings. See [Hooks](hooks.md#hooks).

## Task commands and `settings.json` are the app's own

A desktop `task` knows nothing of them, though your taskrc's
`alias.*` still works there. See [Commands](settings.md#commands).

## `&&`, `||` and `;` belong to the console

, as in a shell: the page splits the line and sends one command at a
time. Taskwarrior's own filter also reads `&&` and `||` but evaluates them wrongly (`+a || +b` finds only `+b`), so
an unquoted operator here always ends a command. In a filter, write `and` and `or`. See
[the console](console.md#the-console).

## tasksh is only `review`

The rest of the shell (`!`, `exec`, `diagnostics`, its prompt and history) means nothing
in a browser, and `e` opens the app's editor, not `$EDITOR`. The `reviewed` attribute and `_reviewed` report the tasksh
tab defines are the lines tasksh adds to a desktop taskrc, so you can copy them there.

## `edit` and `import-v2` are missing

The detail view edits a task, and a browser can't read Taskwarrior 2's data
files. `purge` works, with one difference: it removes the task from your current tasks, but older versions in the
bucket still hold it until they are cleaned up. The app does that as the CLI does: after about one push in twenty,
once they are over 180 days old and a snapshot covers them.

## Storage, sync and offline

### Where your tasks live

The engine runs in the page, with a copy saved in the browser (IndexedDB), so the next visit
pulls only what changed. A new device, or one whose copy was thrown away, downloads the newest snapshot, which takes
a moment for a long history.

### Syncing

A change applies at once and is pushed in the background. The page pulls what other devices did when you
return to the tab and every 30 seconds while you look at it, so a change from your laptop can take that long to show.
Tabs of the same browser tell each other.

### Offline

If the server can't be reached, a banner says so. Changes keep working, are kept in the browser, and go up
with the next sync that works (**Retry** tries now). The page itself must be loaded from the server, so it can't
start with no connection. **Start over from the server**, in the same banner, discards this browser's copy (including
changes that never went up) and loads the tasks again. Use it when the copy and the bucket disagree, say after a
replaced bucket.

### What it needs

Web Workers and WebAssembly, which every current browser has. If the engine can't start, the page
says why and runs no command: nothing on the server can run one instead.


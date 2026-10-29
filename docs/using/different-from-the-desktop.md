# Different from the desktop

- **Numeric ids belong to this app** (pending tasks numbered by creation time), so they won't match your laptop's. Use
  uuid prefixes where it matters.
- **`undo`** works on the last few commands made while this tab is open; it is forgotten when the tab is closed or reloaded.
- **Hooks are Lua, not scripts** (`on-add`, `on-modify`, …): a browser can't run local programs, so they are functions in
  a Lua script that you edit in Settings and that runs on the server. See [Hooks](hooks.md#hooks).
- **Task commands and `settings.json` are the app's own.** They are not in your taskrc, so a desktop `task` knows nothing
  of them; the taskrc's `alias.*` still works there. See [Commands](settings.md#commands).
- **`&&`, `||` and `;` are the console's, as a shell's would be.** The page splits the line and sends the engine one
  command at a time. Taskwarrior's own filter reads `&&` and `||` too, but evaluates them wrongly (`+a || +b` finds only
  the tasks with `+b`), so here an operator that is not quoted always ends a command: write `and` and `or` in a filter.
  See [the console](console.md#the-console).
- **tasksh** is only its `review` here. The rest of the shell (`!` and `exec`, `diagnostics`, its own prompt and
  history) has no meaning in a browser. `e` opens the app's editor, not `$EDITOR`. The `reviewed` attribute and
  `_reviewed` report the tasksh tab defines are the same lines tasksh adds to a desktop taskrc, so you can copy them there.
- **`edit` and `import-v2`** are not in the app (the detail view edits a task; Taskwarrior 2's data files can't be read
  in a browser). `purge` is, with one difference: it removes a task from your current tasks, but the earlier versions in
  your bucket still hold what it used to say until they are cleaned up, which the app does as the CLI does, after
  about one push in twenty, once they are over 180 days old and a snapshot covers them.
- **Where your tasks live.** The engine runs in this page and keeps your tasks in memory, with a copy saved in the
  browser (IndexedDB) so that the next visit starts from it and only pulls what changed. The first visit on a device,
  or after the saved copy is thrown away, downloads the newest snapshot, which takes a moment for a long history.
- **Syncing.** A change is applied at once and pushed to the bucket in the background. The page pulls what other devices
  (your laptop's `task sync`) have done when you come back to the tab and every 30 seconds while you look at it, so a
  change from the desktop can take up to that long to appear. Several tabs of the same browser tell each other.
- **Offline.** If the server cannot be reached, a banner says so; your changes still work and are kept in the browser, and
  they go up with the next sync that works (**Retry** tries now). The page itself still has to be loaded from the
  server, so you cannot open it with no connection. **Start over from the server**, in the same banner, throws this
  browser's copy away (including changes that never went up) and loads the tasks again, for when the copy and the
  bucket no longer agree (a replaced bucket, say).
- **What it needs.** Web Workers and WebAssembly, which every current browser has. If the page cannot start the engine
  it says why and runs no command: nothing on the server can run one instead.

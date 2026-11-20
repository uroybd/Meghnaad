# 0026: Kanban

- **Status:** done <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-20
- **Layer:** 2 (UI). The automatic setup writes the taskrc with `config` lines, as the tasksh tab does
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** not a board (`docs/using/boards.md`); closest to a report on the Tasks page, whose filter,
  sort and columns it takes. The tasksh tab (`docs/using/settings.md#tasksh`) is the model for the setup. Task commands
  (`web/src/lib/taskCommands.ts`) are what a move runs

## Why

Taskwarrior has no kanban, and people build one out of a UDA or tags plus a TUI. A board can show tasks grouped by a
field, but you can't move a task from one group to another. A kanban is ordered columns, each with a filter that picks
its tasks and a command that runs on a task moved into it. Moving a card is then a Taskwarrior command, as it would be
in a shell, so it works with any taskrc and any sync.

## What

### Settings → Kanban (`/settings/kanban`)

Kanban is **off** until it is enabled on this tab. While it is off, the tab offers two ways to set it up:

- **Automatic.** The app defines a `progressState` UDA and creates a first kanban whose columns use it. Like the
  tasksh tab, the tab says whether the UDA is defined, defined differently (shown as it is now) or missing, and lists
  the exact `config` lines **Apply** writes. Nothing else in the taskrc changes:

  ```text
  config uda.progressState.type string
  config uda.progressState.label Progress
  config uda.progressState.values inprogress,review
  ```

  The kanban is named "Kanban", on report `all`, with the filter `( +PENDING or +COMPLETED )`. `all` is the one
  built-in report that shows completed tasks (`next`, `list`, `ls` and `minimal` filter to `+PENDING`, so a Done
  column on them would always be empty), and its filter is empty, so the kanban's filter leaves out deleted and waiting
  tasks and recurring templates (checked on `task` 3.5.0: it keeps pending tasks, recurring instances and completed
  tasks). The cards show `all`'s columns; pick another report on the page, or set another default, for fewer.

  Its columns:

  | Column      | Filter                                    | On move                                               |
  | ----------- | ----------------------------------------- | ----------------------------------------------------- |
  | To do       | `status:pending progressState:`           | `modify status:pending progressState:`                |
  | In progress | `status:pending progressState:inprogress` | `modify status:pending progressState:inprogress`      |
  | Review      | `status:pending progressState:review`     | `modify status:pending progressState:review` (below)  |
  | Done        | `status:completed`                        | `done`                                                |

  and **Completed since** `today-14d` (below).

  - **Not started and not done is To do**: an empty `progressState` on a pending task.
  - **Done is done whatever its `progressState`.** Real `task` keeps the UDA's value on a completed task (checked:
    `done` on a task with `progressState:review` exports `"progressState":"review"`), so the other columns are limited
    to `status:pending` and Done picks completed tasks by status alone.
  - **Moving out of Done reopens the task.** The other moves set `status:pending`, which on a completed task reopens it
    and clears its `end` (checked on `task` 3.5.0); on a pending task it changes nothing.
  - **No `start`.** Started is Taskwarrior's time tracking (`+ACTIVE`), not a stage: no move starts or stops a task.
  - **Review goes back in the review queue** when tasksh's review is set up (the `reviewed` UDA is defined, see the
    tasksh tab): its On move is then `modify status:pending progressState:review reviewed:`, which clears `reviewed` so the next
    `review` picks the task up (`_reviewed` takes tasks never reviewed). Without the `reviewed` UDA it is
    `modify status:pending progressState:review`. The setup decides once, when it writes the columns; the tab says which it chose.

- **Manual.** No taskrc change and no kanban made; you create kanbans and write their columns yourself: by tags
  (`+next`), by an existing UDA, by `+ACTIVE`, anything a filter can say.

Once enabled, the tab lists the kanbans and the UDA's state (so the automatic lines can still be applied later). Turning
Kanban off hides the pages and keeps the kanbans in the settings; the UDA, once written, stays in the taskrc (the tasks
still hold its values).

### A kanban

There can be any number, each with:

- **Name** and **slug** (its address, `/kanban/<slug>`), made and checked as a board's are.
- **Default report** (required): the report the kanban opens on. It gives what the columns don't:
  - its **filter**, which every column's is added to, as `<filter> export <report>` does;
  - its **sort**, the order of the cards in each column;
  - its **columns** and labels, what a card shows (the report's cells, through the same helper the table uses).
- **Filter** (optional): added to every column, for a kanban of one project (`project:Home`) without a report of its
  own. It is what the page's filter box starts with, and can be changed there while you look.
- **Completed since** (optional): a date (`today-14d`, `sow`, `2026-11-01`; anything `end.after:` takes). Completed
  tasks that ended before it are left out of every column, as `( -COMPLETED or end.after:<date> )`; empty keeps them
  all. It is what the page's **Completed since** box starts with, and can be changed there while you look, as the
  filter can.
- **Columns**, below.

### The columns

An ordered list; the order is the order on the page, left to right. Each column has:

- **Name.** What the column's header says.
- **Filter.** Any filter the Tasks page takes (Tab completes). A task is shown in the **first** column whose filter
  matches it, so columns may overlap and the order settles it (a column `+ACTIVE` before To do takes started tasks out of it).
- **On move.** Any command line, run on a task dropped into the column, written as after a filter: `modify
  progressState:inprogress`, `done`, `annotate moved`, or one of your task commands (`fail`). `;`, `&&` and
  `||` chain several, and the task goes in front of each, as with a task command. Empty means the column takes no
  drops.

Columns can be added, removed, reordered (drag and the arrow buttons) and edited.

**Other.** A task that the report picks but no column does goes in a column of its own, **Other**, last and collapsed
by default (a header with its count; open it to see the cards). It takes no drops, since there is no command for it to
run. With the automatic columns it holds only odd cases, such as a value of `progressState` written before a column
for it existed.

### The pages

- **`/kanbans`**: the list of kanbans, with **New kanban**. A nav entry while Kanban is enabled.
- **`/kanban/<slug>`**: one kanban on its default report; **`/kanban/<slug>/<report>`** on another. The report, the
  filter and Completed since can be changed on the page while it is open: a **report picker** (as on the Tasks page), a
  **filter box** and a **Completed since** box, each starting with the kanban's own. None changes the saved kanban;
  its settings do that.

One column per setting. A card's fields are the report's columns, with their labels, in the report's order: change
the report and the cards change with it. A card can be:

- **dragged** to another column, or moved with its **Move to** menu (and keys), which runs that column's On move on it;
- opened for its detail (`/task/<8 characters>` over the kanban), as anywhere else.

A move asks Taskwarrior's questions (a recurring task, `done` on several) as the Console does. If the task does not
land in the column it was moved to (the command didn't make the filter match), the page says where it went, rather
than showing it where it was dropped.

## Not doing

- Ordering cards by hand inside a column (Taskwarrior has no position; the report's sort decides).
- Kanban settings for what a card shows or the order of cards: that is the report's.
- Swimlanes, WIP limits.
- Kanban as a board section kind. It is its own feature.
- Starting or stopping tasks in the automatic columns (`start` is time tracking, not a stage). A column of your own
  may still run it.
- Any new engine command. Columns are read with `<filter> export <report>`; moves are ordinary command lines.

## Design

Proposal; to agree before building.

- **Settings.** `settings.json` key `kanban`, kept whole like the other sections (a key a front end doesn't know, in a
  kanban or a column, is left as it is). No version bump (a new key needs no migration). The UDA, the report, the
  filter, Completed since and the columns are Kanban's, for every kanban (uroybd, 2026-11-20); a kanban is an `id` (its
  slug) and a `name`, and may have its own `report`, `filter`, `completedSince` or `columns` in their place. A key left
  out is Kanban's; the UDA is Kanban's alone:

  ```json
  "kanban": {
    "enabled": true, "setup": "automatic", "uda": "progressState",
    "report": "all", "filter": "( +PENDING or +COMPLETED )", "completedSince": "today-14d",
    "columns": [{ "name": "In progress", "filter": "status:pending progressState:inprogress",
                  "move": "modify status:pending progressState:inprogress" }],
    "kanbans": [{ "id": "kanban", "name": "Kanban" }, { "id": "work", "name": "Work", "filter": "project:Work" }]
  }
  ```

  A kanban with a bad slug or no report (its own or Kanban's), or a column with an empty name or filter, is a warning,
  shown and kept, as a bad board is; a bad column among Kanban's leaves Kanban with none until it is put right.
- **Reading.** One `( filter box ) ( -COMPLETED or end.after:<completed since> ) ( column filter ) <report>` per
  column, in order (the middle part only when Completed since is set), then the same without a column filter for
  Other (the tasks not already placed) (the
  report's own filter and sort applied by the engine, as the Tasks page's export does); a task already placed in an
  earlier column is dropped from later ones. The UI never decides whether a filter matches (that would re-implement
  layer 1). Card fields come from the report's cells helper (`tc_core::helpers`), so they read as the table's cells do.
- **Moving.** The On move line goes through the task-command expansion (`planChain` / `commandSteps`) with the task's
  uuid as the filter, so a chain and a task command behave as in the Console. Then the columns are read again.
- **Automatic setup.** Port of what the tasksh tab does (`web/src/lib/tasksh.ts`, `Tasksh.kt`, `crates/tui/src/review.rs`):
  the UDA (named on the tab, `progressState` by default) reported defined, differs or missing, and
  `rc.uda.reviewed.type` for the Review column's On move; Apply sends the `config` lines and saves Kanban's settings
  and a first kanban with nothing of its own.
  The engine must read a camelCase UDA name as `task` does (check `progressState:` and `progressState:review` in
  filters, `modify` and `export` with an end-to-end test).
- **Address.** `Place` gains `kanbans` and `kanban` (slug, optional report) and the settings tab `kanban`; round-trip
  tests and rows in `docs/using/pages.md`.
- **Keys.** A `kanban` group in `web/public/keymaps/default.json` (move left, move right, next/previous column, open);
  `:` commands in `commands.json` with `"since": "unreleased"` if any.
- **Per front end.** Web: Svelte pages with drag and drop and a Move to menu, settings tab in a lazy page. Android:
  Compose pages, long-press drag or Move to sheet, the same settings tab. Terminal: columns side by side, `h`/`l` to
  move the selected card.

## Acceptance criteria

- [x] Settings → Kanban offers Automatic and Manual while off; Automatic shows the state of the UDA (`progressState`
      unless named otherwise) and the exact `config` lines, Apply writes only those and sets Kanban's settings (report
      `all`, filter `( +PENDING or +COMPLETED )`, Completed since `today-14d`; To do, In progress, Review, Done) with a
      first kanban of its own name only, the moves out of Done reopening the task, Review clearing `reviewed` when
      tasksh's review is set up, nothing starting a task; Manual changes nothing in the taskrc
- [x] The engine reads, filters, modifies and exports `progressState` as `task` 3.5.0 does (an end-to-end test)
- [x] A task the report picks and no column does is in Other, last and collapsed, which takes no drops
- [x] Kanban's report, filter, Completed since and columns can be set for every kanban, and a kanban can have its own
      of each in their place (the UDA is Kanban's alone); kanbans can be created, renamed, given a slug and deleted;
      columns can be added, removed, reordered and edited; all saved under `kanban` in `settings.json`, unknown keys
      kept
- [x] `/kanban/<slug>` shows each column's tasks within the kanban's filter and default report, a task in the first
      column that matches it, cards in the report's sort showing the report's columns; the report picker
      (`/kanban/<slug>/<report>`) and the filter box (starting with the kanban's filter) change them on the page
      without changing the saved kanban, and so does the Completed since box
- [x] Moving a card runs that column's On move on that task only (a chain and a task command included); a column
      with no On move takes no drops; a task that doesn't land where it was dropped is reported
- [x] Web, Android and terminal all do the above
- [x] Tests: web unit tests (settings shape, the setup's lines and states, column reading, dedupe and Other, the
      filter as sent, the move line for a chain and a task command, route round trips); Kotlin ports of them;
      terminal tests on a scratch `TASKRC`
- [x] Docs: `docs/using/kanban.md` (and `SUMMARY.md`), `docs/using/settings.md`, `docs/using/pages.md` address table,
      README if listed there

## Open questions

None left.

## Implementation notes

**2026-11-20.** Built on all three front ends; not committed.

- **Engine.** Nothing added. `crates/tc-core/tests/cli_exec.rs` `kanban_progress_state_behaves_as_task_does` checks a
  camelCase UDA (filter, `modify`, `export`, values enforced), `done` keeping it, and `modify status:pending` reopening
  a completed task, each checked against `task` 3.5.0 on a scratch taskrc first. The engine's wasm is unchanged, so
  nothing to measure.
- **Departure: a report command, not `export` and the cells helper.** Each column runs `( … ) <report>` as a report
  command; its answer already has the report's columns, labels, sort and cells, so the cards draw those (web
  `ReportCell`, Android the same cells, terminal `model::report`, which is `export <report>` plus the cells helper
  there). Same lines, fewer calls.
- **Departure: the shape of `kanban`** (see Design): the UDA, report, filter, Completed since and columns are Kanban's,
  overridable per kanban except the UDA (uroybd's call during the build). The UDA's name is editable (checked: letters,
  digits, `_`, starting with a letter, not one of Taskwarrior's own attributes). The store resolves each kanban
  (`resolveKanban` / `Settings::resolve`), so the pages draw resolved kanbans. "Add the automatic kanban" went: a new
  kanban starts with Kanban's settings.
- **Web.** `web/src/lib/kanban.ts` (+ tests), `KanbanPage.svelte` (drag and drop and a Move to menu; Other folded),
  `KanbanPanel.svelte` (Automatic/Manual while off; then Kanban's settings and each kanban with "Its own …" boxes);
  `appsettings.ts` reads, lints and merges `kanban`; `route.ts` places `kanbans`, `kanban` (slug, report) and the tab.
- **Android.** `Kanban.kt`, `KanbanPage.kt` (a Move to menu, no drag), `KanbanPanel.kt`, `Store.kt`, `Route.kt`
  (`meghnaad://kanbans`, `meghnaad://kanban/<slug>[/<report>]`), tests in `KanbanTest.kt`, `RouteTest.kt`.
- **Terminal.** `crates/tui/src/kanban.rs` (the model, ported with its tests), `app/kanbans.rs` (page, moves, Settings
  section), `kanbanui.rs`; `settings.rs` saves `kanban` with the three-way merge. Moves run as a key's lines
  (`run_steps`), so `task`'s questions are asked in the box. `tests/kanban.rs` runs the automatic setup and moves a card
  through the columns and out of Done against the real `task` 3.5.0 on a scratch taskrc. The Automatic and Manual
  setups are rows of the section (Enter twice on Automatic), with no key or `:` command of their own (uroybd).
- **Keys.** A `kanban`, `kanban.list` and `settings.kanban` group for the terminal in `default.json`; `:kanban` (with
  words for the page and the section) and `:move` (`left`, `right`) in `commands.json`, `"since": "unreleased"`;
  `next`, `previous`, `back`, `open`, `info`, `filter` and `report` act on a kanban too. Departure: Kanban is not in
  the shared `PAGES` (g1 to g9, `:tabn N`): it would renumber Console and Settings. The terminal shows it unnumbered
  after Boards while it is on (`gk`, `gt`/`gT`, `:kanban`); the web and the phone have it in their menus.
- **Docs.** `docs/using/kanban.md` (+ `SUMMARY.md`), `pages.md`, `settings.md`, `terminal.md`, `architecture.md`,
  README, `docs/commands.md` regenerated.
- **Checks.** `npm run format`, `npm run lint`, `cargo test`, `npm --prefix web test`, `npm run check`,
  `node --test scripts/*.test.mjs`, the two wasm32 checks, `:shared:testDebugUnitTest :androidApp:assembleDebug` and
  `mdbook build docs` all pass. Not yet looked at in a browser or on a phone.
- **Criteria 1 and 4** reworded for Kanban's settings and a kanban's own (uroybd's yes, 2026-11-20).
- **Checked by hand (2026-11-20).** Web, on a scratch Worker and bucket (port 8797), driven by headless Chrome: the
  automatic setup (the UDA written, the Kanban entry in the menu), Move to, dragging to a column, dragging out of Done
  (reopened), the page's filter and report (`/kanban/kanban/list`), a second kanban with its own filter and columns,
  Other opened, and a move that doesn't land reported ("annotate moved ran, but the task is in Other, not Odd"); at
  1400 px and at 390 px dark, with no page-wide sideways scroll and no errors in the console. Android, rendered headless
  from the desktop preview's code (phone and wide): the setup, the settings, the list and the kanban, and a move and a
  reopen through `actSteps` as Move to runs them.
- **Fixed from that.** A new kanban's address now follows its name until the address is typed or the kanban saved
  (`renamedKanban`, web and Android, with tests); it stayed `kanban-2`. On Android the column's Name field was squeezed
  beside its buttons and drew empty: it is now a field of its own under a heading with the column's name.
- **Sizes.** The engine is 907,074 bytes compressed against 906,841 at the last commit (+233, the new `:` commands'
  entries in `keymap`); the Worker is unchanged.
- **Left.** The What section still describes the report, filter and columns as each kanban's: wording to agree.

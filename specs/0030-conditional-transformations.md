# 0030: Conditional transformations

- **Status:** done
- **Created:** 2026-11-24
- **Layer:** 2 (UI), with one change to the shared helper (`tc_core::transform`, `tc_core::cells`)
- **Front ends:** all (web, Android, terminal)
- **Supersedes / related:** 0001 (field transformations)

## Why

A field transformation runs on every task that has the field. Often a rule only makes sense for some tasks: `FUL-123`
is a Jira key in `project:FUL`, a different tracker's key elsewhere, or plain text in a personal project. uroybd wants a
rule to apply only to the tasks a Taskwarrior filter picks.

## What

Each rule in `appearance.transformations` gets an optional `condition`, a Taskwarrior filter:

```json
{ "match": "\\b(FUL-\\d+)\\b", "replace": "[$1](https://jira.example/browse/$1)",
  "fields": ["jira", "description"], "condition": "project:FUL" }
```

- A rule with a condition changes a task's fields only when the task matches it; without one, nothing changes.
- The filter means what it means to `task`, for every status (a completed task matches `project:FUL` too) and whatever
  the context.
- It applies everywhere transformations apply: report tables and board tables, kanban cards, task info and details,
  and the terminal app's `o`.
- The Appearance settings (web, Android) get a **Condition** field under each rule; an empty one is no condition. The
  terminal app reads it from `settings.json`, as it reads the rules.
- A condition `task` refuses (a typo) makes its rule apply to nothing, and the problem is said where the rules' other
  problems are.

## Not doing

- Conditions on progress bars or anything else in `appearance`.
- Evaluating the filter in the page or the app's own code: `task` (or the engine, for the web and the phone) decides.

## Design

Agreed with uroybd ("Go ahead", 2026-11-24).

- **Which tasks match** is asked once per load, per rule with a condition: `rc.verbose=nothing ( <condition> ) _uuids`,
  the command line every front end already sends. Checked on `task` 3.5.0: `_uuids` returns every status (completed
  included) and ignores the context, which is what a condition needs. Rules without a condition cost nothing.
  - web and Android: through the engine, when the tasks are read again (after a change or a sync), beside the report;
  - terminal: through `task`, when it reads a report, a board or a kanban.
- **The shared helper** (`tc_core::transform::Rule`) gains `condition: Option<String>` (saved) and `tasks:
  Option<Vec<String>>` (the matching UUIDs, filled in by the front end, never saved). `Rules::apply` and `Rules::tree`
  take the task's UUID; a rule with a condition runs only for a UUID in its `tasks`. `tc_core::cells` passes each row's
  UUID. The rules sent to the `cells` and Markdown helpers carry `tasks`, so the web app and the phone change nothing
  in how they call them beyond filling it in.
- **Settings:** `condition` is read and written with the rule on all three front ends (web `appsettings.ts`, Android
  `AppSettings.kt`, terminal `settings.rs`), and kept when another front end saves the file.

## Acceptance criteria

- [x] A rule with a condition changes only the matching tasks, in a report, a board table, a kanban card and task info,
      on web, Android and terminal.
- [x] A completed task that matches is transformed in a completed report; the context makes no difference.
- [x] An empty condition is no condition; a refused one applies to nothing and its problem is shown.
- [x] The Condition field edits it on web and Android; `settings.json` keeps it through saves by every front end.
- [x] Tests: `tc_core::transform` (a rule with and without `tasks`), `cells`, each front end's settings round trip, the
      terminal app's end-to-end against the real `task`.
- [x] Docs: `docs/using/settings.md` Transformations.

## Open questions

- None open.

## Implementation notes

- 2026-11-24: `_uuids` checked on `task` 3.5.0 (scratch data): all statuses, context not applied.
- 2026-11-24: the key is `condition`, decided by uroybd: `filter` stays the word for picking a group of tasks (boards,
  kanbans); a condition says whether a rule applies to one.
- 2026-11-24: built. Engine: `Rule.condition`/`tasks`, `Rules::{applies,apply,tree}` take the task's UUID, `cells`
  passes each row's, the `markdown` helper takes a 4th argument (the UUIDs). Terminal: `model::resolve_conditions` on
  each `reload` and when the rules change, errors in `condition_errors` beside `transform_errors`; `info` and `o` pass
  the task's UUID; `tests/conditional_transformations.rs` against the real `task`. Web: `store.rules` (resolved through
  the engine on load and after every write), the Condition field, `trees(…, tasks)`. Android: the same in `Store.rules`,
  `resolveConditions`, the Condition field, `markdownTrees(…, tasks)`.

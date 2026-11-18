# 0015: Share text into the Android app to add a task

- **Status:** draft <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-18
- **Layer:** 2 (UI)
- **Front ends:** Android (web and terminal left out; see **Not doing**)
- **Supersedes / related:** 0016 (share a task out of the Android app), whose JSON flavour this one reads back

## Why

On a phone, a task often starts in another app: a line in a chat, a link in the browser, a note. Today that means copying
it, opening Meghnaad, opening the add box and pasting. Android's share sheet should do it in one step, and should land on
the add screen rather than adding blind, so the person can put the task right before saving it.

A task someone else shared as Taskwarrior JSON (0016, or `task <id> export`) should come in with its fields filled, not
as one long description.

## What

1. **Meghnaad is in the share sheet** for plain text (`text/plain`), as *Add to Meghnaad*.
2. **Plain text** (anything that is not task JSON, below) opens the **quick add** (`QuickAdd.kt`, the same box as the +
   button) with the text already typed in it. The preview card under it shows the task the line will make, as it does
   when typing; the person edits the line, taps *More fields* for the full form, or *Add task*. Nothing is added until
   they do.
   - A share that carries a subject as well as text (`EXTRA_SUBJECT`, a browser sharing a page's title and its URL) is
     typed as `subject text`, so the page's title is the description and its URL follows it. With no subject (most
     apps) the line is the text alone.
   - Line breaks become spaces, as the quick add already does with what is typed.
3. **Task JSON** opens the **full add form** (`TaskEditor.kt`, adding) with its fields filled from the JSON. Task JSON
   is, after trimming, either one JSON object or a JSON array holding exactly one object, as `task <id> export`
   writes it, with a string `description`. Anything else is plain text (2), including JSON that is not a task.
   - Filled from it, and only these (what the add form has a field for): `description`, `project`, `priority`,
     `due`, `wait`, `scheduled`, `until` (Taskwarrior's `20261118T120000Z`, shown in the form in the phone's zone),
     `tags`, `recur`, `depends`, and the UDAs this instance defines.
   - Everything else is ignored: what belongs to the task that was shared (`id`, `uuid`, `entry`, `modified`, `end`,
     `status`, `urgency`, `mask`, `imask`, `parent`, `start`), `annotations`, and UDAs this instance does not define.
   - The new task is always pending, whatever `status` the JSON had.
   - The person adjusts the fields and saves; the form sends `task add …` with what it holds, as it does today.
4. **Not set up yet** (no connection, `Setup` not ready): the app opens as usual and says the shared text could not be
   used until it is set up. Nothing is kept for later.
5. Sharing while the app is already open (`singleTask`, `onNewIntent`) does the same, over whatever is on screen; an add
   box already open is replaced by the new one.

## Not doing

- Adding a task without showing it first (a "share and forget" target).
- Sharing files, images or several items (`ACTION_SEND_MULTIPLE`, `EXTRA_STREAM`): text only.
- A JSON array of more than one task, or `task import` semantics (keeping the uuid, updating a task that exists).
- Appending shared text to an existing task as an annotation.
- The web app as a share target (a Web Share Target needs a web app manifest and a service worker the web app does
  not have) and the terminal app (no share sheet).
- Escaping shared plain text so Taskwarrior syntax in it (`due:`, `+word`, `project:`) is not read: the preview shows
  what it was read as, and the person fixes the line.

## Design

Proposed, not agreed yet.

- **The manifest.** `MainActivity` gets an `intent-filter` for `android.intent.action.SEND` with `text/plain`.
- **The activity.** `openWhatWasTapped` handles `ACTION_SEND`: reads `EXTRA_TEXT` (and `EXTRA_SUBJECT`), clears them, and
  calls `store.addFromShare(subject, text)`.
- **What the text means** is a pure function in `commonMain`, `sharedAdd(subject, text, defs, offsetAt): SharedAdd`,
  returning `Quick(line)` or `Form(seed: FormSeed)`, with its tests in `androidUnitTest`. `Store.addFromShare` then calls
  `quickAdd(line)` or `addWithForm(seed)`, both of which exist.
- **JSON to `FormSeed`.** `kotlinx.serialization`'s `JsonElement`, as the app already reads the engine's answers. Dates
  are read with the date code the app already uses for `export` rows and written with `fromParts(toParts(…))`, as
  `seedOf` does; no new Rust. `depends` uuids go into the form as they are (`task add depends:<uuid>` takes a uuid).
  Nothing goes into `FormSeed.carried`: an attribute with no field is ignored.
- **Nothing in `tc-core`**: no new helper, no engine change, so neither wasm grows.

## Acceptance criteria

- [ ] Meghnaad shows in Android's share sheet for plain text.
- [ ] Sharing plain text opens the quick add with that text in it and its preview; nothing is added until *Add task*.
- [ ] Sharing a subject and text puts `subject text` in the line; text alone puts just the text.
- [ ] Sharing 0016's Task JSON (a bare object) or `task <id> export` output (an array of one) opens the full form with description,
      project, priority, dates, tags, recur, depends and defined UDAs filled, dates in the phone's zone.
- [ ] Only the listed fields reach the form; `uuid`, `id`, `entry`, `status`, `start`, `annotations`, undefined UDAs
      and the rest never reach the `add`; the new task is pending.
- [ ] JSON that is not a task (no `description`, an array of two, a number) opens the quick add as plain text.
- [ ] Sharing with the app open, and with it closed, both work; with no connection set up it says so.
- [ ] Tests: `sharedAdd` in `androidUnitTest`: plain text, subject + text, newlines, an export array of one, a bare
      object, an array of two, non-task JSON, a completed task with annotations and `start`, dates across a DST change,
      a defined and an undefined UDA; 0016's short text read back into the right fields.
- [ ] Docs: `docs/android.md` (sharing into the app); `docs/using/` page for adding tasks if it covers the phone.

## Open questions

- Should the web app do the same later through a Web Share Target, as a separate spec?

Decided (2026-11-18, by the person):

- A share with a subject and text (a browser: the page's title and its URL) is typed as `subject text`.

- Only the fields the add form fills are taken from JSON; annotations, `start` and undefined UDAs are ignored.
- 0016's short text is written in Taskwarrior syntax, so sharing it back in fills the quick add correctly.
- 0016's Task JSON is a bare object; this spec reads both that and `export`'s array of one.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

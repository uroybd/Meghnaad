# 0024: Boards load faster on Android

- **Status:** draft <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19
- **Layer:** TBD (likely 2, UI; possibly the engine's board aggregation on the phone)
- **Front ends:** Android
- **Supersedes / related:** 0023 (the web app's reads); boards (`tc_core::board`)

## Why

A board (dashboard) takes noticeably longer to appear on the Android app than the same board in the web app on the
desktop (reported by the person, 2026-11-19). Not yet measured.

## What

TBD. A board opens about as quickly on the phone as on the desktop, or at least shows each section as soon as its own
numbers are ready.

## Not doing

TBD.

## Design

TBD. To find out first, by measuring rather than guessing:

- the time from opening a board to each section drawn, on the phone and on the desktop, for the same board and data;
- whether the sections' queries (`export` per section, `tc_core::board` aggregation) run one after another or together,
  and on which thread;
- the cost of the JNI round trips and JSON for each section;
- the cost of drawing (Compose recomposition, Koala Plot, the heatmap grid);
- whether the phone's build profile (`android`) or the device makes the engine itself slower.

## Acceptance criteria

TBD.

- [ ] …

## Open questions

- What counts as fast enough: a target in milliseconds for a given board?

## Implementation notes

(none yet)

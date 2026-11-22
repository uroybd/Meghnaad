---
name: smell-review
description: >
  Holistic code-smell review of this repository, Rust first: measure, find the smells (with the pitfalls models fall
  into when they write Rust), verify each against the code and the real `task`, fix them, and commit under the
  `(smells)` scope. Use when uroybd asks for a smell review, asks whether there is room to simplify or abstract, or
  when the release reminder in AGENTS.md says one is due.
---

# Smell review

A review that ends in fixes, not a list. Findings are verified before they are reported; a smell you did not check
against the code is a guess.

## 1. Measure

First, how much has changed since the last review (the last commit with the `smells` scope, else the first commit):

```bash
last=$(git log -1 --format=%H -E --grep='^[a-z]+\(smells\)!?:')
git diff --shortstat "${last:-$(git rev-list --max-parents=0 HEAD)}"..HEAD -- crates web/src app
```

Say the number in the report; at 10,000 lines added or more a review is due (AGENTS.md).

```bash
# biggest files and longest functions (tests cut off)
git ls-files '*.rs' '*.ts' '*.svelte' '*.kt' | grep -v test | xargs wc -l | sort -rn | sed -n 2,25p
# lint escape hatches and leftovers
git grep -n "#\[allow" -- 'crates/*/src/*'; git grep -nE "TODO|FIXME|HACK" -- crates web/src app
# stringly typed errors, string-mapped errors, silent fallbacks
git grep -c 'Result<[^>]*, String>' -- 'crates/*/src/*'
git grep -cE 'map_err\(\|e\| (e\.to_string\(\)|format!)' -- 'crates/*/src/*'
git grep -nE '\.ok\(\)\?|\.ok\(\);|unwrap_or_default\(\)|from_(str|slice|value)\([^)]*\)\.ok\(\)' -- 'crates/*/src/*'
# hidden global state
git grep -nE 'static .*(Mutex|RwLock|OnceLock)|thread_local!' -- 'crates/*/src/*'
# the same in the web app and the phone: errors swallowed, types escaped, nulls forced
git grep -nE 'catch *(\([^)]*\))? *\{ *\}|\.catch\(\(\) *=> *(\{ *\}|undefined|null)\)|as any|: any\b|@ts-(ignore|expect-error)' -- web/src ':!*.test.ts'
git grep -nE 'runCatching|getOrNull\(\)|getOrDefault|catch *\([^)]*\) *\{ *\}|!!' -- 'app/*/src/*Main*'
# the same small rule written twice (classifiers, parsers): twins by name across tc-core and the ports
for n in $(git grep -hoE 'fn [a-z_]+' -- crates/tc-core/src | sed 's/fn //' | sort -u); do
  c=$(echo "$n" | perl -pe 's/_(.)/\U$1/g'); git grep -qE "function $c\b|fun $c\b" -- web/src app && echo "$n"
done
```

Then let clippy look for the pitfalls below with lints the workspace does not turn on (a probe, not a gate; a scratch
target so the user's builds are untouched, and the wasm-only crates left to their own checks):

```bash
CARGO_TARGET_DIR=<scratchpad>/clippy-probe cargo clippy -q --workspace \
  --exclude tw-worker --exclude tc-browser --exclude tc-browser-hooks -- \
  -W clippy::redundant_clone -W clippy::needless_pass_by_value -W clippy::unused_self \
  -W clippy::trivially_copy_pass_by_ref -W clippy::needless_collect -W clippy::redundant_allocation \
  -W clippy::rc_buffer -W clippy::significant_drop_tightening -W clippy::type_complexity -W clippy::too_many_lines
```

(Baseline after the 2026-11-23 review: 38 `too_many_lines` and three known false positives, which are not findings:
`rc_buffer` on `Arc<Vec<Facts>>` in `cli/mod.rs`, which `bring_up_to_date` patches in place through `Arc::make_mut`;
`needless_collect` in `keymap.rs`, where the `collect` ends a borrow of the list being extended; and
`significant_drop_tightening` in `tui/src/task.rs` `turn`, where only the return value follows the guard.) `option_if_let_else` is noise here; leave it out. A lint that stays at zero over two reviews can
move into `[workspace.lints.clippy]`, with its reason.

A long function or a big file is a lead, not a finding: read it before saying it is a smell.

## 2. Rust pitfalls to look for

What models (and people in a hurry) get wrong when they write Rust. Each is a smell only when the code bears it out.
For the reasoning and good/bad examples, read the matching rule of the `rust-skills` skill
(`~/.claude/skills/rust-skills/rules/<id>.md`), named in brackets; where this repo decides otherwise, this file wins.

**Ownership and allocation**
- `.clone()` to quiet the borrow checker, where a borrow, a move or a reordered statement would do. A clone of a
  value never used again after it is a move written the long way. [`own-borrow-over-clone`, `anti-clone-excessive`]
- `String` / `Vec<T>` parameters that are only read (`&str`, `&[T]`); `to_owned()` then `&` back again.
  [`own-slice-over-vec`, `anti-string-for-str`]
- `Arc<Mutex<T>>` (or `Rc<RefCell<T>>`) for state one owner could hold, or that a channel or plain `&mut` would pass.
  [`own-mutex-interior`, `own-rwlock-readers`]

**Abstraction**
- A generic or a trait with one implementation and one caller; trait bounds piled on that the body does not use;
  `impl Trait` / `dyn Trait` / `<T>` chosen by habit. Prefer the concrete type until a second caller needs another.
  [`anti-over-abstraction`, `type-generic-bounds`]
- A wrapper, builder or newtype that only forwards; a helper used once that names nothing the call site did not.
- The opposite, too: the same check written in three places (a uuid rule, a size limit, a request-body parse) that has
  drifted. One function, called from each.

**Errors**
- `Result<_, String>` where a caller has to tell failures apart (a route's status, a retry, "missing" vs "damaged").
  Strings are fine for a message only a person reads. [`anti-stringly-typed`, `type-no-stringly`] `thiserror` is
  already a dependency of `tc-core` and the TUI, so a typed error there costs nothing new; the Worker's own errors are
  hand-written enums, and a new crate in the Worker is weighed against its size.
- `.map_err(|e| e.to_string())` repeated where a `From` impl would let `?` do it. [`err-from-impl`]
- A failure turned into a value: `.ok()`, `unwrap_or_default()`, `let _ =` that makes "damaged" read as "none",
  "unreadable" read as "empty", or caches a failed read as a result. Fall back only where the function says so, and
  never remember a failure as an answer.
- Two functions that answer the same question differently (a "has previous" that says yes where "restore" can't).

**State and control flow**
- Global or thread-local mutable state where an argument would do; when it is a deliberate cache, its comment says why.
- A function over ~150 lines that mixes deciding with drawing or with I/O; nesting a guard clause (`let … else`) would
  flatten. A flat `match` table (`helpers::dispatch`) is not this.
- `bool` flags or string modes that are really an enum. [`type-no-stringly`, `api-newtype-safety`]

**Async**
- A `std::sync::Mutex` guard held across `.await`; blocking I/O in an `async fn`. [`anti-lock-across-await`,
  `async-spawn-blocking`]

The repository's own lints already forbid `unwrap`, `expect`, `panic!`, printing and ignored `#[must_use]`; don't
re-find those.

**Suppressions.** A lint is silenced with `#[expect(lint, reason = "...")]`, not `#[allow]`: when a later change
removes what the lint was about, the compiler says the expectation is unfulfilled, so stale suppressions don't pile up.
`#[allow]` stays only where the lint fires on some targets or features and not others (wasm vs native, `lua`), where
`#[expect]` would warn on the rest; its reason says so. An `#[allow]` without that reason is a finding.

## 3. Not smells here

Check AGENTS.md before calling something a smell; these are by design:

- Layer 2 is ported per front end (web, Kotlin, the TUI), with its tests, except sortspec, boards and keys. Duplication
  between ports is a smell only where a pure rule every UI needs has drifted: then it belongs in `tc_core::helpers`,
  which is uroybd's call (a rule change), so propose it.
- Big `tc-core` files that mirror Taskwarrior's C++ (`filter.rs`, `dates.rs`, `calendar.rs`): splitting them makes
  them harder to check against `task`.
- One `ObjectStore` (or `HookHost`, `Net`) implementation per host.
- Size, for the Worker only: it has Cloudflare's 1 MB limit, so a change that reaches its crates is measured before and
  after (AGENTS.md, "Mind the Worker's size"). The browser engine has no limit: never keep a duplicate or skip an
  abstraction to save its bytes; report its size when it moves.

## 4. Verify, then report

- For behaviour (what a filter word, a uuid or a date means), run the real `task` 3.5.0 on a scratch `TASKRC` and data
  directory, never the user's own.
- Report findings in order: bugs first (behaviour that is wrong), then smells worth fixing (with the fix), then what
  looks like a smell and isn't. Say what you did not check.

## 5. Fix and commit

- Fix with tests: a test that fails before the fix for each bug, and the existing tests green after.
- Run the checks of "Before you say a change is done" in AGENTS.md (format, lint, `cargo test`, the web tests,
  svelte-check, the Kotlin tests with the unstripped host library).
- Commit under the `smells` scope with the honest type, as AGENTS.md says: `fix(smells): …` for a bug the review found,
  `refactor(smells): …` for the rest; a formatting-only change in its own commit. That scope is how the next reminder
  finds this review.

## Where the checklist comes from

Practitioner reports, not studies: Microsoft's Pragmatic Rust Guidelines
(prefer concrete types, then generics, then `dyn`), Google's Rust style guide (no premature generalization),
Effective Rust item 4 (idiomatic error types), and accounts of reviewing large amounts of model-written Rust
(clone and `unwrap` overuse, `Arc<Mutex<T>>` by default, traits and generics for one caller).

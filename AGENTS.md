# Working on Meghnaad

Taskwarrior 3.5.0 in the browser: a Rust → WASM Cloudflare Worker (`crates/worker`) around a pure engine
(`crates/tc-core`), a Svelte 5 app (`web/`), and an R2 bucket as the TaskChampion store. The README and
`docs/architecture.md` explain the design; this file is how to change it.

## Before you say a change is done

Format, lint and test, in that order. CI runs the same checks and fails on any of them.

```bash
npm run format                # cargo fmt (rustfmt.toml, 120 columns) and Prettier (.prettierrc.json): fixes in place
npm run lint                  # Rust: rustfmt check + clippy -D warnings (engine, and the Worker for wasm32)
                              # JS/TS/Svelte: Prettier check + ESLint (eslint.config.js)
cargo test                    # engine, end-to-end command tests, sync cost, memory budgets
npm --prefix web test         # web unit tests
npm run check                 # svelte-check (types, accessibility)
node --test scripts/*.test.mjs  # the build and deploy scripts
cargo check -p tw-worker --target wasm32-unknown-unknown
```

`npm run lint:rust` and `npm run lint:js` run each half alone. Everything is formatted and linted, Rust and web alike:
run `npm run format` after editing any code, not just at the end.

- Clippy is clean; keep it so. Fix a warning rather than allowing it. An `#[allow]` needs a reason (the existing
  `too_many_arguments` ones are for the async command functions that thread the replica and its context).
- ESLint is clean too. Fix a finding rather than disabling it; a `// eslint-disable-next-line` needs a reason after `--`.
  Two Svelte rules are off in `eslint.config.js`, each with the reason beside it (`prefer-svelte-reactivity`,
  `no-unused-svelte-ignore`); don't add to that list to get a change through.
- A Svelte `{#each}` needs a key. Use something unique (an id, a name); for display-only lists whose values can repeat
  (table headers, cells, lines), key by index, because a duplicate key breaks the list.
- A bare `store.x;` inside `$effect` to subscribe to it is written `void store.x;`.
- Don't hand-format around the formatters. Markdown in `docs/` and the README is hand-wrapped and not run through Prettier.
- If a change touches sync, the Worker or crypto, also run `./scripts/interop-local.sh` (needs the `tw-s3` Docker
  container and a built Worker; see the README).

## Conventions

- **Follow Taskwarrior 3.5.0, not a lookalike.** For behaviour (filters, dates, reports, confirmations, `calc`, settings)
  read the C++ first and check against the real `task` run on a scratch `TASKRC` and data directory. Don't guess.
- **Docs move with the code.** A new or changed feature updates `docs/using.md`, and `docs/taskrc-support.md` for a
  setting, `docs/architecture.md` for a module, and the README if it is user-visible. Keep the Mermaid diagram identical
  in the README and `docs/architecture.md`.
- **Taskwarrior logic lives in `tc-core`**; the browser only draws. One endpoint, `POST /api/cli` (and `POST /api/import` for the text of a file; the browser only reads it).
- **Secrets never leave the Worker.** The taskrc importer, `show` and `config` refuse sync and credential-like names and
  never echo their values; keep tests that prove it passing, and add one for any new path that handles settings.
- **Mind the Worker's size.** The compressed Wasm has a budget (the README says how much is left for hooks), and every
  feature spends some. Measure a release build before and after (`gzip -9 -c crates/worker/build/index_bg.wasm | wc -c`),
  and prefer reusing what is already compiled in (an existing `BTreeSet<String>`, `serde_json::from_slice`, the `config`
  line editing) over a new instantiation. `twiggy` on a build made in a scratch directory shows where bytes go. The build
  drops the debug names (`KEEP_WASM_NAMES=1` keeps them); measure with them left out, as the deploy does.
- **Hooks** are user code. Users edit only `crates/tc-core/src/my_hooks.rs`, so keep engine changes in `hooks.rs` and
  keep `my_hooks.rs` stable, so their patch keeps applying (see `docs/using.md`, "Keeping your hooks across updates").

## Running things without disturbing the person using the app

The user may have dev servers running and real tasks in them (`wrangler dev` on 8787, Vite on 5273; 5173 is an
unrelated app). Never kill servers broadly, never write test tasks into their data.

- Test anything that writes on an isolated stack: build, copy `crates/worker/build` and `web/dist` somewhere scratch,
  write a small `wrangler.jsonc` there (no `build` command), and run
  `wrangler dev --local --config <that> --port <unused> --persist-to <scratch>/state`. Stop only your own processes.
- Keep scratch files (copies, screenshots, throwaway scripts) out of the repo.
- Reads against the running server (`count`, `next`) are fine.

## Git

Don't commit or push unless asked. When asked, use a conventional message (`feat:`, `fix:`, `doc:`, `ci:`) that says what
and why, and keep a formatting-only change in its own commit.

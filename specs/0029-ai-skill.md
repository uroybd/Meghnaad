# 0029: Meghnaad for AI agents (a skill, `meghnaad skill`, `meghnaad --dry-run`)

- **Status:** done
- **Created:** 2026-11-24
- **Layer:** 2 (UI)
- **Front ends:** terminal (the web app and the phone have no shell an agent can drive)
- **Supersedes / related:** 0002 (`meghnaad export`), 0025 (task commands)

## Why

The terminal app's command line is already a good surface for an AI agent: `meghnaad export` gives Taskwarrior's JSON
or a compact TSV with chosen fields, and `meghnaad <line>` runs a line on the real `task` with the person's task
commands and `&&` `||` `;` chains. What an agent lacks is the know-how to use it sanely (UUIDs, not working-set IDs;
compact reads; nothing interactive; asking before changing many tasks or deleting) and an easy way to get it.

## What

- A skill, `skills/meghnaad/SKILL.md`, the one source, delivered two ways:
  - as a Claude Code plugin from this repository: `/plugin marketplace add uroybd/Meghnaad`, then
    `/plugin install meghnaad@meghnaad`;
  - printed by the binary, which embeds the file: `meghnaad skill`, for example
    `meghnaad skill > ~/.claude/skills/meghnaad/SKILL.md`, or appended to an `AGENTS.md`.
- `meghnaad --dry-run <line>` (`-n`): prints the statements the line stands for (task commands written out, one per
  line, each after its joiner) and runs nothing, so an agent, or a person, can see what a task command will do first.

## Not doing

- An MCP server, for the terminal app or the web app (decided by uroybd, 2026-11-24): skills only for now; the CLI is
  the interface. For the web app it would need a local wrapper around `scripts/engine-cli.mjs` or WebMCP, never the
  Worker running commands.
- A guard for `task`'s questions without a terminal: the real `task` already answers "no" (below).
- Android AppFunctions (Android 16): see Open questions.

## Design

- `crates/tui/src/main.rs`: `skill` prints `include_str!("../../../skills/meghnaad/SKILL.md")` (as `keys.rs` embeds
  the default keys); `--dry-run`/`-n` prints `run::plan` and exits 0, or 2 for a malformed chain, as a run does.
- `crates/tui/src/run.rs`: `dry_run_lines`, the text printed, tested beside `plan`.
- `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json` at the root; no `version` in them (the one version
  is `package.json`'s; the plugin follows the git ref).

### Checked against the real `task` 3.5.0 (scratch `TASKRC`, `TASKDATA`)

- A question with stdin `/dev/null` or a closed pipe is answered "no": `Task not modified.` / `Task not deleted.`, exit
  1, nothing changed (bulk `modify` of 4 tasks; `delete` of 1).
- A `modify` of up to `rc.bulk` (3) tasks asks nothing, so "ask before changing more than one task" is the skill's
  rule, not `task`'s.
- `rc.verbose=new-uuid add …` prints `Created task <uuid>.`; an 8-character UUID prefix works as a filter.
- `TASKRC override` / `TASKDATA override` go to stderr.

## Acceptance criteria

- [x] `skills/meghnaad/SKILL.md` covers: checking the versions, compact reads, UUIDs, writing, the guardrails, chains
      and `--dry-run`, scratch data for experiments; every command in it run as written on scratch data.
- [x] `meghnaad skill` prints the file byte for byte.
- [x] `meghnaad --dry-run` prints a chain with a task command written out and changes nothing.
- [x] The plugin validates (`claude plugin validate .`).
- [x] Tests: `run.rs` tests for the dry-run text; a test that `skill` is the file.
- [x] Docs: `docs/terminal.md` "With an AI agent"; `--help`; a README line.

## Open questions

- Android AppFunctions: the phone app could declare "add task", "list tasks" functions over the engine's command line
  for an on-device agent. When last checked, only selected agents could call them.
- Should `scripts/version.mjs` write the version into `plugin.json` too, so a plugin update is offered on each release?

## Implementation notes

- 2026-11-24: non-terminal behaviour of `task` checked (above); no guard code added.
- 2026-11-24: built. `skills/meghnaad/SKILL.md`; `meghnaad skill` (`SKILL` in `crates/tui/src/lib.rs`, `include_str!`);
  `meghnaad --dry-run` (`run::dry_run`, `run::dry_run_text`, tested); `.claude-plugin/` (validated; warnings left: no
  `version`, on purpose, and the repo's `CLAUDE.md` at the plugin root). Every command in the skill run on scratch data.
  The skill is bundled only in the terminal app's binary: `scripts/package-release.test.mjs` puts `skills/` and
  `.claude-plugin/` in the source tree and checks the package leaves them out; the web, static, Android and docs builds
  read only their own folders. Docs: `docs/terminal.md` "With an AI agent", README, `--help`. MCP dropped (uroybd).
- 2026-11-24: cold read by a fresh agent with only `SKILL.md`, on scratch data behind wrappers that pin `TASKRC` and
  `TASKDATA`: it listed, added, completed and ran a task command, and stopped to ask before the bulk tag and the
  delete. Its gaps fixed in the skill: `-r next -F table` had no UUID and printed JSON; named dates (`eom + 1mo` is not
  the end of next month, `eonm` is); UTC in exports against local time in `calc` and reports; saying which filter was
  used and asking when it is a guess; `--dry-run` before every task command; `rc.` words per statement; checking the
  scratch `data.location`. Checked on `task` 3.5.0 while fixing: `rc.bulk=0` alone turns the bulk question off (0 means
  never ask), `rc.confirmation=off` alone does not, and a bulk delete needs both.
  Left after a push: installing the plugin from GitHub.

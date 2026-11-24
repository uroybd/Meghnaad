# 0031: Agents in the web app (WebMCP tools and llms.txt)

- **Status:** done
- **Created:** 2026-11-24
- **Layer:** 2 (UI)
- **Front ends:** web (both flavours: Cloudflare and static)
- **Supersedes / related:** 0029 (the skill, for agents with a shell)

## Why

An AI agent in the browser has to read the screen and hunt for buttons. uroybd wants the web app to tell an agent what
it is and give it a direct way in, on every deployment, without a server of our own (the reason MCP was dropped in 0029).

## What

- **WebMCP tools** where the browser has them (`document.modelContext.registerTool`): `export`, `places`, `dry_run`,
  `run`, `open`. Nothing changes in a browser without them.
- **`/llms.txt`**, a public asset: generic (the same file for every deployment, naming nothing of anyone's), with the
  tools, how to work without WebMCP (the console), the rules (UUIDs, asking first), the address patterns, the docs.
- **The Access bypass** for `/llms.txt` on Cloudflare: setup's Access step prints it, the deploy guide has it.

## Not doing

- Any engine change, or a command of our own for agents: the tools send command lines (`export`, the console's own path).
- Answering Taskwarrior's questions from a tool: `run` goes through the console, where the person answers.
- An origin-trial token in the build: it is per origin, so a deployment adds its own (or the flag is used).

## Design

Agreed with uroybd ("go ahead and build", 2026-11-24), `run` may change tasks with the console's questions as the guardrail.

- `web/src/lib/webmcp.ts`: the tools and `registerTools()`, called once the app starts (`App.svelte`). `export` and
  `places` are read-only (`readOnlyHint`, `untrustedContentHint`); `run` is `consequentialHint` and runs the line as
  `runChain` does for a typed line, returning each statement's short result (`declined` where the person said no).
- `web/public/llms.txt`, kept in step by `webmcp.test.ts` (every tool named, every address one `parsePath` opens).
- `scripts/deploy-lib.mjs` `llmsBypass(host)`, printed in setup step 6 whether Access is new or kept.

## Acceptance criteria

- [x] The five tools register where `document.modelContext` exists, and nothing happens where it does not.
- [x] `run` leaves every question to the person; `export` returns Taskwarrior's JSON, trimmed to the fields asked.
- [x] `llms.txt` is generic, describes the tools and the console way without them, and a test keeps it in step.
- [x] Setup and the deploy guide give the Access bypass for `/llms.txt`.
- [x] Docs: `docs/using/console.md` (AI agents), `docs/deploy.md`.

## Open questions

- None open.

## Implementation notes

- 2026-11-24: Chrome's docs (updated 2026-11-20) name `document.modelContext.registerTool`; `navigator.modelContext`
  is tried too, for older previews. Origin trial in Chrome 149; the flag "WebMCP for testing" before that.
- 2026-11-24: static assets are served before the Worker runs, so `/llms.txt` needs only the Access rule, no Worker
  change (the Worker's size is untouched).
- 2026-11-24: not checked in a browser with WebMCP on: the tools are tested with a stand-in `modelContext`.
- 2026-11-24, uroybd's call: the tools are opt-in per device (`web.agents: true` in the device's own file, the switch
  in Settings → This device), and `@mcp-b/global` 6.2.0 (MIT; wraps the W3C polyfill and a native context, bridges to
  MCP-B's extension, so desktop agents reach the tools) replaces `@w3c/webmcp-polyfill`; it is imported only once a
  device turns it on, so a device that never does loads none of it. Turning off aborts the registrations.
- 2026-11-24, with uroybd's yes: `@w3c/webmcp-polyfill` 0.2.0 (W3C, Apache-2.0, no runtime dependencies), imported
  on demand only where there is no native `modelContext` (its chunk: 6.7 KB gzipped; never replaces a native one), so
  an agent running JavaScript in the tab gets the tools in any browser. The app also looks again after `load` and when
  the tab is shown, for a `modelContext` an extension adds later; `index.html` links `llms.txt` (`rel="alternate"`).
  The page's JavaScript grew by about 8 KB gzipped (424 to 432 KB) for the tools, besides the polyfill's chunk.

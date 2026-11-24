// The app's tools for an AI agent in this tab (WebMCP, spec 0031): `document.modelContext.registerTool`, where the browser
// has it (Chrome's origin trial, or its flag), and nothing at all where it does not. Each tool is the UI's own way in: a
// command line through the engine (`runCli`), or the console as a person types it, so a question `task` asks is shown
// there for the person to answer, never answered by the agent. `public/llms.txt` describes them; a test keeps the two in
// step (`webmcp.test.ts`).

import { runCli } from './api';
import { splitWords } from './cmdline';
import { placePath, parsePath } from './route';
import { store } from './store.svelte';
import { planChain } from './taskCommands';
import type { CliResult } from './types';

/** A tool as `registerTool` takes it. */
export interface Tool {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
  annotations?: { readOnlyHint?: boolean; consequentialHint?: boolean; untrustedContentHint?: boolean };
  execute: (input: Record<string, unknown>) => Promise<unknown>;
}

interface ModelContext {
  registerTool: (tool: Tool, options?: { signal?: AbortSignal }) => unknown;
}

const text = (v: unknown) => (typeof v === 'string' ? v : '');

/** What a result says, short: the data is `export`'s to give. */
function brief(r: CliResult | null): Record<string, unknown> {
  if (!r) return { kind: 'none' };
  switch (r.kind) {
    case 'error':
      return { kind: 'error', message: r.message };
    case 'text':
      return { kind: 'text', lines: r.lines };
    case 'changed':
      return { kind: 'changed', message: r.message, tasks: r.tasks };
    case 'json':
      return { kind: 'json', value: r.value };
    case 'confirm':
      return { kind: 'confirm', message: r.message };
    default:
      return { kind: r.kind, note: 'Shown in the app. For the tasks themselves, use the export tool.' };
  }
}

/** The tools, by name. */
export function tools(): Tool[] {
  return [
    {
      name: 'export',
      description:
        "The person's tasks matching a Taskwarrior filter, as Taskwarrior's export JSON (every status; the context is not applied). Name tasks by their uuid afterwards, never by id.",
      inputSchema: {
        type: 'object',
        properties: {
          filter: {
            type: 'string',
            description: 'A Taskwarrior filter, such as "status:pending project:Home +errand".',
          },
          fields: {
            type: 'array',
            items: { type: 'string' },
            description: 'Only these fields of each task (uuid is always kept), such as ["description","due"].',
          },
        },
      },
      annotations: { readOnlyHint: true, untrustedContentHint: true },
      async execute(input) {
        const args = ['rc.verbose=nothing', ...splitWords(text(input.filter)), 'export'];
        const r = (await runCli({ args })).result;
        if (r.kind !== 'json' || !Array.isArray(r.value)) return brief(r);
        const fields = Array.isArray(input.fields) ? input.fields.filter((f) => typeof f === 'string') : [];
        if (!fields.length) return r.value;
        const keep = new Set(['uuid', ...fields]);
        return (r.value as Record<string, unknown>[]).map((t) =>
          Object.fromEntries(Object.entries(t).filter(([k]) => keep.has(k))),
        );
      },
    },
    {
      name: 'places',
      description:
        "The person's own reports, boards and kanbans, and the contexts, each page with the address that opens it (for the open tool).",
      inputSchema: { type: 'object', properties: {} },
      annotations: { readOnlyHint: true, untrustedContentHint: true },
      async execute() {
        return {
          reports: store.reports.map((r) => ({
            name: r.name,
            description: r.description,
            address: placePath({ page: 'tasks', report: r.name }),
          })),
          boards: store.boards.map((b) => ({
            name: b.name,
            description: b.description ?? null,
            address: placePath({ page: 'boards', board: b.id }),
          })),
          kanbans: store.kanbans.map((k) => ({
            name: k.name,
            address: placePath({ page: 'kanban', kanban: k.id }),
          })),
          contexts: Object.keys(store.config?.config.contexts ?? {}),
        };
      },
    },
    {
      name: 'dry_run',
      description:
        "What a command line would run, statement by statement, with the person's task commands written out. Runs nothing.",
      inputSchema: {
        type: 'object',
        properties: { line: { type: 'string', description: 'A command line, as typed after "task".' } },
        required: ['line'],
      },
      annotations: { readOnlyHint: true },
      async execute(input) {
        try {
          return planChain(text(input.line), store.taskCommands);
        } catch (e) {
          return { kind: 'error', message: e instanceof Error ? e.message : String(e) };
        }
      },
    },
    {
      name: 'run',
      description:
        "Run a Taskwarrior command line in the app's console, as the person would type it (task commands and && || ; work). Any question Taskwarrior asks (a change to several tasks, delete, undo) is shown to the person, who answers it; the result says whether it went ahead. Name tasks by uuid.",
      inputSchema: {
        type: 'object',
        properties: { line: { type: 'string', description: 'A command line, as typed after "task".' } },
        required: ['line'],
      },
      annotations: { consequentialHint: true },
      async execute(input) {
        const line = text(input.line).trim();
        if (!line) return { kind: 'error', message: 'No command line.' };
        if (store.chainBusy) return { kind: 'error', message: 'The console is busy with another line.' };
        let steps;
        try {
          steps = planChain(line, store.taskCommands);
        } catch (e) {
          return { kind: 'error', message: e instanceof Error ? e.message : String(e) };
        }
        if (!steps.length) return { kind: 'error', message: 'No command line.' };
        store.view = 'console';
        const before = new Set(store.entries.map((e) => e.id));
        await store.runChain(steps, line);
        return store.entries
          .filter((e) => !before.has(e.id))
          .map((e) => ({
            command: e.title,
            ...brief(e.result),
            ...(e.declined ? { declined: true } : {}),
            ...(e.failure ? { failure: e.failure } : {}),
          }));
      },
    },
    {
      name: 'open',
      description:
        'Show the person a page of the app by its address: one from the places tool, /task/<first 8 characters of a uuid>, or a page such as /summary or /calendar.',
      inputSchema: {
        type: 'object',
        properties: { address: { type: 'string', description: 'Such as /report/next or /task/1a2b3c4d.' } },
        required: ['address'],
      },
      async execute(input) {
        const address = text(input.address);
        if (!parsePath(address)) return { kind: 'error', message: `No page at ${address}.` };
        const at = await store.openFromAddress(address);
        return { opened: at ?? address };
      },
    },
  ];
}

/** The tools registered now, and how to take them away. */
let offered: AbortController | null = null;

/**
 * Register the tools on the `modelContext` there is now; false when there is none. Aborting `signal` (or
 * [`disableAgents`]) takes them away.
 */
export function offer(signal?: AbortSignal): boolean {
  if (offered) return true;
  const holder =
    (document as unknown as { modelContext?: ModelContext }).modelContext ??
    (navigator as unknown as { modelContext?: ModelContext }).modelContext;
  if (typeof holder?.registerTool !== 'function') return false;
  const own = new AbortController();
  signal?.addEventListener('abort', () => own.abort(), { once: true });
  offered = own;
  for (const tool of tools()) {
    try {
      void Promise.resolve(holder.registerTool(tool, { signal: own.signal })).catch(() => undefined);
    } catch {
      // One tool the browser refuses leaves the others.
    }
  }
  return true;
}

/**
 * Offer the tools to agents, once this device has turned it on (Settings, This device). Loads MCP-B's WebMCP runtime
 * (`@mcp-b/global`) only then: it wraps the browser's own `document.modelContext` or installs the W3C polyfill where there
 * is none, and bridges the tools to MCP-B's extension, through which desktop agents reach them too.
 */
export async function enableAgents(): Promise<boolean> {
  if (offered) return true;
  try {
    await import('@mcp-b/global');
  } catch {
    // Not a secure context, or the file did not load: the browser's own WebMCP, if any, is all there is.
  }
  return offer();
}

/** Take the tools away (this device turned it off). The runtime stays loaded until the page is reloaded. */
export function disableAgents(): void {
  offered?.abort();
  offered = null;
}

/** For tests: forget that the tools were registered. */
export function resetOffered(): void {
  offered = null;
}

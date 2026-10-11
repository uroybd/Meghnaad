import { engine, engineAvailable } from '../engine/client.svelte';
import type { CliResponse, ConfigResponse, ImportReport, TaskrcResponse } from './types';

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

/** The browser's UTC offset in seconds east of UTC, so "today"/"tomorrow" mean the user's own. */
export function tzOffsetSeconds(d = new Date()): number {
  return -d.getTimezoneOffset() * 60;
}

/** Commands that change something: a request that carries one is never repeated on its own. */
const WRITES = /\b(add|modify|done|delete|start|stop|annotate|denotate|append|prepend|undo|sync)\b/i;

/** Could this request be sent a second time without doing anything twice? */
function safeToRepeat(init?: RequestInit): boolean {
  const method = (init?.method ?? 'GET').toUpperCase();
  if (method === 'GET') return true;
  if (method !== 'POST' || typeof init?.body !== 'string') return false;
  try {
    const b = JSON.parse(init.body) as { line?: string; args?: string[] } | null;
    // Only a command line is known to be repeatable; any other body (a file to import) is not.
    if (!b || Array.isArray(b) || typeof b !== 'object' || (b.line === undefined && b.args === undefined)) return false;
    return !WRITES.test(b.line ?? (b.args ?? []).join(' '));
  } catch {
    return false;
  }
}

/**
 * Cloudflare answers 1102 ("Worker exceeded resource limits") when a request uses more CPU than the
 * plan allows. The first request to an idle Worker does the most (it derives the key), and the next
 * finds it warm, so one more try is usually all it takes.
 */
const RESOURCE_LIMIT = /\b1102\b|exceeded resource limits/i;

async function request<T>(path: string, init?: RequestInit, retried = false): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, init);
  } catch {
    throw new ApiError('Could not reach the server.', 0);
  }
  if (res.status === 401 || res.status === 403) {
    // Cloudflare Access redirects/denies when the session has expired.
    throw new ApiError('Your session may have expired. Reload the page to sign in again.', res.status);
  }
  const text = await res.text();
  let body: unknown;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    if (!res.ok && !retried && RESOURCE_LIMIT.test(text) && safeToRepeat(init)) {
      await new Promise((r) => setTimeout(r, 400));
      return request<T>(path, init, true);
    }
    const why = RESOURCE_LIMIT.test(text)
      ? 'The Worker ran out of CPU time on this request (Cloudflare error 1102). Try again; if it keeps happening, see docs/deploy.md (Troubleshooting a deployment).'
      : `Server error (${res.status}).`;
    throw new ApiError(res.ok ? 'The server sent an unreadable response.' : why, res.status);
  }
  if (!res.ok) {
    const msg = (body as { error?: string } | null)?.error ?? `Server error (${res.status}).`;
    throw new ApiError(msg, res.status);
  }
  return body as T;
}

/** Answers already given to the engine's questions about one command. */
export interface Answers {
  /** Yes to the plain questions (undo, a command with no filter). */
  confirmed?: boolean;
  /** The tasks to go ahead with, when asked which ones (Taskwarrior's yes/no/all/quit, as ticks). */
  approved?: string[];
  /** The follow-up questions answered yes (repair a dependency chain, change a recurring series). */
  extras?: string[];
}

export interface CliInput extends Answers {
  /** Exactly one of these: a typed command line, or pre-split args (no quoting concerns). */
  line?: string;
  args?: string[];
}

// The engine (Taskwarrior's logic and the tasks) runs in this page, in a Web Worker (src/engine); the Worker on the
// server keeps the secret and the bucket and only passes versions through. Where the browser cannot run it (no Web
// Workers or WebAssembly, `?engine=server`, or it failed to start) the server runs the engine, as it used to, through
// `POST /api/cli` and `POST /api/import`.

/** Resolves to whether the engine in this page is in use; set by the first `getConfig`. */
let booted: Promise<boolean> | undefined;

async function bootEngine(config: ConfigResponse): Promise<boolean> {
  if (!engineAvailable()) return false;
  try {
    await engine.start(JSON.stringify(config.config));
    return true;
  } catch (e) {
    console.warn('The engine could not start in this browser; the server will run it.', e);
    engine.stop();
    return false;
  }
}

async function useEngine(): Promise<boolean> {
  // Where the browser cannot run it there is nothing to start, and nothing to ask the server first.
  if (!engineAvailable()) return false;
  if (booted === undefined) await getConfig();
  return (await booted) ?? false;
}

/** What the engine answered, as the server's `/api/cli` and `/api/import` do: an `error` is a refused request. */
function answer<T>(text: string): T {
  const body = JSON.parse(text) as T & { error?: string };
  if (typeof body.error === 'string') throw new ApiError(body.error, 400);
  return body;
}

export async function runCli(input: CliInput): Promise<CliResponse> {
  if (!(await useEngine())) {
    return request<CliResponse>('/api/cli', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ ...input, tz: tzOffsetSeconds() }),
    });
  }
  const res = answer<CliResponse & { config?: unknown }>(
    await engine.run({ ...input, tz: tzOffsetSeconds(), now: Date.now() / 1000 }),
  );
  // `config` changed the settings: they are saved on the server, where they live, and the engine takes them up.
  const { config, ...response } = res;
  if (config) {
    try {
      await request('/api/config/engine', {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(config),
      });
      await engine.setConfig(JSON.stringify(config));
    } catch {
      return {
        ...response,
        result: { kind: 'error', message: 'The settings could not be saved. Nothing was changed.' },
      };
    }
  }
  if (response.wrote) engine.pushSoon();
  return response;
}

/** The most the server takes in one import (it says so in the same words). */
export const MAX_IMPORT_BYTES = 3 * 1024 * 1024;

/**
 * Import a file of tasks (what `export` writes). Without `apply` it only reports what would happen; with it the tasks
 * are written, all of them or none.
 */
export async function importTasks(text: string, apply: boolean): Promise<ImportReport> {
  if (!(await useEngine())) {
    return request<ImportReport>(`/api/import?apply=${apply ? 1 : 0}&tz=${tzOffsetSeconds()}`, {
      method: 'POST',
      headers: { 'content-type': 'text/plain; charset=utf-8' },
      body: text,
    });
  }
  const report = answer<ImportReport>(await engine.import(text, apply, tzOffsetSeconds()));
  if (apply) engine.pushSoon();
  return report;
}

export async function getConfig(): Promise<ConfigResponse> {
  const config = await request<ConfigResponse>('/api/config');
  if (booted === undefined) booted = bootEngine(config);
  else if (await booted) void engine.setConfig(JSON.stringify(config.config)).catch(() => {});
  return config;
}

/** The saved settings, rendered as a taskrc to edit and import again. */
export async function getTaskrcText(): Promise<string> {
  let res: Response;
  try {
    res = await fetch('/api/config/taskrc');
  } catch {
    throw new ApiError('Could not reach the server.', 0);
  }
  if (!res.ok) throw new ApiError(`Server error (${res.status}).`, res.status);
  return res.text();
}

/** Go back to the settings from before the last import. */
export function restoreTaskrc(): Promise<TaskrcResponse> {
  return request<TaskrcResponse>('/api/config/taskrc/restore', { method: 'POST' });
}

export function putTaskrc(text: string): Promise<TaskrcResponse> {
  return request<TaskrcResponse>('/api/config/taskrc', {
    method: 'PUT',
    headers: { 'content-type': 'text/plain' },
    body: text,
  });
}

/** Fetch a taskrc from a link (the Worker does the fetch) and save it as a pasted one is. */
export function importTaskrcUrl(url: string): Promise<TaskrcResponse> {
  return request<TaskrcResponse>('/api/config/taskrc/url', {
    method: 'POST',
    headers: { 'content-type': 'text/plain' },
    body: url,
  });
}

/** The app's own `settings.json`, as saved (`{}` before the first save). */
export function getAppSettings(): Promise<unknown> {
  return request<unknown>('/api/settings');
}

/** Save `settings.json`; the Worker keeps the text as it is, once it is a JSON object of a sensible size. */
export function putAppSettings(text: string): Promise<{ ok: true }> {
  return request<{ ok: true }>('/api/settings', {
    method: 'PUT',
    headers: { 'content-type': 'application/json; charset=utf-8' },
    body: text,
  });
}

/** Save the urgency settings: every one that differs from the built-in value, and `urgency.inherit`. */
export function putUrgency(urgency: Record<string, number>, inherit: boolean): Promise<{ ok: true }> {
  return request<{ ok: true }>('/api/config/urgency', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ urgency, inherit }),
  });
}

export interface SetupStatus {
  configured: boolean;
  /** Names of the Worker settings still to fill in (TEAM_DOMAIN, POLICY_AUD). */
  missing: string[];
  /** The hostname this page is served from, for the Access application. */
  host: string;
}

/** Whether the Worker has its sign-in settings. Needs no sign-in itself; null if it can't be told. */
export async function getSetup(): Promise<SetupStatus | null> {
  try {
    const res = await fetch('/api/setup');
    if (!res.ok) return null;
    return (await res.json()) as SetupStatus;
  } catch {
    return null;
  }
}

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

/** Could this request be sent a second time without doing anything twice? Only a read is. */
function safeToRepeat(init?: RequestInit): boolean {
  return (init?.method ?? 'GET').toUpperCase() === 'GET';
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
// server keeps the secret and the bucket and only passes versions through. Commands and imports go to the engine, and
// the settings (`getConfig` and the rest) to the server, where they are kept.

/** Resolves when the engine is running, or rejects with why it could not. Set by the first `getConfig`. */
let booted: Promise<void> | undefined;

async function bootEngine(config: ConfigResponse): Promise<void> {
  if (!engineAvailable()) {
    throw new ApiError('This browser cannot run the engine: it needs Web Workers and WebAssembly.', 0);
  }
  try {
    await engine.start(JSON.stringify(config.config));
    await refreshHooks();
  } catch (e) {
    engine.stop();
    throw new ApiError(`The engine could not start in this browser: ${e instanceof Error ? e.message : String(e)}`, 0);
  }
}

/**
 * Tell the engine which hooks the saved script defines (it only asks the Worker to run those). A failure leaves it as it
 * was: with no hooks known, none run.
 */
export async function refreshHooks(): Promise<void> {
  try {
    const { defined } = await request<{ defined: string[] }>('/api/hooks/defined');
    await engine.setHooks(defined);
  } catch {
    // The hooks are an extra: the engine works without them.
  }
}

/** The engine, started (it needs the settings first). */
async function ready(): Promise<void> {
  if (booted === undefined) await getConfig();
  await booted;
}

/** What the engine answered, as JSON text: an `error` is a refused request. */
function answer<T>(text: string): T {
  const body = JSON.parse(text) as T & { error?: string };
  if (typeof body.error === 'string') throw new ApiError(body.error, 400);
  return body;
}

export async function runCli(input: CliInput): Promise<CliResponse> {
  await ready();
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

/** The most the page takes in one import. */
export const MAX_IMPORT_BYTES = 3 * 1024 * 1024;

/**
 * Import a file of tasks (what `export` writes). Without `apply` it only reports what would happen; with it the tasks
 * are written, all of them or none.
 */
export async function importTasks(text: string, apply: boolean): Promise<ImportReport> {
  await ready();
  const report = answer<ImportReport>(await engine.import(text, apply, tzOffsetSeconds()));
  if (apply) engine.pushSoon();
  return report;
}

export async function getConfig(): Promise<ConfigResponse> {
  const config = await request<ConfigResponse>('/api/config');
  if (booted === undefined) {
    booted = bootEngine(config);
    // The reason is given to whoever runs a command; here it only has to not go unnoticed by the runtime.
    booted.catch(() => {});
  } else {
    // Read again (a setting changed): the engine takes the new ones, if it is running.
    await booted.then(() => engine.setConfig(JSON.stringify(config.config))).catch(() => {});
  }
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

/** The hooks script as saved (the editor asks; nothing else does, since the script may hold secrets). */
export async function getHooks(): Promise<string> {
  return (await request<{ source: string }>('/api/hooks')).source;
}

/** What is wrong with a script, and where: `line` is a line of the script, when Lua says. */
export type HookCheck = { ok: true; defined: string[] } | { ok: false; line: number | null; message: string };

/** Is this script all right? Saves nothing. */
export function checkHooks(text: string): Promise<HookCheck> {
  return request<HookCheck>('/api/hooks/check', {
    method: 'POST',
    headers: { 'content-type': 'text/plain; charset=utf-8' },
    body: text,
  });
}

/** Keep the script (an empty one removes the hooks), and tell the engine which hooks it defines. */
export async function putHooks(text: string): Promise<string[]> {
  const res = await request<{ ok: true; defined: string[] }>('/api/hooks', {
    method: 'PUT',
    headers: { 'content-type': 'text/plain; charset=utf-8' },
    body: text,
  });
  await refreshHooks();
  return res.defined;
}

/** The names of the secrets kept from the app for hooks (never their values). */
export async function getSecretNames(): Promise<string[]> {
  return (await request<{ names: string[] }>('/api/secrets')).names;
}

/** Keep a secret for hooks, sealed in the bucket. It can be replaced, never read back; a script asks for it by name. */
export async function putSecret(name: string, value: string): Promise<void> {
  await request<{ ok: true }>(`/api/secrets/${encodeURIComponent(name)}`, {
    method: 'PUT',
    headers: { 'content-type': 'text/plain; charset=utf-8' },
    body: value,
  });
}

export async function deleteSecret(name: string): Promise<void> {
  await request<{ ok: true }>(`/api/secrets/${encodeURIComponent(name)}`, { method: 'DELETE' });
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

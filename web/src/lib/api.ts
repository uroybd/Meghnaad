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

export function runCli(input: CliInput): Promise<CliResponse> {
  return request<CliResponse>('/api/cli', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ ...input, tz: tzOffsetSeconds() }),
  });
}

/** The most the server takes in one import (it says so in the same words). */
export const MAX_IMPORT_BYTES = 3 * 1024 * 1024;

/**
 * Send a file of tasks (what `export` writes) to be imported. Without `apply` it only reports what would
 * happen; with it the tasks are written, all of them or none.
 */
export function importTasks(text: string, apply: boolean): Promise<ImportReport> {
  return request<ImportReport>(`/api/import?apply=${apply ? 1 : 0}&tz=${tzOffsetSeconds()}`, {
    method: 'POST',
    headers: { 'content-type': 'text/plain; charset=utf-8' },
    body: text,
  });
}

export function getConfig(): Promise<ConfigResponse> {
  return request<ConfigResponse>('/api/config');
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

import type { CliResponse, ConfigResponse, TaskrcResponse } from './types';

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
  }
}

/** The browser's UTC offset in seconds east of UTC, so "today"/"tomorrow" mean the user's own. */
export function tzOffsetSeconds(d = new Date()): number {
  return -d.getTimezoneOffset() * 60;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
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
    throw new ApiError(res.ok ? 'The server sent an unreadable response.' : `Server error (${res.status}).`, res.status);
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

// The Android app: the engine is native (Rust, in the app's process) and the settings live on the phone, so there is no
// Web Worker and no server. The page talks to the app's plugin instead, in the same words: a `NativePort` answers the
// engine's messages (protocol.ts), and requests the page would send to its server (`/api/config`, …) are answered by the
// phone through `installNativeApi`.

import { Capacitor, registerPlugin } from '@capacitor/core';
import type { Call, EngineReply } from './protocol';

/** What the app's plugin (android/…/TcEnginePlugin.kt) offers. */
export interface TcEnginePlugin {
  /** Start with these settings (JSON text). Whether a copy of the tasks was found on the phone. */
  start(o: { config: string }): Promise<{ restored: boolean }>;
  /** `config`, `run`, `import` or `sync`; the engine's own JSON text (`sync`: `{ changed, settings }`: whether the tasks, and whether the shared settings or hooks, were replaced from the bucket). */
  send(o: { op: 'config' | 'run' | 'import' | 'sync'; arg?: string }): Promise<{ value: string }>;
  /** A request to one of the settings routes. */
  api(o: { method: string; path: string; body: string }): Promise<{ status: number; type: string; body: string }>;
  forget(): Promise<void>;
  /** Whether the phone is connected. Nothing about the bucket is given back: not its address, not a key. */
  getConnection(): Promise<Connection>;
  /** Use the phone without a bucket: tasks stay here until a connection is made. */
  useLocally(): Promise<void>;
  /** What a widget was tapped for, if anything is waiting: a task, or a report with a filter. Given once. */
  getLaunch(): Promise<Launch>;
  /** Called when a widget is tapped while the app is open (then ask what for). */
  addListener(event: 'launch', fn: () => void): Promise<{ remove: () => Promise<void> }>;
  /** Which of the settings and the hooks are kept in step with the bucket's. */
  getSyncOptions(): Promise<SyncOptions>;
  setSyncOptions(o: SyncOptions): Promise<void>;
  setConnection(o: ConnectionInput): Promise<void>;
  clearConnection(): Promise<void>;
}

export interface Connection {
  configured: boolean;
  /** The phone was told to keep the tasks here only. */
  localOnly: boolean;
}

/** What a widget asked the app to open. */
export interface Launch {
  task: string | null;
  report: string | null;
  /** The filter, as one string of words (`( due:2026-10-17 or scheduled:2026-10-17 )`). */
  filter: string | null;
}

/** What the phone keeps in step with the bucket besides the tasks (the secrets of hooks never are). */
export interface SyncOptions {
  settings: boolean;
  hooks: boolean;
}

export interface ConnectionInput {
  endpoint: string;
  bucket: string;
  region: string;
  accessKey: string;
  secretKey: string;
  encryptionSecret: string;
}

/** Is this page running inside the Android app? */
export function isNative(): boolean {
  return Capacitor.isNativePlatform();
}

export const plugin = registerPlugin<TcEnginePlugin>('TcEngine');

/** The part of a Web Worker the engine client uses. */
export interface Port {
  postMessage(m: Call): void;
  onmessage: ((e: MessageEvent<EngineReply>) => void) | null;
  onerror: ((e: { message?: string }) => void) | null;
  terminate(): void;
}

/** The engine's messages, answered by the app one at a time in the order they arrive, as the Web Worker does. */
export class NativePort implements Port {
  onmessage: ((e: MessageEvent<EngineReply>) => void) | null = null;
  onerror: ((e: { message?: string }) => void) | null = null;
  /** Called when a sync replaced the settings or the hooks here with the bucket's, so the page reads them again. */
  onsettings: (() => void) | null = null;
  private queue: Promise<unknown> = Promise.resolve();

  constructor(private readonly api: TcEnginePlugin = plugin) {}

  postMessage(m: Call): void {
    this.queue = this.queue
      .then(() => this.handle(m))
      .then(
        (value) => this.onmessage?.({ data: { id: m.id, ok: true, value } } as MessageEvent<EngineReply>),
        (err: unknown) => {
          const error = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
          this.onmessage?.({ data: { id: m.id, ok: false, error } } as MessageEvent<EngineReply>);
        },
      );
  }

  terminate(): void {}

  private async handle(m: Call): Promise<unknown> {
    switch (m.op) {
      case 'start':
        return (await this.api.start({ config: m.config })).restored;
      case 'config':
        await this.api.send({ op: 'config', arg: m.config });
        return null;
      case 'hooks':
        // The phone reads its own script: nothing to say.
        return null;
      case 'run':
        return (await this.api.send({ op: 'run', arg: m.request })).value;
      case 'import':
        return (
          await this.api.send({
            op: 'import',
            arg: JSON.stringify({ text: m.text, apply: m.apply, tz: m.tz, now: m.now }),
          })
        ).value;
      case 'sync': {
        const done = JSON.parse((await this.api.send({ op: 'sync' })).value) as { changed: boolean; settings: boolean };
        if (done.settings) this.onsettings?.();
        return done.changed;
      }
      case 'forget':
        await this.api.forget();
        return null;
    }
  }
}

/**
 * Answer the page's requests to its server from the phone: `/api/setup` says whether the phone is set up, and every
 * other `/api/…` goes to the app, which keeps the settings in a folder. Other addresses are fetched as usual.
 */
export function installNativeApi(api: TcEnginePlugin = plugin, win: Pick<Window, 'fetch' | 'location'> = window): void {
  const real = win.fetch.bind(win);
  win.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const request = input instanceof Request ? input : undefined;
    const raw = typeof input === 'string' ? input : input instanceof URL ? input.href : (request?.url ?? '');
    const url = new URL(raw, win.location.href);
    if (url.origin !== win.location.origin || !url.pathname.startsWith('/api/')) return real(input, init);
    const method = (init?.method ?? request?.method ?? 'GET').toUpperCase();
    if (method === 'GET' && url.pathname === '/api/setup') {
      // Set up means connected, or told to go without: either way the app can start.
      const { configured, localOnly } = await api.getConnection();
      return Response.json({ configured: configured || localOnly, missing: [], host: '' });
    }
    const body = typeof init?.body === 'string' ? init.body : request ? await request.text() : '';
    const answer = await api.api({ method, path: url.pathname + url.search, body });
    return new Response(answer.body, { status: answer.status, headers: { 'content-type': answer.type } });
  };
}

/**
 * Open what a widget was tapped for: the one that started the app, and any tapped while it is open. Returns a function that
 * stops listening.
 */
export async function watchLaunch(
  open: (what: { task?: string; report?: string; filter?: string }) => void,
  api: TcEnginePlugin = plugin,
): Promise<() => void> {
  const take = async () => {
    const got = await api.getLaunch().catch(() => null);
    if (got?.task) open({ task: got.task });
    else if (got?.report) open({ report: got.report, filter: got.filter ?? '' });
  };
  await take();
  const handle = await api.addListener('launch', () => void take());
  return () => void handle.remove();
}

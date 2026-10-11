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
  /** `config`, `run`, `import` or `sync`; the engine's own JSON text (`sync`: whether tasks changed, as JSON). */
  send(o: { op: 'config' | 'run' | 'import' | 'sync'; arg?: string }): Promise<{ value: string }>;
  /** A request to one of the settings routes. */
  api(o: { method: string; path: string; body: string }): Promise<{ status: number; type: string; body: string }>;
  forget(): Promise<void>;
  /** Whether the phone is connected and to what; never the secrets. */
  getConnection(): Promise<Connection>;
  setConnection(o: ConnectionInput): Promise<void>;
  clearConnection(): Promise<void>;
}

export interface Connection {
  configured: boolean;
  endpoint?: string;
  bucket?: string;
  region?: string;
  accessKey?: string;
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
      case 'sync':
        return JSON.parse((await this.api.send({ op: 'sync' })).value) as boolean;
      case 'forget':
        await this.api.forget();
        return null;
    }
  }
}

/**
 * Answer the page's requests to its server from the phone: `/api/setup` says whether the phone is connected, and every
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
      const { configured } = await api.getConnection();
      return Response.json({ configured, missing: [], host: '' });
    }
    const body = typeof init?.body === 'string' ? init.body : request ? await request.text() : '';
    const answer = await api.api({ method, path: url.pathname + url.search, body });
    return new Response(answer.body, { status: answer.status, headers: { 'content-type': answer.type } });
  };
}

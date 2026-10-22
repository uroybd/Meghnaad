// The page's side of the engine: starts the Web Worker, sends it commands, and keeps it in step with the bucket.
//
// Syncing: before the first command if there was no saved replica to start from; then in the background after every
// write (to push it), when the tab is looked at again, every so often while it is, and when another tab of this
// browser has pushed (they tell each other over a BroadcastChannel). A write is already applied and saved here, so a
// push that fails (offline, a signed-out session) is only tried again later; what is waiting is pushed with it.

import type { Call, EngineReply, EngineRequest, Port } from './protocol';

/** How often a visible tab pulls what other devices have done. */
const PULL_EVERY_MS = 30_000;
/** A pull that comes this soon after the last one is not worth a request. */
const PULL_NOT_SOONER_MS = 4_000;
const CHANNEL = 'meghnaad-engine';
export type SyncStatus = 'idle' | 'syncing' | 'failed';

/** Whether this browser can run the engine: it needs Web Workers and WebAssembly. */
export function engineAvailable(): boolean {
  return typeof Worker !== 'undefined' && typeof WebAssembly !== 'undefined';
}

export class EngineClient {
  /** What the sync is doing, for the page to show. */
  status = $state<SyncStatus>('idle');
  /** Why the last sync failed, until one succeeds. */
  error = $state<string | null>(null);
  /** When the last sync succeeded (ms since the epoch). */
  lastSync = $state<number | null>(null);

  private worker: Port | undefined;
  private next = 1;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
  private starting: Promise<void> | undefined;
  /** The Worker, started: whether it found a saved replica. */
  private spawned: Promise<boolean> | undefined;
  private syncing: Promise<void> | undefined;
  private again = false;
  /** The next sync follows a write here, so the other tabs are told when it is done. */
  private announce = false;
  private lastPull = 0;
  private channel: BroadcastChannel | undefined;
  private timer: ReturnType<typeof setInterval> | undefined;
  private changedListeners = new Set<() => void>();

  /** Called when a sync brought in changes (from another device or tab), so views can reload. */
  onChanged(fn: () => void): () => void {
    this.changedListeners.add(fn);
    return () => this.changedListeners.delete(fn);
  }

  private send(m: EngineRequest): Promise<unknown> {
    const worker = this.worker;
    if (!worker) return Promise.reject(new Error('the engine has not started'));
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      worker.postMessage({ ...m, id } satisfies Call);
    });
  }

  /**
   * Load the engine's Worker now, before the settings are known: its WebAssembly is fetched and compiled, and the saved
   * replica read, while the page asks the server for the rest. Harmless to call again, or where there is no Worker.
   */
  preload(): void {
    if (engineAvailable()) this.spawn().catch(() => undefined);
  }

  /** Start the engine with these settings (JSON text), once; later calls wait for the first. */
  start(config: string): Promise<void> {
    this.starting ??= this.boot(config);
    return this.starting;
  }

  private spawn(): Promise<boolean> {
    this.spawned ??= (() => {
      performance.mark('engine-boot');
      const worker = new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' }) as Port;
      this.worker = worker;
      worker.onmessage = (e: MessageEvent<EngineReply>) => {
        const p = this.pending.get(e.data.id);
        if (!p) return;
        this.pending.delete(e.data.id);
        if (e.data.ok) p.resolve(e.data.value);
        else p.reject(new Error(e.data.error));
      };
      worker.onerror = (e) => {
        const err = new Error(e.message || 'the engine stopped');
        for (const p of this.pending.values()) p.reject(err);
        this.pending.clear();
      };
      return this.send({ op: 'start', persist: true }) as Promise<boolean>;
    })();
    return this.spawned;
  }

  private async boot(config: string): Promise<void> {
    const restored = await this.spawn();
    await this.send({ op: 'config', config });
    // With nothing saved the tasks have to come from the bucket before the first command is answered.
    if (restored) void this.sync();
    else await this.sync(true);
    // How long it took to be ready to answer, and from what (shown in the browser's performance tools).
    performance.measure(
      restored ? 'engine-ready (from the saved copy)' : 'engine-ready (from the bucket)',
      'engine-boot',
    );
    this.watch();
  }

  /** Pull and push now (or after the one under way). Resolves once it is done; rejects with why it failed, if told. */
  sync(strict = false): Promise<void> {
    if (this.syncing) {
      this.again = true;
      return strict ? this.syncing : this.syncing.catch(() => {});
    }
    const run = async () => {
      this.status = 'syncing';
      try {
        const changed = (await this.send({ op: 'sync' })) as boolean;
        this.error = null;
        this.status = 'idle';
        this.lastSync = Date.now();
        this.lastPull = this.lastSync;
        if (changed) for (const fn of this.changedListeners) fn();
        // Only what this tab wrote is news to the others (told after every sync, they would answer each other for ever).
        if (this.announce) this.channel?.postMessage('pushed');
        this.announce = false;
      } catch (e) {
        // TaskChampion's own prefix adds nothing to "could not reach the server".
        this.error = (e instanceof Error ? e.message : String(e)).replace(/^Server Error: /, '');
        this.status = 'failed';
        console.warn('Sync failed:', this.error);
        throw e;
      }
    };
    this.syncing = run().finally(() => {
      this.syncing = undefined;
      if (this.again) {
        this.again = false;
        void this.sync();
      }
    });
    return strict ? this.syncing : this.syncing.catch(() => {});
  }

  /** After a write: push it, without making the caller wait. */
  pushSoon(): void {
    this.announce = true;
    void this.sync();
  }

  private pullIfStale = () => {
    if (document.visibilityState === 'visible' && Date.now() - this.lastPull > PULL_NOT_SOONER_MS) void this.sync();
  };

  private watch(): void {
    document.addEventListener('visibilitychange', this.pullIfStale);
    window.addEventListener('online', this.pullIfStale);
    this.timer = setInterval(() => {
      if (document.visibilityState === 'visible') void this.sync();
    }, PULL_EVERY_MS);
    if (typeof BroadcastChannel !== 'undefined') {
      this.channel = new BroadcastChannel(CHANNEL);
      // Another tab pushed: what it did is in the bucket, and it is not in this tab until it is pulled.
      this.channel.onmessage = () => void this.sync();
    }
  }

  setHooks(defined: string[]): Promise<unknown> {
    return this.send({ op: 'hooks', defined: JSON.stringify(defined) });
  }

  setConfig(json: string): Promise<unknown> {
    return this.send({ op: 'config', config: json });
  }

  run(request: object): Promise<string> {
    return this.send({ op: 'run', request: JSON.stringify(request) }) as Promise<string>;
  }

  import(text: string, apply: boolean, tz: number): Promise<string> {
    return this.send({ op: 'import', text, apply, tz, now: Date.now() / 1000 }) as Promise<string>;
  }

  /** Throw away the copy kept in this browser; the page reloads from the bucket. */
  async forget(): Promise<void> {
    await this.send({ op: 'forget' });
  }

  stop(): void {
    document.removeEventListener('visibilitychange', this.pullIfStale);
    window.removeEventListener('online', this.pullIfStale);
    clearInterval(this.timer);
    this.channel?.close();
    this.worker?.terminate();
    this.worker = undefined;
    this.starting = undefined;
    this.spawned = undefined;
  }
}

/** The one engine of this page. */
export const engine = new EngineClient();

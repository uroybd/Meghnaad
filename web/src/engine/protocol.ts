// What the page and the engine's Web Worker say to each other.

export type EngineRequest =
  /** Start up: load the engine, and whether to keep the replica in IndexedDB. Answers whether a saved replica was found.
   * The settings come after, as `config`: they are not needed to load the engine or the saved copy. */
  | { op: 'start'; persist: boolean }
  | { op: 'config'; config: string }
  /** Which hooks the Worker's script defines, as JSON (`["on_add"]`): only these are ever called. */
  | { op: 'hooks'; defined: string }
  | { op: 'run'; request: string }
  | { op: 'import'; text: string; apply: boolean; tz: number; now: number }
  /** Pull and push. Answers a `Synced`. */
  | { op: 'sync' }
  /** Forget the saved replica (it is fetched again from the bucket on the next start). */
  | { op: 'forget' };

/** A sync's answer: whether anything of the tasks changed, and why the copy in this browser could not be saved
 * (`null` when it was, or is not kept here). The push can work while the save fails. */
export interface Synced {
  changed: boolean;
  unkept: string | null;
}

export type EngineReply = { id: number; ok: true; value: unknown } | { id: number; ok: false; error: string };

export type Call = EngineRequest & { id: number };

/** The part of a Web Worker the engine client uses. */
export interface Port {
  postMessage(m: Call): void;
  onmessage: ((e: MessageEvent<EngineReply>) => void) | null;
  onerror: ((e: { message?: string }) => void) | null;
  terminate(): void;
}

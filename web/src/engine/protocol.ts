// What the page and the engine's Web Worker say to each other.

export type EngineRequest =
  /** Start up: the settings, and whether to keep the replica in IndexedDB. Answers whether a saved replica was found. */
  | { op: 'start'; config: string; persist: boolean }
  | { op: 'config'; config: string }
  /** Which hooks the Worker's script defines, as JSON (`["on_add"]`): only these are ever called. */
  | { op: 'hooks'; defined: string }
  | { op: 'run'; request: string }
  | { op: 'import'; text: string; apply: boolean; tz: number; now: number }
  /** Pull and push. Answers whether anything of the tasks changed. */
  | { op: 'sync' }
  /** Forget the saved replica (it is fetched again from the bucket on the next start). */
  | { op: 'forget' };

export type EngineReply = { id: number; ok: true; value: unknown } | { id: number; ok: false; error: string };

export type Call = EngineRequest & { id: number };

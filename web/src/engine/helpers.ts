// The engine's own helpers, called from the page's thread: the parts of `tc-core` that take nothing from the replica, so
// they need no worker and no round trip (`tc_core::helpers`). The page writes dates in the user's `dateformat` this way
// instead of keeping a formatter of its own, so what it shows is what the engine means.
//
// They are the same WebAssembly module as the engine's, loaded again here (the browser has the bytes already: it is one
// download) and used at once, which a call to the worker, being asynchronous, could not be. They are used while the page is
// built, so main.ts loads them before it imports the app.

import init, { format_date, helper, initSync } from 'tc-engine';
import wasmUrl from './pkg/tc_browser_bg.wasm?url';

let loading: Promise<void> | undefined;
let loaded = false;

/** Load the helpers, once; later calls wait for the first. */
export function initHelpers(): Promise<void> {
  loading ??= init({ module_or_path: wasmUrl }).then(() => {
    loaded = true;
  });
  return loading;
}

/** The same from bytes in hand (the tests, which have no network and no page to fetch from). */
export function initHelpersFrom(bytes: BufferSource): void {
  initSync({ module: bytes });
  loaded = true;
}

/** `epoch` (seconds since 1970) in a `dateformat` pattern, in the zone `tz` seconds east of UTC. */
export function formatDate(pattern: string, epoch: number, tz: number, weekStartsMonday: boolean): string {
  if (!loaded) throw new Error('the engine helpers have not loaded');
  return format_date(pattern, epoch, tz, weekStartsMonday);
}

/** Any other helper of `tc_core::helpers` by name, with its arguments: its answer, or an error when it is not a helper. */
export function callHelper<T>(name: string, ...args: unknown[]): T {
  if (!loaded) throw new Error('the engine helpers have not loaded');
  const out = JSON.parse(helper(name, JSON.stringify(args))) as T | { error: string };
  if (out && typeof out === 'object' && 'error' in out) throw new Error(`helper ${name}: ${out.error}`);
  return out as T;
}

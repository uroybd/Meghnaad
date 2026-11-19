import { untrack } from 'svelte';

/**
 * How many requests and commands the page is waiting on, for the thin bar under the top bar. Background work (syncing,
 * which has its own chip) is not counted.
 */
export const waiting = $state({ count: 0 });

/**
 * `p`, counted while it is on its way. The count is changed untracked: a request made inside an `$effect` would
 * otherwise make the effect depend on it, and run again (and ask again) when the count goes down.
 */
export function waitFor<T>(p: Promise<T>): Promise<T> {
  untrack(() => waiting.count++);
  return p.finally(() => untrack(() => waiting.count--));
}

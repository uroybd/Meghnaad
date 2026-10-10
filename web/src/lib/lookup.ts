import { runCli } from './api';
import { store } from './store.svelte';
import type { Row } from './types';

// What was fetched, kept until the tasks change, so moving along a row of ids asks once each.
const cache = new Map<string, Promise<Row | null>>();
let cachedRev = -1;

/** One task by uuid, or `null` when it is gone or the request failed. Asked for only when someone wants to see it. */
export function fetchTask(uuid: string): Promise<Row | null> {
  if (cachedRev !== store.rev) {
    cache.clear();
    cachedRev = store.rev;
  }
  let p = cache.get(uuid);
  if (!p) {
    p = runCli({ args: [uuid, 'info'] })
      .then(({ result }) => (result.kind === 'info' ? (result.tasks[0] ?? null) : null))
      .catch(() => null);
    cache.set(uuid, p);
  }
  return p;
}

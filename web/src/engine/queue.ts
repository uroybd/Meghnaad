// The engine's Web Worker answers one message at a time, as the engine cannot be entered twice. A sync is the one
// exception, split so that its slow part, asking the store what is new, waits for nothing and holds nothing up.

/** Runs each job after the one before has ended (either way), in the order they were given. */
export type Serial = <T>(job: () => T | Promise<T>) => Promise<T>;

export function serial(): Serial {
  let tail: Promise<unknown> = Promise.resolve();
  return <T>(job: () => T | Promise<T>): Promise<T> => {
    const next = tail.then(job);
    tail = next.catch(() => undefined);
    return next;
  };
}

/**
 * A sync in three steps: where to sync from, taken in turn (a moment); the fetch, outside the queue, so that commands
 * sent meanwhile are answered; then the sync itself, in turn, applying what was fetched and pushing what is waiting.
 */
export async function splitSync<S, F, R>(
  inTurn: Serial,
  source: () => S,
  fetch: (source: S) => Promise<F>,
  apply: (fetched: F) => Promise<R>,
): Promise<R> {
  const from = await inTurn(source);
  const fetched = await fetch(from);
  return inTurn(() => apply(fetched));
}

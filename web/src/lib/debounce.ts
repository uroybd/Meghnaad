/**
 * `fn`, called `ms` after the last call, with that call's arguments: typing calls it at every key, and it runs once
 * the typing stops. `flush()` runs a waiting call now (before Apply, so nothing typed is lost); `cancel()` drops it.
 */
export interface Debounced<A extends unknown[]> {
  (...args: A): void;
  flush(): void;
  cancel(): void;
}

export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms: number): Debounced<A> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let waiting: A | undefined;
  const run = () => {
    timer = undefined;
    const args = waiting;
    waiting = undefined;
    if (args) fn(...args);
  };
  const call = ((...args: A) => {
    waiting = args;
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(run, ms);
  }) as Debounced<A>;
  call.flush = () => {
    if (timer !== undefined) clearTimeout(timer);
    run();
  };
  call.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    waiting = undefined;
  };
  return call;
}

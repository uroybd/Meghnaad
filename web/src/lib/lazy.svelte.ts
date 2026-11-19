// A component whose code is fetched when first wanted rather than with the app (spec 0023, step 4). The app's main
// bundle used to carry every page and every settings tab, though a visit opens one or two of them.

import type { Component } from 'svelte';

/* eslint-disable @typescript-eslint/no-explicit-any -- a component of any props: the page that draws it knows them */
export interface Lazy {
  /** The component, once its code is here. */
  readonly component: Component<any> | undefined;
  /** Its code could not be fetched (offline, or the app was updated and the old file is gone). */
  readonly failed: boolean;
  /** Fetch its code, once; a failed fetch is tried again on the next call. */
  load(): Promise<void>;
}

export function lazy(importer: () => Promise<{ default: Component<any> }>): Lazy {
  /* eslint-enable @typescript-eslint/no-explicit-any */
  let s = $state.raw<{ component: Lazy['component']; failed: boolean }>({ component: undefined, failed: false });
  let loading: Promise<void> | undefined;
  return {
    get component() {
      return s.component;
    },
    get failed() {
      return s.failed;
    },
    load() {
      loading ??= importer().then(
        (m) => {
          s = { component: m.default, failed: false };
        },
        () => {
          loading = undefined;
          s = { component: undefined, failed: true };
        },
      );
      return loading;
    },
  };
}

/** Fetch these once the page has nothing else to do, so opening them later doesn't wait. */
export function loadWhenIdle(all: Lazy[]): void {
  const go = () => {
    for (const l of all) void l.load();
  };
  if (typeof window === 'undefined') return;
  if ('requestIdleCallback' in window) window.requestIdleCallback(go, { timeout: 5000 });
  else setTimeout(go, 3000);
}

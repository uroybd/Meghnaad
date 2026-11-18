import './app.css';
import { mount, unmount } from 'svelte';
import { engine } from './engine/client.svelte';
import { STATIC } from './engine/flavour';
import { initHelpers } from './engine/helpers';
import { prefetch } from './lib/api';

// The engine's helpers (dates in the user's format, quoting and sorting a filter) are used while the page is built, so they
// load first: it is the engine's WebAssembly, which the engine needs a moment later and finds in the browser's cache. The
// app is imported after, because its modules use them as they are made.
const target = document.getElementById('app')!;

function failed(e: unknown) {
  target.textContent = `The app could not start in this browser: ${e instanceof Error ? e.message : String(e)}`;
}

async function startApp() {
  const { default: App } = await import('./App.svelte');
  mount(App, { target });
}

if (STATIC) {
  // No server to ask: the device is connected or unlocked first (lib/StaticGate.svelte), and only then does the engine
  // start, with the branch and the key. `config.json` beside the page (from the deploy template) fills in the form.
  initHelpers()
    .then(async () => {
      const defaults = (await fetch('config.json', { cache: 'no-store' })
        .then((r) => (r.ok ? r.json() : {}))
        .catch(() => ({}))) as { repo?: string; branch?: string; api?: string };
      const [{ default: Gate }, { onLock }, locked] = await Promise.all([
        import('./lib/StaticGate.svelte'),
        import('./engine/session'),
        import('./lib/lockedReminders'),
      ]);
      // A tab that locks keeps its reminder list for the locked page; one that forgets the device keeps nothing.
      onLock((forgetting) => (forgetting ? locked.dropList() : locked.keepForLock()));
      const gate = mount(Gate, {
        target,
        props: {
          defaults,
          onunlocked: () => {
            locked.dropList();
            void unmount(gate);
            engine.preload();
            prefetch();
            startApp().catch(failed);
          },
        },
      });
    })
    .catch(failed);
} else {
  // What starting needs is on its way before the app's code has been fetched: the engine's Worker (its WebAssembly and
  // the saved copy of the tasks), and the answers the server gives the page at the start.
  engine.preload();
  prefetch();
  initHelpers().then(startApp).catch(failed);
}

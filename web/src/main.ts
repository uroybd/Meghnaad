import './app.css';
import { mount } from 'svelte';
import { engine } from './engine/client.svelte';
import { initHelpers } from './engine/helpers';
import { prefetch } from './lib/api';

// The engine's helpers (dates in the user's format, quoting and sorting a filter) are used while the page is built, so they
// load first: it is the engine's WebAssembly, which the engine needs a moment later and finds in the browser's cache. The
// app is imported after, because its modules use them as they are made.
const target = document.getElementById('app')!;
// What starting needs is on its way before the app's code has been fetched: the engine's Worker (its WebAssembly and the
// saved copy of the tasks), and the answers the server gives the page at the start.
engine.preload();
prefetch();
initHelpers()
  .then(async () => {
    const { default: App } = await import('./App.svelte');
    mount(App, { target });
  })
  .catch((e: unknown) => {
    target.textContent = `The app could not start in this browser: ${e instanceof Error ? e.message : String(e)}`;
  });

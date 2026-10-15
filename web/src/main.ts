import './app.css';
import { mount } from 'svelte';
import { initHelpers } from './engine/helpers';

// The engine's helpers (dates in the user's format, quoting and sorting a filter) are used while the page is built, so they
// load first: it is the engine's WebAssembly, which the engine needs a moment later and finds in the browser's cache. The
// app is imported after, because its modules use them as they are made.
const target = document.getElementById('app')!;
initHelpers()
  .then(async () => {
    const { default: App } = await import('./App.svelte');
    mount(App, { target });
  })
  .catch((e: unknown) => {
    target.textContent = `The app could not start in this browser: ${e instanceof Error ? e.message : String(e)}`;
  });

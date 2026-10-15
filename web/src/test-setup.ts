/// <reference types="node" />
// The engine runs in a Web Worker, which these tests (jsdom) do not have. They stand in for it with one that gives its
// answers by calling `fetch`, as the Worker's endpoints used to, so a test describes what a command answers by stubbing
// `fetch` (the URLs `/api/cli` and `/api/import` below exist only in these tests) and the components and the store run as they do in the page. (`api.engine.test.ts`
// and `engine/client.test.ts` test the real client's side with their own stand-ins.)
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { vi } from 'vitest';
import { initHelpersFrom } from './engine/helpers';

// The engine's helpers (dates in the user's format) are Rust, called through the engine's WebAssembly: the tests load the
// built one, as the page does. `npm --prefix web test` builds it first when it is out of date (`npm run build:engine`).
try {
  initHelpersFrom(readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'engine', 'pkg', 'tc_browser_bg.wasm')));
} catch (e) {
  throw new Error('The engine is not built, and the tests need its helpers: run `npm run build:engine`', { cause: e });
}

vi.mock('./engine/client.svelte', () => ({
  engineAvailable: () => true,
  engine: {
    status: 'idle',
    error: null,
    lastSync: null,
    start: async () => undefined,
    stop: () => undefined,
    setConfig: async () => null,
    setHooks: async () => null,
    pushSoon: () => undefined,
    sync: async () => undefined,
    forget: async () => undefined,
    onChanged: () => () => undefined,
    run: async (request: object) =>
      (
        await fetch('/api/cli', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(request),
        })
      ).text(),
    import: async (text: string, apply: boolean, tz: number) =>
      (
        await fetch(`/api/import?apply=${apply ? 1 : 0}&tz=${tz}`, {
          method: 'POST',
          headers: { 'content-type': 'text/plain; charset=utf-8' },
          body: text,
        })
      ).text(),
  },
}));

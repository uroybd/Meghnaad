// @vitest-environment node
// The engine's generated JavaScript glue loads its WebAssembly with the global `fetch` (and `Response`, `WebAssembly`,
// …). An export of ours with one of those names would shadow it inside the glue, and the engine would not start: the
// tests load the WebAssembly from bytes, past that code, so only this catches it.
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const pkg = join(dirname(fileURLToPath(import.meta.url)), 'pkg');
// The browser's globals the glue may use, beside whatever this runtime has.
const BROWSER = ['fetch', 'Response', 'Request', 'URL', 'WebAssembly', 'TextEncoder', 'TextDecoder', 'self', 'window'];

describe("the engine's exports", () => {
  for (const glue of [join(pkg, 'tc_browser.js'), join(pkg, 'static', 'tc_browser.js')]) {
    it.skipIf(!existsSync(glue))(`shadow no global the glue uses (${glue.slice(pkg.length + 1)})`, () => {
      const source = readFileSync(glue, 'utf8');
      const names = [...source.matchAll(/^export (?:async )?(?:function|class|const|let) (\w+)/gm)].map((m) => m[1]);
      expect(names.length).toBeGreaterThan(0);
      const shadowing = names.filter((n) => BROWSER.includes(n) || n in globalThis);
      expect(shadowing).toEqual([]);
    });
  }
});

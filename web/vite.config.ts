import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// The release this build is: the Release workflow says (APP_VERSION), otherwise the version in the root package.json.
const version =
  process.env.APP_VERSION ??
  (JSON.parse(readFileSync(fileURLToPath(new URL('../package.json', import.meta.url)), 'utf8')).version as string);

export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  // Tests mount components, which needs Svelte's browser build rather than its server one.
  resolve: {
    // The engine, built by `node scripts/build-engine.mjs` (typed by src/engine/tc-engine.d.ts).
    alias: { 'tc-engine': fileURLToPath(new URL('./src/engine/pkg/tc_browser.js', import.meta.url)) },
    ...(process.env.VITEST ? { conditions: ['browser'] } : {}),
  },
  server: {
    // `npm run dev` here, `npm run dev` (wrangler) at the repo root for the API.
    proxy: { '/api': process.env.API_URL ?? 'http://127.0.0.1:8787' },
  },
  build: { outDir: 'dist', sourcemap: false, target: 'es2022' },
  // The engine's Web Worker is a module (it imports the engine package).
  worker: { format: 'es' },
  test: { environment: 'jsdom', include: ['src/**/*.test.ts'], setupFiles: ['./src/test-setup.ts'] },
});

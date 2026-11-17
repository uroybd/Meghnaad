import { copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Plugin } from 'vite';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// The release this build is: the Release workflow says (APP_VERSION), otherwise the version in the root package.json.
const version =
  process.env.APP_VERSION ??
  (JSON.parse(readFileSync(fileURLToPath(new URL('../package.json', import.meta.url)), 'utf8')).version as string);

/** `static` builds the app with no Worker: tasks and settings on a git branch, unlocked in the page (spec 0013). */
const flavour = process.env.VITE_FLAVOUR ?? 'worker';
const pkg = (file: string) => fileURLToPath(new URL(`./src/engine/pkg/${file}`, import.meta.url));
const engine = (file: string) => pkg(flavour === 'static' ? `static/${file}` : file);

/**
 * The static site's policy: its own scripts only, and requests to any https address (the forge's API is the person's to
 * name, and so are the services their hooks call), or to this machine (a forge run locally to try it).
 */
const STATIC_CSP = [
  "default-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "worker-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data:",
  "connect-src 'self' https: http://localhost:* http://127.0.0.1:*",
  "manifest-src 'self'",
  "base-uri 'self'",
  "form-action 'self'",
  "object-src 'none'",
].join('; ');

/**
 * The static site, ready for any static host: the policy in the page (GitHub Pages sends no headers of a site's own) and
 * in `_headers` (Cloudflare Pages and Netlify read it), and `404.html` as a copy of the page, so a deep link served as a
 * "not found" still opens the app at that address.
 */
function staticSite(): Plugin {
  return {
    name: 'meghnaad-static-site',
    apply: 'build',
    transformIndexHtml: (html) =>
      html.replace(
        '<meta charset="UTF-8" />',
        `<meta charset="UTF-8" />\n    <meta http-equiv="Content-Security-Policy" content="${STATIC_CSP}" />`,
      ),
    writeBundle(options) {
      const dir = options.dir ?? 'dist-static';
      copyFileSync(join(dir, 'index.html'), join(dir, '404.html'));
      writeFileSync(
        join(dir, '_headers'),
        [
          '# The static site: the policy is also in the page, for hosts that send no headers of their own.',
          '/*',
          '  X-Content-Type-Options: nosniff',
          '  Referrer-Policy: no-referrer',
          '  X-Frame-Options: DENY',
          `  Content-Security-Policy: ${STATIC_CSP}; frame-ancestors 'none'`,
          '',
        ].join('\n'),
      );
    },
  };
}

export default defineConfig({
  // A static site may be served from a folder (GitHub Pages: /<repo>/), which only its deploy knows: its files are
  // addressed relative to the page's <base>, which the deploy sets.
  base: flavour === 'static' ? './' : '/',
  plugins: [svelte(), ...(flavour === 'static' ? [staticSite()] : [])],
  define: { __APP_VERSION__: JSON.stringify(version) },
  // Tests mount components, which needs Svelte's browser build rather than its server one.
  resolve: {
    // The engine, built by `node scripts/build-engine.mjs` (typed by src/engine/tc-engine.d.ts): for the static flavour
    // the one with the git branch (`--static`, in pkg/static), and its hooks module (pkg/hooks).
    alias: [
      { find: /^tc-engine$/, replacement: engine('tc_browser.js') },
      { find: /^tc-engine-wasm(\?.*)?$/, replacement: `${engine('tc_browser_bg.wasm')}$1` },
      ...(flavour === 'static'
        ? [
            { find: /^tc-hooks$/, replacement: pkg('hooks/tc_browser_hooks.js') },
            { find: /^tc-hooks-wasm(\?.*)?$/, replacement: `${pkg('hooks/tc_browser_hooks_bg.wasm')}$1` },
          ]
        : [
            // The Worker runs the hooks: nothing to bundle.
            { find: /^tc-hooks$/, replacement: fileURLToPath(new URL('./src/engine/hooks-stub.ts', import.meta.url)) },
            { find: /^tc-hooks-wasm\?url$/, replacement: 'data:application/wasm,' },
          ]),
    ],
    ...(process.env.VITEST ? { conditions: ['browser'] } : {}),
  },
  server: {
    // `npm run dev` here, `npm run dev` (wrangler) at the repo root for the API.
    proxy: { '/api': process.env.API_URL ?? 'http://127.0.0.1:8787' },
  },
  build: { outDir: flavour === 'static' ? 'dist-static' : 'dist', sourcemap: false, target: 'es2022' },
  // The engine's Web Worker is a module (it imports the engine package).
  worker: { format: 'es' },
  test: { environment: 'jsdom', include: ['src/**/*.test.ts'], setupFiles: ['./src/test-setup.ts'] },
});

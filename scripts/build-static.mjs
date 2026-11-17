#!/usr/bin/env node
// Builds the static web app (spec 0013): the engine with the git branch and the hooks module (`build-engine.mjs
// --static`), and the page with VITE_FLAVOUR=static, into web/dist-static. That folder is the whole site: any static host
// serves it, and the deploy template (templates/static) downloads it from a release (`meghnaad-static-<version>.tar.gz`).
// No Worker, no wrangler, nothing to set up on a server.
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { ROOT, run } from './deploy-lib.mjs';

export const STATIC_DIST = join(ROOT, 'web', 'dist-static');

/** The release asset's name for a version (the template's fetch-release.mjs asks for the same). */
export const staticAsset = (version) => `meghnaad-static-${version}.tar.gz`;

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const must = (r, what) => {
    if (r.status !== 0) {
      console.error(`[static] ${what} failed`);
      process.exit(r.status ?? 1);
    }
  };
  console.log('[static] engine (with the git branch) and the hooks module');
  must(run('node', ['scripts/build-engine.mjs', '--static', '--if-stale']), 'the engine build');
  console.log('[static] the page');
  must(
    run('npm', ['--prefix', 'web', 'run', 'build'], { env: { ...process.env, VITE_FLAVOUR: 'static' } }),
    'the web build',
  );
  console.log(`[static] built ${STATIC_DIST}`);
}

import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { assemble, releaseConfig, releasePackage } from './package-release.mjs';
import { ROOT, buildConfig, stripJsonc } from './deploy-lib.mjs';

const source = () => JSON.parse(stripJsonc(readFileSync(join(ROOT, 'wrangler.jsonc'), 'utf8')));

test('the release config has no build step and points at the files as packaged', () => {
  const c = releaseConfig(source());
  assert.equal(c.build, undefined);
  assert.equal(c.main, 'worker/index.js');
  assert.equal(c.assets.directory, 'app');
  assert.deepEqual(c.assets.run_worker_first, ['/api/*']);
  assert.equal(c.assets.not_found_handling, 'single-page-application');
  // Everything else is the source config's: the bucket binding, the date, keep_vars.
  assert.deepEqual(c.r2_buckets, source().r2_buckets);
  assert.equal(c.compatibility_date, source().compatibility_date);
  assert.equal(c.keep_vars, true);
  // The deploy overlay (bucket, domain, Access) still applies to it.
  const deployed = buildConfig(c, { R2_BUCKET: 'mine', CUSTOM_DOMAIN: 'tasks.example.com' });
  assert.equal(deployed.r2_buckets[0].bucket_name, 'mine');
  assert.equal(deployed.routes[0].pattern, 'tasks.example.com');
});

test('the release package.json deploys with the scripts and carries what the Deploy button shows', () => {
  const root = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
  const p = releasePackage(root, '1.2.3');
  assert.equal(p.version, '1.2.3');
  assert.deepEqual(Object.keys(p.scripts), ['setup', 'deploy']);
  assert.equal(p.devDependencies.wrangler, root.devDependencies.wrangler);
  assert.ok(p.cloudflare.bindings.TC_ENCRYPTION_SECRET.description);
  assert.equal(p.devDependencies.eslint, undefined);
});

test('assembling copies the built Worker and app, the scripts and the configuration, and nothing from the sources', () => {
  const root = mkdtempSync(join(tmpdir(), 'pkg-root-'));
  const out = join(mkdtempSync(join(tmpdir(), 'pkg-out-')), 'release');
  try {
    mkdirSync(join(root, 'crates', 'worker', 'build', 'worker'), { recursive: true });
    mkdirSync(join(root, 'web', 'dist', 'assets'), { recursive: true });
    mkdirSync(join(root, 'scripts'));
    for (const [f, text] of [
      ['crates/worker/build/index.js', 'js'],
      ['crates/worker/build/index_bg.wasm', 'wasm'],
      ['crates/worker/build/package.json', '{}'],
      ['crates/worker/build/.gitignore', '*'],
      ['crates/worker/build/worker/shim.mjs', 'shim'],
      ['web/dist/index.html', '<html>'],
      ['web/dist/assets/app.js', 'app'],
      ['web/dist/_headers', '/*'],
      ['LICENSE', 'MIT'],
      ['wrangler.jsonc', readFileSync(join(ROOT, 'wrangler.jsonc'), 'utf8')],
      ['package.json', readFileSync(join(ROOT, 'package.json'), 'utf8')],
      ['scripts/setup.mjs', 's'],
      ['scripts/deploy.mjs', 'd'],
      ['scripts/deploy-lib.mjs', 'l'],
      ['scripts/dev.mjs', 'not packaged'],
    ])
      writeFileSync(join(root, f), text);

    assemble(root, out, '1.2.3');

    const has = (f) => existsSync(join(out, f));
    for (const f of ['worker/index.js', 'worker/index_bg.wasm', 'app/index.html', 'app/assets/app.js', 'app/_headers'])
      assert.ok(has(f), f);
    for (const f of ['scripts/setup.mjs', 'scripts/deploy.mjs', 'scripts/deploy-lib.mjs', 'LICENSE', 'README.md'])
      assert.ok(has(f), f);
    assert.ok(!has('scripts/dev.mjs'));
    assert.ok(!has('worker/.gitignore'), 'a .gitignore of * would hide the whole Worker from git');
    assert.equal(readFileSync(join(out, '.dev.vars.example'), 'utf8'), 'TC_ENCRYPTION_SECRET=\n');
    assert.equal(JSON.parse(readFileSync(join(out, 'wrangler.jsonc'), 'utf8')).main, 'worker/index.js');
    assert.deepEqual(readdirSync(out).sort(), [
      '.dev.vars.example',
      'LICENSE',
      'README.md',
      'app',
      'package.json',
      'scripts',
      'worker',
      'wrangler.jsonc',
    ]);
    // Assembling again starts clean.
    writeFileSync(join(out, 'stale'), 'x');
    assemble(root, out, '1.2.4');
    assert.ok(!has('stale'));
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(join(out, '..'), { recursive: true, force: true });
  }
});

test('assembling something that was not built says what to run', () => {
  const root = mkdtempSync(join(tmpdir(), 'pkg-empty-'));
  try {
    assert.throws(() => assemble(root, join(root, 'out'), '1'), /build first/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

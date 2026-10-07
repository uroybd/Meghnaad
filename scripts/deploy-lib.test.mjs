import assert from 'node:assert/strict';
import { test } from 'node:test';
import { accessReady, buildConfig, problem, stripJsonc } from './deploy-lib.mjs';

const AUD = 'a'.repeat(64);
const base = {
  name: 'taskwarrior-web',
  r2_buckets: [{ binding: 'TASKS', bucket_name: 'taskwarrior-sync' }],
  keep_vars: true,
};

test('stripJsonc keeps URLs inside strings and drops comments and trailing commas', () => {
  const text = `{
    // a comment
    "url": "https://x.example/a//b", /* block */
    "list": [1, 2,],
    "esc": "quote \\" // not a comment",
  }`;
  assert.deepEqual(JSON.parse(stripJsonc(text)), { url: 'https://x.example/a//b', list: [1, 2], esc: 'quote " // not a comment' });
});

test('the repo wrangler.jsonc parses', async () => {
  const { readBaseConfig } = await import('./deploy-lib.mjs');
  const c = readBaseConfig();
  assert.equal(c.r2_buckets[0].binding, 'TASKS');
  assert.equal(c.keep_vars, true);
  assert.equal(c.vars, undefined, 'no personal or placeholder vars are committed');
});

test('no settings leaves the config untouched apart from empty vars', () => {
  const c = buildConfig(base, {});
  assert.equal(c.r2_buckets[0].bucket_name, 'taskwarrior-sync');
  assert.deepEqual(c.vars, {});
  assert.equal(c.routes, undefined);
});

test('an existing bucket replaces the default and the input is not mutated', () => {
  const c = buildConfig(base, { R2_BUCKET: 'my-existing-bucket' });
  assert.equal(c.r2_buckets[0].bucket_name, 'my-existing-bucket');
  assert.equal(base.r2_buckets[0].bucket_name, 'taskwarrior-sync');
});

test('a custom domain adds the route and turns workers.dev off', () => {
  const c = buildConfig(base, { CUSTOM_DOMAIN: 'tasks.example.com' });
  assert.deepEqual(c.routes, [{ pattern: 'tasks.example.com', custom_domain: true }]);
  assert.equal(c.workers_dev, false);
  assert.equal(c.preview_urls, false);
});

test('Access settings become vars and the worker can be renamed', () => {
  const s = { TEAM_DOMAIN: 'https://me.cloudflareaccess.com', POLICY_AUD: AUD, WORKER_NAME: 'tasks-2' };
  const c = buildConfig(base, s);
  assert.deepEqual(c.vars, { TEAM_DOMAIN: s.TEAM_DOMAIN, POLICY_AUD: AUD });
  assert.equal(c.name, 'tasks-2');
  assert.equal(accessReady(s), true);
});

test('validation rejects typos with a readable reason', () => {
  assert.match(problem('TEAM_DOMAIN', 'https://me.example.com'), /cloudflareaccess/);
  assert.match(problem('POLICY_AUD', 'xyz'), /64/);
  assert.match(problem('CUSTOM_DOMAIN', 'https://tasks.example.com'), /hostname/);
  assert.match(problem('R2_BUCKET', 'Bad_Name'), /bucket/);
  assert.equal(problem('R2_BUCKET', ''), null, 'empty means not set');
  assert.equal(problem('CUSTOM_DOMAIN', 'tasks.example.co.uk'), null);
  assert.equal(accessReady({ TEAM_DOMAIN: 'https://me.cloudflareaccess.com' }), false);
});

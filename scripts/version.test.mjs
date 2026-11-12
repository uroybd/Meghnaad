import test from 'node:test';
import assert from 'node:assert/strict';
import { cargoVersion, currentVersion, parseVersion, withCargoVersion } from './version.mjs';

test('a version is read with or without the v, and gives a tag and an Android code', () => {
  assert.deepEqual(parseVersion('v1.2.3'), { name: '1.2.3', tag: 'v1.2.3', code: 1_002_003 });
  assert.deepEqual(parseVersion('0.1.0'), { name: '0.1.0', tag: 'v0.1.0', code: 1_000 });
  assert.equal(parseVersion('2.0.0-rc.1').name, '2.0.0-rc.1');
});

test('codes rise with the version, so an update is always newer than what it replaces', () => {
  const order = ['0.1.0', '0.1.1', '0.2.0', '0.10.0', '1.0.0', '1.0.1'].map((v) => parseVersion(v).code);
  assert.deepEqual(
    order,
    [...order].sort((a, b) => a - b),
  );
  assert.equal(new Set(order).size, order.length);
});

test('what is not a version, or would overflow, is refused', () => {
  for (const bad of ['', '1', '1.2', 'main-abc1234', '1.2.3.4', 'v1.2.x', '0.1000.0', '3000.0.0'])
    assert.throws(() => parseVersion(bad), /version|large/, bad);
});

test('the repository has a valid version', () => {
  assert.match(currentVersion().name, /^\d+\.\d+\.\d+/);
});

test('the crates have the version package.json has', () => {
  assert.equal(cargoVersion(), currentVersion().name, 'run `node scripts/version.mjs sync` (npm version does)');
});

test('only the workspace version is changed in Cargo.toml', () => {
  const toml =
    '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n\n[x]\nversion = "9"\n';
  assert.equal(withCargoVersion(toml, '1.2.3'), toml.replace('version = "0.1.0"', 'version = "1.2.3"'));
  assert.throws(() => withCargoVersion('[workspace]\n', '1.2.3'), /no version/);
});

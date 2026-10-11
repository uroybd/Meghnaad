import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bindgenVersion } from './build-engine.mjs';

test('the wasm-bindgen version is the one the lock file pins, not another crate that starts the same', () => {
  const lock = [
    '[[package]]',
    'name = "wasm-bindgen-futures"',
    'version = "0.4.79"',
    '',
    '[[package]]',
    'name = "wasm-bindgen"',
    'version = "0.2.129"',
    '',
    '[[package]]',
    'name = "wasm-bindgen-macro"',
    'version = "0.2.129"',
  ].join('\n');
  assert.equal(bindgenVersion(lock), '0.2.129');
});

test('a lock file without it is an error, not an empty version', () => {
  assert.throws(() => bindgenVersion('[[package]]\nname = "serde"\nversion = "1.0.0"\n'), /not in Cargo.lock/);
});

test("this repository's own lock file has one", () => {
  assert.match(bindgenVersion(), /^\d+\.\d+\.\d+$/);
});

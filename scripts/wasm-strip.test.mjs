import test from 'node:test';
import assert from 'node:assert/strict';
import { dropCustomSections } from './wasm-strip.mjs';

const HEADER = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
const custom = (name, payload) => [0, 1 + name.length + payload.length, name.length, ...Buffer.from(name), ...payload];

test('drops the named custom sections and nothing else', () => {
  const types = [1, 4, 1, 0x60, 0, 0]; // one function type: () -> ()
  const wasm = Buffer.from([
    ...HEADER,
    ...custom('name', [1, 2, 3]),
    ...types,
    ...custom('keep-me', [9]),
    ...custom('producers', []),
  ]);
  const out = dropCustomSections(wasm, ['name', 'producers']);
  assert.deepEqual([...out], [...HEADER, ...types, ...custom('keep-me', [9])]);
});

test('reads section sizes of more than one byte', () => {
  const big = new Array(300).fill(7);
  const size = 1 + 4 + big.length; // name length byte, "name", payload
  const section = [0, (size & 0x7f) | 0x80, size >> 7, 4, ...Buffer.from('name'), ...big];
  const out = dropCustomSections(Buffer.from([...HEADER, ...section]), ['name']);
  assert.deepEqual([...out], HEADER);
});

test('refuses something that is not WebAssembly', () => {
  assert.throws(() => dropCustomSections(Buffer.from('hello world'), ['name']), /not a WebAssembly binary/);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { formula, sumsIn } from './homebrew-formula.mjs';

const sum = (c) => c.repeat(64);
const sums = {
  'aarch64-apple-darwin': sum('a'),
  'x86_64-apple-darwin': sum('b'),
  'x86_64-unknown-linux-gnu': sum('c'),
  'aarch64-unknown-linux-gnu': sum('d'),
};

test('the formula names each platform’s archive and checks it', () => {
  const text = formula({ repo: 'uroybd/Meghnaad', version: '0.6.38', sums });
  const base = 'https://github.com/uroybd/Meghnaad/releases/download/v0.6.38/meghnaad-hook-0.6.38';
  for (const [target, s] of Object.entries(sums))
    assert.ok(text.includes(`url "${base}-${target}.tar.gz"\n      sha256 "${s}"`), target);
  assert.match(text, /class MeghnaadHook < Formula/);
  assert.match(text, /version "0\.6\.38"/);
  assert.match(text, /bin\.install "meghnaad-hook"/);
  // macOS and Linux each have their own two.
  assert.match(
    text,
    /on_macos do\n {4}if Hardware::CPU\.arm\?[\s\S]*aarch64-apple[\s\S]*CPU\.intel\?[\s\S]*x86_64-apple[\s\S]*on_linux/,
  );
});

test('a formula is not made from what is missing or wrong', () => {
  assert.throws(
    () => formula({ repo: 'uroybd/Meghnaad', version: '0.6.38', sums: { ...sums, 'x86_64-apple-darwin': '' } }),
    /x86_64-apple-darwin/,
  );
  assert.throws(() => formula({ repo: 'Meghnaad', version: '0.6.38', sums }), /owner\/name/);
  assert.throws(() => formula({ repo: 'uroybd/Meghnaad', version: '0.7.0-rc.1', sums }), /release version/);
});

test('the checksums are those of the archives in a folder', () => {
  const dir = mkdtempSync(join(tmpdir(), 'brew-'));
  for (const target of Object.keys(sums)) writeFileSync(join(dir, `meghnaad-hook-1.0.0-${target}.tar.gz`), target);
  const got = sumsIn(dir, '1.0.0');
  assert.equal(Object.keys(got).length, 4);
  assert.ok(Object.values(got).every((s) => /^[0-9a-f]{64}$/.test(s)));
  assert.notEqual(got['aarch64-apple-darwin'], got['x86_64-apple-darwin']);
  assert.throws(() => sumsIn(dir, '9.9.9'), /ENOENT/);
});

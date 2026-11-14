import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { isWorkspaceBuild, prune, workspaceNames } from './prune-target.mjs';

const names = workspaceNames({
  packages: [
    { name: 'tc-core', targets: [{ name: 'tc_core' }, { name: 'cli_exec' }] },
    { name: 'meghnaad-tui', targets: [{ name: 'meghnaad-tui' }] },
  ],
});

test('a build of a workspace crate is told from a dependency’s', () => {
  for (const e of [
    'libtc_core-0123456789abcdef.rlib',
    'tc_core-0123456789abcdef.d',
    'cli_exec-3dw0cu1n11sbs',
    'tc-core-0123456789abcdef',
    'meghnaad_tui-0123456789abcdef',
  ]) {
    assert.ok(isWorkspaceBuild(e, names), e);
  }
  for (const e of ['libserde-0123456789abcdef.rlib', 'tc_core_extra-0123456789abcdef', 'tc_core', 'cli_exec-xyz']) {
    assert.ok(!isWorkspaceBuild(e, names), e);
  }
});

test('every profile loses its builds of the workspace and keeps the dependencies’', () => {
  const target = mkdtempSync(join(tmpdir(), 'prune-target-'));
  try {
    const put = (p) => {
      mkdirSync(join(target, p, '..'), { recursive: true });
      writeFileSync(join(target, p), 'x');
    };
    const ours = [
      'debug/deps/libtc_core-0123456789abcdef.rlib',
      'debug/incremental/cli_exec-3dw0cu1n11sbs/s',
      'debug/.fingerprint/tc-core-0123456789abcdef/lib',
      'aarch64-linux-android/android/deps/libtc_core-fedcba9876543210.so',
    ];
    const theirs = ['debug/deps/libserde-0123456789abcdef.rlib', 'debug/meghnaad-tui', 'release/deps/x-1'];
    [...ours, ...theirs].forEach(put);
    mkdirSync(join(target, 'release/deps'), { recursive: true });
    const got = prune(target, names);
    assert.equal(got.entries, 4);
    for (const p of ours) assert.ok(!existsSync(join(target, p)), p);
    for (const p of theirs) assert.ok(existsSync(join(target, p)), p);
  } finally {
    rmSync(target, { recursive: true, force: true });
  }
});

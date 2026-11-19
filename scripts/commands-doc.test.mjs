import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import {
  COMMANDS_DOC,
  COMMANDS_FILES,
  commandsDoc,
  stampCommands,
  stampSince,
  unreleased,
  written,
} from './commands-doc.mjs';
import { ROOT } from './deploy-lib.mjs';

const read = (root, file) => readFileSync(join(root, file), 'utf8');
const data = () => JSON.parse(read(ROOT, COMMANDS_FILES[0]));

test('docs/commands.md is made from the commands file', () => {
  assert.equal(read(ROOT, COMMANDS_DOC), commandsDoc(data()), 'run node scripts/commands-doc.mjs');
});

test('every command has an anchor, its short form and what it takes', () => {
  const page = commandsDoc(data());
  for (const c of data().commands) {
    assert.match(page, new RegExp(`<a id="${c.name}"></a>`), c.name);
    assert.ok(c.summary && c.doc && c.since, c.name);
  }
  assert.equal(written({ name: 'tabnext', shortest: 4 }), 'tabn[ext]');
  assert.equal(written({ name: 'add', shortest: 3 }), 'add');
});

test('a release writes its version over unreleased, and leaves a version that is already there', () => {
  const d = { commands: [{ since: 'unreleased' }, { since: '0.1.0' }], lines: [{ since: 'unreleased' }] };
  assert.deepEqual(stampSince(d, '0.9.0'), {
    commands: [{ since: '0.9.0' }, { since: '0.1.0' }],
    lines: [{ since: '0.9.0' }],
  });
});

test('the bump stamps every copy and the page, so a release has none unreleased', () => {
  const root = mkdtempSync(join(tmpdir(), 'commands-'));
  try {
    for (const f of [...COMMANDS_FILES, COMMANDS_DOC]) {
      mkdirSync(dirname(join(root, f)), { recursive: true });
      cpSync(join(ROOT, COMMANDS_FILES[0]), join(root, f));
    }
    stampCommands('9.9.9', root);
    assert.deepEqual(unreleased(root), []);
    assert.ok(JSON.parse(read(root, COMMANDS_FILES[1])).commands.every((c) => c.since !== 'unreleased'));
    assert.match(read(root, COMMANDS_DOC), /Since 9\.9\.9\./);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

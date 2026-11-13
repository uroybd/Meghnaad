import test from 'node:test';
import assert from 'node:assert/strict';
import { commitsFor, parseSubject, releaseNotes } from './release-notes.mjs';

test('a conventional subject gives its type, scope and text', () => {
  assert.deepEqual(parseSubject('fix(web): keys work'), {
    type: 'fix',
    scope: 'web',
    breaking: false,
    text: 'keys work',
  });
  assert.deepEqual(parseSubject('feat!: new store'), { type: 'feat', scope: null, breaking: true, text: 'new store' });
  assert.equal(parseSubject('Release 1.2.3'), null);
});

test('the notes group features, fixes and the rest, oldest first, and leave chores out', () => {
  const commits = [
    { sha: 'c'.repeat(40), subject: 'chore: bump to 1.2.3' },
    { sha: 'b'.repeat(40), subject: 'fix(web): keys work on every page' },
    { sha: 'a'.repeat(40), subject: 'feat: just tidy' },
    { sha: '9'.repeat(40), subject: 'fix: colours follow Taskwarrior' },
    { sha: '8'.repeat(40), subject: 'ci: cache npm' },
    { sha: '7'.repeat(40), subject: 'doc: architecture' },
    { sha: '6'.repeat(40), subject: 'not conventional' },
  ];
  assert.equal(
    releaseNotes(commits),
    [
      '### Features',
      '',
      '- just tidy (aaaaaaa)',
      '',
      '### Fixes',
      '',
      '- colours follow Taskwarrior (9999999)',
      '- **web:** keys work on every page (bbbbbbb)',
      '',
      '### Documentation',
      '',
      '- architecture (7777777)',
    ].join('\n'),
  );
});

test('a release with nothing to note has empty notes', () => {
  assert.equal(releaseNotes([{ sha: 'a'.repeat(40), subject: 'chore: bump to 1.2.3' }]), '');
});

test('the commits of a release are read from git', () => {
  const commits = commitsFor('HEAD');
  assert.ok(commits.every((c) => /^[0-9a-f]{40}$/.test(c.sha) && typeof c.subject === 'string'));
});

import { describe, expect, it } from 'vitest';
import type { TaskCommand } from './appsettings';
import { commandWords, expandTaskCommands, isFilterWord } from './taskCommands';

const FAIL: TaskCommand = { name: 'fail', command: 'done outcome:fail' };
const LATE: TaskCommand = { name: 'late', command: '+OVERDUE list' };
const CMDS = [FAIL, LATE, { name: 'bad', command: 'fail +flagged' }];

const expand = (line: string, cmds = CMDS) => expandTaskCommands(line, cmds);

describe('expandTaskCommands', () => {
  it('stands for the whole command line, with or without `task`', () => {
    expect(expand('fail')).toBe('done outcome:fail');
    expect(expand('task fail')).toBe('done outcome:fail');
  });

  it('keeps the filter before it, and what follows after', () => {
    expect(expand('3 fail')).toBe('3 done outcome:fail');
    expect(expand('task 3,5-7 fail')).toBe('3,5-7 done outcome:fail');
    expect(expand('project:Home +work fail')).toBe('project:Home +work done outcome:fail');
    expect(expand('( +a or +b ) fail')).toBe('( +a or +b ) done outcome:fail');
    expect(expand('1 fail +x')).toBe('1 done outcome:fail +x');
    expect(expand('+work late')).toBe('+work +OVERDUE list');
  });

  it('can stand for a command that has a filter of its own', () => {
    expect(expand('late')).toBe('+OVERDUE list');
    expect(expand('project:Home late')).toBe('project:Home +OVERDUE list');
  });

  it('lets one task command use another', () => {
    expect(expand('2 bad')).toBe('2 done outcome:fail +flagged');
  });

  it('ends a loop', () => {
    const loop = [
      { name: 'a', command: 'b' },
      { name: 'b', command: 'a' },
    ];
    expect(expand('a', loop)).toBe('a');
  });

  it('wins over a Taskwarrior command or report of the same name', () => {
    expect(expand('1 done', [{ name: 'done', command: 'done outcome:ok' }])).toBe('1 done outcome:ok');
    expect(expand('next', [{ name: 'next', command: 'list +PENDING' }])).toBe('list +PENDING');
    // Written out once, so a command can be extended without looping on itself.
    expect(expand('task 2 done', [{ name: 'done', command: 'done outcome:ok' }])).toBe('2 done outcome:ok');
  });

  it('leaves alone a word that is not where the command goes', () => {
    expect(expand('add fail the test')).toBe('add fail the test');
    expect(expand('1 modify fail')).toBe('1 modify fail');
    expect(expand('1 annotate late for work')).toBe('1 annotate late for work');
    expect(expand('1 done -- fail')).toBe('1 done -- fail');
  });

  it('returns the line exactly as typed when nothing is expanded', () => {
    expect(expand('1 done   outcome:"a b"')).toBe('1 done   outcome:"a b"');
    expect(expand('fail', [])).toBe('fail');
    expect(expand('')).toBe('');
  });

  it('keeps quoted words together', () => {
    expect(expand(`project:'My Work' fail`)).toBe(`'project:My Work' done outcome:fail`);
    expect(expand('1 note', [{ name: 'note', command: `annotate 'a b'` }])).toBe(`1 annotate 'a b'`);
  });
});

describe('isFilterWord', () => {
  it('knows the shapes of a filter', () => {
    for (const w of [
      '+tag',
      '-tag',
      'project:Home',
      'rc.confirmation=no',
      '12',
      '1,3-5',
      '/re/',
      '(',
      ')',
      'and',
      'OR',
    ])
      expect(isFilterWord(w), w).toBe(true);
    expect(isFilterWord('2c2b9cb2-0f55-4a5c-9b7a-2f5b3c0e1d11')).toBe(true);
    for (const w of ['done', 'fail', 'next', 'add', 'deadbeef']) expect(isFilterWord(w), w).toBe(false);
  });
});

describe('commandWords', () => {
  it('is what a click adds after the tasks', () => {
    expect(commandWords({ name: 'x', command: `modify 'a b' +c` })).toEqual(['modify', 'a b', '+c']);
  });
});

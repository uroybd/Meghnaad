import { describe, expect, it } from 'vitest';
import type { TaskCommand } from './appsettings';
import { ChainError } from './compound';
import { commandSteps, expandTaskCommands, isFilterWord, planChain } from './taskCommands';

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

describe('commandSteps', () => {
  it('is what a click adds after the tasks, a statement at a time', () => {
    expect(commandSteps({ name: 'x', command: `modify 'a b' +c` })).toEqual([
      { words: ['modify', 'a b', '+c'], join: null },
    ]);
    expect(commandSteps({ name: 'x', command: 'done && modify +failed ; annotate x' })).toEqual([
      { words: ['done'], join: null },
      { words: ['modify', '+failed'], join: '&&' },
      { words: ['annotate', 'x'], join: ';' },
    ]);
  });
});

describe('planChain', () => {
  const both: TaskCommand[] = [
    ...CMDS,
    { name: 'wrap', command: 'done outcome:fail && modify +failed' },
    { name: 'tidy', command: 'wrap || annotate gave up' },
  ];
  const plan = (line: string, cmds = both) => planChain(line, cmds).map((s) => [s.join, s.line]);

  it('is the line itself when it is one statement', () => {
    expect(plan('1 done')).toEqual([[null, '1 done']]);
    expect(plan('', both)).toEqual([]);
  });

  it('writes out the task commands of each statement', () => {
    expect(plan('3 fail ; late')).toEqual([
      [null, '3 done outcome:fail'],
      [';', '+OVERDUE list'],
    ]);
  });

  it('puts the filter in front of every statement a command stands for, and what follows after the last', () => {
    expect(plan('3 wrap +x')).toEqual([
      [null, '3 done outcome:fail'],
      ['&&', '3 modify +failed +x'],
    ]);
  });

  it('puts a command that stands for a chain into the chain it is in', () => {
    expect(plan('count && 7 tidy ; list')).toEqual([
      [null, 'count'],
      ['&&', '7 done outcome:fail'],
      ['&&', '7 modify +failed'],
      ['||', '7 annotate gave up'],
      [';', 'list'],
    ]);
  });

  it('does not take an operator in quotes for one', () => {
    expect(plan(`add 'a && b' ; add c\\;d`)).toEqual([
      [null, `add 'a && b'`],
      [';', `add c';'d`],
    ]);
  });

  it('refuses a line with nothing on one side of an operator', () => {
    expect(() => plan('&& 1 done')).toThrow(ChainError);
    expect(() => plan('1 done &&')).toThrow(ChainError);
    expect(() => plan('1 done && ; 2 done')).toThrow(ChainError);
  });

  it('writes a chain back as a line', () => {
    expect(expandTaskCommands('3 wrap', both)).toBe('3 done outcome:fail && 3 modify +failed');
    expect(expandTaskCommands('1 done ; 2 done', both)).toBe('1 done ; 2 done');
  });
});

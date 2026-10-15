import { describe, expect, it } from 'vitest';
import { apply, complete, COMMAND_NAMES, insert, VIRTUAL_TAGS, type Vocab } from './completion';

// The completing is the engine's (tc_core::complete, tested there with the cases that were here). These check that the
// page's side carries the vocabulary, the line and the answers across.
const vocab: Vocab = {
  contexts: ['work'],
  reports: [
    { name: 'next', description: 'Most urgent' },
    { name: 'mine', description: 'My tasks' },
  ],
  tasks: [
    { id: 3, uuid: 'aaaa1111-0000-0000-0000-000000000000', description: 'Fix the tap' },
    { id: null, uuid: 'cccc3333-0000-0000-0000-000000000000', description: 'Old done task' },
  ],
  projects: ['Home', 'Work'],
  tags: ['errand'],
  udas: [
    { name: 'estimate', type: 'string', label: 'Size', values: ['big', 'small', ''], default: null, indicator: null },
  ],
};
const other: Vocab = { ...vocab, projects: ['Elsewhere'] };

describe('complete', () => {
  it('answers options with hints, and the span of the word', () => {
    const r = complete('mi', 2, vocab);
    expect(r.options).toEqual([{ value: 'mine', hint: 'My tasks' }]);
    expect([r.start, r.end, r.common]).toEqual([0, 2, 'mine']);
    expect(complete('estimate:b', 10, vocab).options.map((o) => o.value)).toEqual(['estimate:big']);
  });
  it('uses the vocabulary it is given, and takes another when that is another', () => {
    expect(complete('project:W', 9, vocab).options.map((o) => o.value)).toEqual(['project:Work']);
    expect(complete('project:E', 9, other).options.map((o) => o.value)).toEqual(['project:Elsewhere']);
    expect(complete('project:W', 9, other).options).toEqual([]);
    expect(complete('project:W', 9, vocab).options.length).toBe(1);
  });
  it('counts positions in UTF-16 units', () => {
    expect(complete('😀 pro', 6, vocab).start).toBe(3);
  });
});

describe('insert and apply', () => {
  it('insert puts a value in the word, with a space unless told not to', () => {
    const c = complete('3 do', 4, vocab);
    expect(insert('3 do', c, 'done')).toEqual({ line: '3 done ', caret: 7 });
    expect(insert('3 do', c, 'done', false)).toEqual({ line: '3 done', caret: 6 });
  });
  it('apply completes the shared prefix, and says whether it got anywhere', () => {
    expect(apply('3 do', complete('3 do', 4, vocab))).toEqual({ line: '3 done ', caret: 7, progressed: true });
    expect(apply('zzz', complete('zzz', 3, vocab)).progressed).toBe(false);
  });
});

describe('what the engine lists', () => {
  it('the commands and the virtual tags', () => {
    expect(COMMAND_NAMES).toContain('modify');
    expect(COMMAND_NAMES[0]).toBe('add');
    expect(VIRTUAL_TAGS).toContain('OVERDUE');
  });
});

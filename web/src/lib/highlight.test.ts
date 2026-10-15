import { describe, expect, it } from 'vitest';
import { highlight, highlightPieces } from './highlight';

// The same table as app/…/HighlightTest.kt: the page and the app colour a line the same way.
const commands = new Set(['add', 'modify', 'done', 'list', 'next', 'info']);
const parts = (line: string) => highlight(line, commands).map((t) => [line.slice(t.start, t.end), t.kind]);
const cap = (k: string) => k[0].toUpperCase() + k.slice(1);
const named = (line: string) => parts(line).map(([w, k]) => [w, cap(k)]);

describe('highlight', () => {
  it('a command line with an attribute and a tag', () => {
    expect(named('task add Buy milk due:tomorrow +home')).toEqual([
      ['task', 'Keyword'],
      ['add', 'Command'],
      ['Buy', 'Text'],
      ['milk', 'Text'],
      ['due', 'Attr'],
      [':', 'Punct'],
      ['tomorrow', 'Value'],
      ['+home', 'Tag'],
    ]);
  });

  it('ids before the command', () => {
    expect(named('1,3-5 done')).toEqual([
      ['1,3-5', 'Id'],
      ['done', 'Command'],
    ]);
    expect(named('3f2a9b1c-1111-2222-3333-444455556666 info')).toEqual([
      ['3f2a9b1c-1111-2222-3333-444455556666', 'Id'],
      ['info', 'Command'],
    ]);
  });

  it('brackets and operators', () => {
    expect(named('project:Home (+work or +play) list')).toEqual([
      ['project', 'Attr'],
      [':', 'Punct'],
      ['Home', 'Value'],
      ['(', 'Op'],
      ['+work', 'Tag'],
      ['or', 'Op'],
      ['+play', 'Tag'],
      [')', 'Op'],
      ['list', 'Command'],
    ]);
    expect(named('((+a))')).toEqual([
      ['(', 'Op'],
      ['(', 'Op'],
      ['+a', 'Tag'],
      [')', 'Op'],
      [')', 'Op'],
    ]);
  });

  it('quoted words are one thing', () => {
    expect(named('add "buy milk" due:"next week"')).toEqual([
      ['add', 'Command'],
      ['"buy milk"', 'Str'],
      ['due', 'Attr'],
      [':', 'Punct'],
      ['"next week"', 'Str'],
    ]);
  });

  it('settings for one command', () => {
    expect(named('rc.confirmation:no 1 modify -old')).toEqual([
      ['rc.confirmation', 'Config'],
      [':', 'Punct'],
      ['no', 'Config'],
      ['1', 'Id'],
      ['modify', 'Command'],
      ['-old', 'Tag'],
    ]);
  });

  it('an attribute with a modifier and an empty value', () => {
    expect(named('due.before:eow')).toEqual([
      ['due.before', 'Attr'],
      [':', 'Punct'],
      ['eow', 'Value'],
    ]);
    expect(named('project:')).toEqual([
      ['project', 'Attr'],
      [':', 'Punct'],
    ]);
  });

  it('only the first command counts and unknown words are text', () => {
    expect(named('add list done')).toEqual([
      ['add', 'Command'],
      ['list', 'Text'],
      ['done', 'Text'],
    ]);
    expect(named('unknown thing')).toEqual([
      ['unknown', 'Text'],
      ['thing', 'Text'],
    ]);
    expect(named('   ')).toEqual([]);
  });

  it('a minus before numbers is not a tag', () => {
    expect(named('add -5')).toEqual([
      ['add', 'Command'],
      ['-5', 'Text'],
    ]);
  });

  it('after add the rest is a description', () => {
    expect(named('add milk and eggs +home')).toEqual([
      ['add', 'Command'],
      ['milk', 'Text'],
      ['and', 'Text'],
      ['eggs', 'Text'],
      ['+home', 'Tag'],
    ]);
  });
});

describe('highlightPieces', () => {
  it('keeps every character, spaces included, in order', () => {
    const line = 'add a<b & c  +x';
    const pieces = highlightPieces(line, commands);
    expect(pieces.map((p) => p.text).join('')).toBe(line);
    expect(pieces[0]).toEqual({ text: 'add', kind: 'command' });
    expect(pieces.at(-1)).toEqual({ text: '+x', kind: 'tag' });
  });
});

import { describe, expect, it } from 'vitest';
import { previewLine, reportArgs, shellQuote, splitWords } from './cmdline';

describe('splitWords', () => {
  it('splits on whitespace and honours quotes', () => {
    expect(splitWords(`project:"Home Stuff" 'a b' c`)).toEqual(['project:Home Stuff', 'a b', 'c']);
  });
  it('handles escapes and empties', () => {
    expect(splitWords('a\\ b c')).toEqual(['a b', 'c']);
    expect(splitWords('   ')).toEqual([]);
    expect(splitWords("''")).toEqual(['']);
  });
});

describe('previewLine', () => {
  it('shows what would be typed', () => {
    expect(previewLine(['project:Home', '+work', 'list'])).toBe('task project:Home +work list');
  });
  it('quotes tokens that need it', () => {
    expect(previewLine(['add', 'description:Buy milk'])).toBe("task add 'description:Buy milk'");
    expect(shellQuote("it's")).toBe(`'it'\\''s'`);
    expect(shellQuote('')).toBe("''");
  });
  it('round-trips through splitWords', () => {
    const args = ['add', 'description:Buy "fresh" milk', "project:O'Neil", 'due:2026-12-25T08:30'];
    expect(splitWords(previewLine(args).slice(5))).toEqual(args);
  });
});

describe('reportArgs', () => {
  it('puts the report after the filter', () => {
    expect(reportArgs('project:Home +work', 'next')).toEqual(['project:Home', '+work', 'next']);
    expect(reportArgs('', 'list')).toEqual(['list']);
  });
});

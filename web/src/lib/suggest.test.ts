import { describe, expect, it } from 'vitest';
import { choose, isNew, suggest, wordAt } from './suggest';

const projects = ['Home', 'Home.Kitchen', 'Work', 'Work.Reports', 'Garden'];

describe('wordAt', () => {
  it('is the whole field for a single value', () => {
    expect(wordAt('Home.Ki', 3, false)).toEqual({ start: 0, end: 7, text: 'Home.Ki' });
  });

  it('is the word around the caret in a list, without a leading +', () => {
    expect(wordAt('errand wo', 9, true)).toEqual({ start: 7, end: 9, text: 'wo' });
    expect(wordAt('errand +wo', 10, true)).toEqual({ start: 8, end: 10, text: 'wo' });
    expect(wordAt('errand work', 3, true)).toEqual({ start: 0, end: 6, text: 'errand' });
    expect(wordAt('a,b', 3, true)).toEqual({ start: 2, end: 3, text: 'b' });
  });

  it('is empty after a space, ready for the next word', () => {
    expect(wordAt('errand ', 7, true)).toEqual({ start: 7, end: 7, text: '' });
    expect(wordAt('', 0, true)).toEqual({ start: 0, end: 0, text: '' });
  });
});

describe('suggest', () => {
  it('offers names that begin with what is typed, then names that contain it, ignoring case', () => {
    expect(suggest(projects, 'ho').map((o) => o.value)).toEqual(['Home', 'Home.Kitchen']);
    expect(suggest(projects, 'kit').map((o) => o.value)).toEqual(['Home.Kitchen']);
    // Nothing begins with it; these contain it.
    expect(suggest(projects, 'r').map((o) => o.value)).toEqual(['Work', 'Work.Reports', 'Garden']);
  });

  it('offers a few names for nothing typed, and none a list already has', () => {
    expect(suggest(projects, '').length).toBe(5);
    expect(suggest(['a', 'b', 'c'], '', new Set(['b'])).map((o) => o.value)).toEqual(['a', 'c']);
    expect(
      suggest(
        Array.from({ length: 30 }, (_, i) => `t${i}`),
        '',
      ).length,
    ).toBe(8);
  });

  it('has nothing to offer for a new name, or when what is typed is all there is', () => {
    expect(suggest(projects, 'Nowhere')).toEqual([]);
    expect(suggest(projects, 'Garden')).toEqual([]);
    expect(suggest(projects, 'Home').map((o) => o.value)).toEqual(['Home', 'Home.Kitchen']);
  });
});

describe('choose', () => {
  it('replaces a single value', () => {
    expect(choose('Ho', wordAt('Ho', 2, false), 'Home.Kitchen', false)).toEqual({ value: 'Home.Kitchen', caret: 12 });
  });

  it('replaces the word in a list and leaves a space for the next', () => {
    const v = 'errand wo';
    expect(choose(v, wordAt(v, 9, true), 'work', true)).toEqual({ value: 'errand work ', caret: 12 });
    const mid = 'er wo';
    expect(choose(mid, wordAt(mid, 2, true), 'errand', true)).toEqual({ value: 'errand wo', caret: 7 });
  });

  it('keeps a + the user typed', () => {
    const v = '+wo';
    expect(choose(v, wordAt(v, 3, true), 'work', true)).toEqual({ value: '+work ', caret: 6 });
  });
});

describe('isNew', () => {
  it('is true for a name nobody has used', () => {
    expect(isNew(projects, 'Taxes')).toBe(true);
    expect(isNew(projects, 'Home')).toBe(false);
    expect(isNew(projects, '  ')).toBe(false);
  });
});

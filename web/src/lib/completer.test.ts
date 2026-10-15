import { describe, expect, it } from 'vitest';
import { Completer } from './completer.svelte';
import type { Vocab } from './completion';

// The same cases as TypingSuggestionsTest in the app.
const vocab: Vocab = {
  reports: [],
  projects: ['Home', 'Home.Kitchen', 'Work'],
  tags: ['errand', 'work'],
  udas: [],
  tasks: [],
  contexts: [],
};

describe('suggestions while typing', () => {
  it('offers the completions of a word being typed without changing the line', () => {
    const c = new Completer();
    c.suggest('pro', 3, vocab);
    expect(c.menu?.base).toBe('pro');
    expect(c.menu?.options.some((o) => o.value.startsWith('project:'))).toBe(true);
    expect(c.menu?.sel).toBe(-1);
  });

  it('offers a value that is about to be typed', () => {
    const c = new Completer();
    c.suggest('project:', 8, vocab);
    expect(c.menu?.options.map((o) => o.value.replace('project:', ''))).toEqual(['Home', 'Home.Kitchen', 'Work']);
  });

  it('offers nothing for an empty word or what is already there', () => {
    const c = new Completer();
    c.suggest('', 0, vocab);
    expect(c.menu).toBeNull();
    c.suggest('add ', 4, vocab);
    expect(c.menu).toBeNull();
    c.suggest('project:Work', 12, vocab);
    expect(c.menu).toBeNull();
  });

  it('a tapped suggestion takes the words place', () => {
    const c = new Completer();
    c.suggest('project:Ho', 10, vocab);
    const e = c.pick(0);
    expect(e?.line.startsWith('project:Home')).toBe(true);
    expect(c.menu).toBeNull();
  });
});

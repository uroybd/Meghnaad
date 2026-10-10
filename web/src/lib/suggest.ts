// Type-ahead for the form fields that take names (a project, a list of tags): which names to offer for what is
// being typed, and what the field says once one is chosen. Typing a name that is not on offer is always fine;
// the offers are only a help. (The console's Tab completion is `completion.ts`.)

import type { Option } from './completion';

/** The part of a field being typed: `text` is what to offer names for, `start`/`end` is what a choice replaces. */
export interface Word {
  start: number;
  end: number;
  text: string;
}

const MAX = 8;
const SEPARATOR = /[\s,]/;

/**
 * For one value (a project) the whole field; for a list (tags, separated by spaces or commas) the word around
 * the caret, without a `+` in front of it.
 */
export function wordAt(value: string, caret: number, list: boolean): Word {
  if (!list) return { start: 0, end: value.length, text: value };
  let start = Math.min(caret, value.length);
  let end = start;
  while (start > 0 && !SEPARATOR.test(value[start - 1])) start--;
  while (end < value.length && !SEPARATOR.test(value[end])) end++;
  if (value[start] === '+') start++;
  return { start, end: Math.max(start, end), text: value.slice(start, end) };
}

/**
 * The names to offer for `typed`: those that begin with it first, then those that contain it (so `kit` finds
 * `Home.Kitchen`), ignoring case, never one in `taken`. Nothing when `typed` is already a name and nothing else
 * could be meant.
 */
export function suggest(names: readonly string[], typed: string, taken: ReadonlySet<string> = new Set()): Option[] {
  const t = typed.toLowerCase();
  const pool = names.filter((n) => !taken.has(n));
  const starts = pool.filter((n) => n.toLowerCase().startsWith(t));
  const inside = t ? pool.filter((n) => !n.toLowerCase().startsWith(t) && n.toLowerCase().includes(t)) : [];
  const all = [...starts, ...inside];
  if (all.length === 1 && all[0] === typed) return [];
  return all.slice(0, MAX).map((value) => ({ value }));
}

/** `value` with `word` replaced by `choice`; in a list a space follows, ready for the next one. */
export function choose(value: string, word: Word, choice: string, list: boolean): { value: string; caret: number } {
  const before = value.slice(0, word.start);
  const after = value.slice(word.end);
  // A list gets a space after the word unless one is there already; the caret goes past it either way.
  const gap = list && !/^[\s,]/.test(after) ? ' ' : '';
  return { value: before + choice + gap + after, caret: before.length + choice.length + (list ? 1 : 0) };
}

/** Is `typed` a name nobody has used yet? (Then the field says so.) */
export function isNew(names: readonly string[], typed: string): boolean {
  const t = typed.trim();
  return t !== '' && !names.includes(t);
}

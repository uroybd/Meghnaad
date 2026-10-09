// Toggling a tag in a report's filter, the way a click on a tag chip does.
import { shellQuote, splitWords } from './cmdline';

/** The tags a filter requires, as in `+work +home` (a `-tag` or any other word is not one). */
export function activeTags(filter: string): string[] {
  return splitWords(filter)
    .filter((w) => /^\+[^\s+-]/.test(w) || /^\+[^\s]+$/.test(w))
    .map((w) => w.slice(1));
}

/** `filter` with `+tag` removed if it is there, else added (replacing a `-tag` that would contradict it). */
export function toggleTag(filter: string, tag: string): string {
  const words = splitWords(filter);
  const plus = `+${tag}`;
  const next = words.includes(plus) ? words.filter((w) => w !== plus) : [...words.filter((w) => w !== `-${tag}`), plus];
  return next.map(shellQuote).join(' ');
}

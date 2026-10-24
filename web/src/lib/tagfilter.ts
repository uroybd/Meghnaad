// Toggling a tag in a report's filter, the way a click on a tag chip does.
import { shellQuote, splitWords } from './cmdline';

const isTag = (w: string) => /^\+[^\s+-]/.test(w) || /^\+[^\s]+$/.test(w);

/**
 * Where an `( +a or +b )` group of tags starts and ends in `words`, or null. Tags picked one after the other are kept
 * in one, so that picking `a` and then `b` shows the tasks that have either, and not only those with both.
 */
function tagGroup(words: string[]): [number, number] | null {
  for (let start = words.indexOf('('); start >= 0; start = words.indexOf('(', start + 1)) {
    let i = start + 1;
    let tags = 0;
    while (i < words.length && isTag(words[i])) {
      tags++;
      i++;
      if (words[i] !== 'or') break;
      i++;
    }
    if (tags >= 2 && words[i] === ')' && words[i - 1] !== 'or') return [start, i];
  }
  return null;
}

/** The tags a filter requires, as in `+work +home` or `( +work or +home )` (a `-tag` or any other word is not one). */
export function activeTags(filter: string): string[] {
  return splitWords(filter)
    .filter(isTag)
    .map((w) => w.slice(1));
}

/**
 * `filter` with `+tag` removed if it is there, else added (replacing a `-tag` that would contradict it). A second tag
 * joins the first with `or`: `+a` and then `b` is `( +a or +b )`, and a third goes into the same parentheses. A filter
 * the person wrote with several `+tag` words in it, which all have to be there, has the next one added the same way.
 */
export function toggleTag(filter: string, tag: string): string {
  let words = splitWords(filter);
  const plus = `+${tag}`;
  const group = tagGroup(words);
  if (group) {
    const [start, end] = group;
    const inside = words.slice(start + 1, end).filter((w) => w !== 'or');
    if (inside.includes(plus)) {
      const left = inside.filter((w) => w !== plus);
      words = [
        ...words.slice(0, start),
        ...(left.length === 1 ? left : ['(', ...left.flatMap((w, i) => (i ? ['or', w] : [w])), ')']),
        ...words.slice(end + 1),
      ];
    } else {
      words = [...words.slice(0, end), 'or', plus, ...words.slice(end)].filter((w) => w !== `-${tag}`);
    }
  } else if (words.includes(plus)) {
    words = words.filter((w) => w !== plus);
  } else {
    const alone = words.filter(isTag);
    words = words.filter((w) => w !== `-${tag}`);
    if (alone.length === 1) words = words.flatMap((w) => (w === alone[0] ? ['(', w, 'or', plus, ')'] : [w]));
    else words = [...words, plus];
  }
  return words.map(shellQuote).join(' ');
}

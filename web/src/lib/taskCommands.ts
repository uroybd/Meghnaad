// Task commands: a word that stands for a whole command line. `fail` for `done outcome:fail`, `late` for
// `+OVERDUE list`. They work with or without `task` in front, and with a filter before them (`project:Home fail`,
// `3 fail`, `+work late`), which stays in front of what the command stands for.
//
// Unlike the taskrc's `alias.*` (the engine's, which stands for words anywhere on the line), this is the browser's:
// it reads the line as Taskwarrior does, a filter and then the command, and only the command word can be a task
// command. So `add fail the test` is still a task named "fail the test". A task command wins over a Taskwarrior
// command or report with the same name.

import { shellQuote, splitWords } from './cmdline';
import type { TaskCommand } from './appsettings';

/** Taskwarrior's limit for aliases that use aliases. */
const ROUNDS = 10;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const LOGIC = new Set(['(', ')', 'and', 'or', 'xor', 'not', '!']);

/**
 * Is this word part of a filter, so the command (if there is one) comes later? The same shapes the engine takes
 * for a filter: `+tag` and `-tag`, anything with `:` or `=` (attributes, `rc.` settings), task numbers and ranges,
 * `/pattern/`, a uuid, and the words that join them.
 */
export function isFilterWord(w: string): boolean {
  return (
    /^[+-]./.test(w) ||
    /[:=]/.test(w) ||
    /^[\d,.-]+$/.test(w) ||
    w.startsWith('/') ||
    UUID.test(w) ||
    LOGIC.has(w.toLowerCase())
  );
}

/**
 * The line with a task command in it written out, or the line as it was when there is none. The command word is
 * the first word that isn't part of the filter; the rest of the line goes after what the command stands for, so
 * `1 fail +x` is `1 done outcome:fail +x`.
 */
export function expandTaskCommands(line: string, commands: TaskCommand[]): string {
  if (!commands.length) return line;
  const byName = new Map(commands.map((c) => [c.name, c.command]));
  let words = splitWords(line);
  if (words[0] === 'task') words = words.slice(1);
  let changed = false;
  // A name is written out once: `done` that stands for `done outcome:ok` then reaches Taskwarrior's `done`, and
  // two that stand for each other stop after one round each.
  const used = new Set<string>();
  for (let round = 0; round < ROUNDS; round++) {
    const at = words.findIndex((w) => w === '--' || !isFilterWord(w));
    if (at < 0 || words[at] === '--' || used.has(words[at])) break;
    const stands = byName.get(words[at]);
    if (stands === undefined) break;
    used.add(words[at]);
    words = [...words.slice(0, at), ...splitWords(stands), ...words.slice(at + 1)];
    changed = true;
  }
  return changed ? words.map(shellQuote).join(' ') : line;
}

/** The words a command adds after the tasks it is run on (a click on a task or on the selected tasks). */
export function commandWords(c: TaskCommand): string[] {
  return splitWords(c.command);
}

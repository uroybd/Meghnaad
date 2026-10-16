// Task commands: a word that stands for a whole command line. `fail` for `done outcome:fail`, `late` for
// `+OVERDUE list`. They work with or without `task` in front, and with a filter before them (`project:Home fail`,
// `3 fail`, `+work late`), which stays in front of what the command stands for.
//
// Unlike the taskrc's `alias.*` (the engine's, which stands for words anywhere on the line), this is the browser's:
// it reads the line as Taskwarrior does, a filter and then the command, and only the command word can be a task
// command. So `add fail the test` is still a task named "fail the test". A task command wins over a Taskwarrior
// command or report with the same name.

import { shellQuote, splitWords } from './cmdline';
import { splitChain, type Joiner, type Statement } from './compound';
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

/** One statement of a line, as the words to run and how it follows the one before. */
interface Piece {
  words: string[];
  join: Joiner | null;
}

/**
 * The statements a task command stands for in `words` (a statement as words), or `null` when the command word is not
 * a task command. The words before the command word (the filter) go in front of every statement it stands for, and the
 * words after it go after the last one, as an alias in a shell takes its arguments.
 */
function expandWords(words: string[], byName: Map<string, string>, used: Set<string>, depth: number): Piece[] | null {
  if (depth >= ROUNDS) return null;
  const at = words.findIndex((w) => w === '--' || !isFilterWord(w));
  if (at < 0 || words[at] === '--' || used.has(words[at])) return null;
  const stands = byName.get(words[at]);
  if (stands === undefined) return null;
  // A name is written out once: `done` that stands for `done outcome:ok` then reaches Taskwarrior's `done`, and
  // two that stand for each other stop after one round each.
  const seen = new Set(used).add(words[at]);
  const inner = splitChain(stands);
  const out: Piece[] = [];
  inner.forEach((s, i) => {
    const own = [...words.slice(0, at), ...splitWords(s.line), ...(i === inner.length - 1 ? words.slice(at + 1) : [])];
    const deeper = expandWords(own, byName, seen, depth + 1) ?? [{ words: own, join: null }];
    deeper.forEach((p, j) => out.push({ words: p.words, join: j === 0 ? s.join : p.join }));
  });
  return out;
}

/**
 * The statements of a line (`a && b ; c`, see `compound.ts`) with each task command in them written out. A statement
 * that is not a task command is kept as it was typed. Throws a `ChainError` for a line that is not a well-formed chain.
 */
export function planChain(line: string, commands: TaskCommand[]): Statement[] {
  return plan(line, commands).steps;
}

function plan(line: string, commands: TaskCommand[]): { steps: Statement[]; expanded: boolean } {
  const parts = splitChain(line);
  if (!commands.length) return { steps: parts, expanded: false };
  const byName = new Map(commands.map((c) => [c.name, c.command]));
  const steps: Statement[] = [];
  let expanded = false;
  for (const s of parts) {
    let words = splitWords(s.line);
    if (words[0] === 'task') words = words.slice(1);
    const pieces = expandWords(words, byName, new Set(), 0);
    if (!pieces) {
      steps.push(s);
      continue;
    }
    expanded = true;
    pieces.forEach((p, j) => steps.push({ line: p.words.map(shellQuote).join(' '), join: j === 0 ? s.join : p.join }));
  }
  return { steps, expanded };
}

/**
 * The line with its task commands written out, or the line as it was when there is none. A task command's own
 * statements are joined with their operators again, so the line reads as what runs.
 */
export function expandTaskCommands(line: string, commands: TaskCommand[]): string {
  const { steps, expanded } = plan(line, commands);
  if (!expanded) return line;
  return steps.map((s, i) => (i ? ` ${s.join} ` : '') + s.line).join('');
}

/** The statements a command stands for, as the words each adds after the tasks it is run on (a click on one or more tasks). */
export function commandSteps(c: TaskCommand): { words: string[]; join: Joiner | null }[] {
  return splitChain(c.command).map((s) => ({ words: splitWords(s.line), join: s.join }));
}

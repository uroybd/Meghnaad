// The app's own settings (`settings.json`), kept in the bucket beside the taskrc settings. The Worker only stores the
// text; this page reads it. So far it holds one thing: `taskCommands`, words you type (or click) that stand for a
// whole command line.
//
//   { "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" } ],
//     "boards": [ … ] }            (what a board is: boards.ts and tc_core::board)

import { normalizeBoards, type Board } from './boards';
import { ChainError, splitChain } from './compound';

/** The most the Worker saves (it says so in the same words). */
export const MAX_SETTINGS_BYTES = 64 * 1024;

export interface TaskCommand {
  /** The word that stands for `command`. */
  name: string;
  /** What it stands for: a command line, with or without a filter of its own. */
  command: string;
  /** A Lucide icon, by its name (`circle-x`), for the buttons that run it. */
  icon?: string;
}

/** The page the app opens on: a report (`{ "report": "next" }`) or a board (`{ "board": "week" }`). Nothing set is `next`. */
export type Homepage = { report: string } | { board: string };

/** A saved `homepage` that is one of the two shapes; anything else is no choice (the app opens on `next`). */
export function parseHomepage(v: unknown): Homepage | null {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
  const { report, board, ...more } = v as Record<string, unknown>;
  if (Object.keys(more).length || (report === undefined) === (board === undefined)) return null;
  if (typeof report === 'string' && report.trim()) return { report: report.trim() };
  if (typeof board === 'string' && board.trim()) return { board: board.trim() };
  return null;
}

export function sameHomepage(a: Homepage | null, b: Homepage | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export interface ParsedSettings {
  /** Every top-level key as it was, so what this page does not know about is kept when it saves. */
  raw: Record<string, unknown>;
  /** The valid task commands, in order. */
  commands: TaskCommand[];
  /** The valid boards, in order, as the engine tidied them. */
  boards: Board[];
  /** Where the app opens, if the settings say. */
  homepage: Homepage | null;
  /** What is wrong with the boards, one line each; a board or section with a problem is left out. */
  boardErrors: string[];
  /** What is wrong, one line each; a command with a problem is left out. */
  errors: string[];
}

/** A Lucide icon's name: lower case words joined by dashes (`circle-x`, `arrow-up-0-1`). */
export const ICON_NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/;

/**
 * Why `name` can't be a task command, or null. It has to be one word that can't be taken for part of a filter (a
 * number, `+tag`, `attribute:value`), because the line is read word by word.
 */
export function nameProblem(name: string): string | null {
  if (!name) return 'needs a name';
  if (/\s/.test(name)) return 'a name is one word (no spaces)';
  if (/^[+\-/(]/.test(name) || /[:='"]/.test(name)) return 'a name can’t start with + - / ( or contain : = or quotes';
  if (/^[\d,.]+$/.test(name)) return 'a name can’t be a number';
  if (name === 'task' || name === '--') return `‘${name}’ can’t be a name`;
  return null;
}

/** Why what a command stands for is not a command line (`done &&` has nothing after the `&&`), or `null`. */
export function commandProblem(command: string): string | null {
  if (!command.trim()) return 'needs a command';
  try {
    splitChain(command);
    return null;
  } catch (e) {
    if (e instanceof ChainError) return e.message;
    throw e;
  }
}

export function parseSettings(text: string): ParsedSettings {
  const out: ParsedSettings = { raw: {}, commands: [], boards: [], homepage: null, boardErrors: [], errors: [] };
  if (!text.trim()) return out;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (e) {
    out.errors.push(`Not valid JSON: ${e instanceof Error ? e.message : String(e)}`);
    return out;
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    out.errors.push('The settings must be a JSON object: { "taskCommands": [ … ] }');
    return out;
  }
  out.raw = value as Record<string, unknown>;
  out.homepage = parseHomepage(out.raw.homepage);
  if (out.raw.boards !== undefined) {
    const n = normalizeBoards(out.raw.boards);
    out.boards = n.boards;
    out.boardErrors = n.errors;
  }
  const list = out.raw.taskCommands;
  if (list === undefined) return out;
  if (!Array.isArray(list)) {
    out.errors.push('taskCommands must be a list');
    return out;
  }
  const seen = new Set<string>();
  list.forEach((c: unknown, i) => {
    const at = `taskCommands[${i}]`;
    if (!c || typeof c !== 'object' || Array.isArray(c))
      return void out.errors.push(`${at}: must be { "name": …, "command": … }`);
    const { name, command, icon } = c as Record<string, unknown>;
    if (typeof name !== 'string') return void out.errors.push(`${at}: name must be text`);
    const bad = nameProblem(name);
    if (bad) return void out.errors.push(`${at} (${name || 'no name'}): ${bad}`);
    const wrong = typeof command !== 'string' ? 'needs a command' : commandProblem(command);
    if (wrong || typeof command !== 'string') return void out.errors.push(`${at} (${name}): ${wrong}`);
    if (icon !== undefined && (typeof icon !== 'string' || !ICON_NAME.test(icon)))
      return void out.errors.push(`${at} (${name}): icon must be a Lucide icon name such as "circle-x"`);
    if (seen.has(name)) return void out.errors.push(`${at}: there is already a command named ${name}`);
    seen.add(name);
    out.commands.push(icon ? { name, command: command.trim(), icon } : { name, command: command.trim() });
  });
  return out;
}

/** The text to save: the other top-level keys as they were, and `taskCommands` as given. */
export function serializeSettings(raw: Record<string, unknown>, commands: TaskCommand[]): string {
  const { taskCommands: _old, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (commands.length)
    next.taskCommands = commands.map(({ name, command, icon }) => (icon ? { name, command, icon } : { name, command }));
  return JSON.stringify(next, null, 2) + '\n';
}

/**
 * The text to save: the other top-level keys as they were, and `boards` as given (none leaves the key out). A `homepage` of
 * `undefined` is left as it was; `null` takes it out, and a value sets it (a board whose slug changed must take the home page
 * with it).
 */
export function serializeBoards(raw: Record<string, unknown>, boards: Board[], homepage?: Homepage | null): string {
  const { boards: _old, boardsVersion: _v, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (homepage !== undefined) {
    delete next.homepage;
    if (homepage) next.homepage = homepage;
  }
  if (boards.length) {
    next.boardsVersion = 1;
    next.boards = boards;
  }
  return JSON.stringify(next, null, 2) + '\n';
}

/** The text to save: the other top-level keys as they were, and `homepage` as given (none leaves the key out). */
export function serializeHomepage(raw: Record<string, unknown>, homepage: Homepage | null): string {
  const { homepage: _old, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (homepage) next.homepage = homepage;
  return JSON.stringify(next, null, 2) + '\n';
}

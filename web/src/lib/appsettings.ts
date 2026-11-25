// The app's own settings (`settings.json`), kept in the bucket beside the taskrc settings. The Worker only stores the
// text; this page reads it. So far it holds one thing: `taskCommands`, words you type (or click) that stand for a
// whole command line.
//
//   { "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" } ],
//     "aliasIcons": { "rem": "bell" },   (an icon for a taskrc alias in the menus: taskCommands.ts aliasCommands)
//     "boards": [ … ],             (what a board is: boards.ts and tc_core::board)
//     "appearance": { "progress": { "progress": { "min": 0, "max": 100 } },
//                     "transformations": [ { "match": "\\b(BOR-\\d+)\\b", "replace": "[$1](https://…/$1)",
//                                            "fields": ["description"] } ] } }

import { callHelper } from '../engine/helpers';
import type { ConfigResponse } from './types';
import { normalizeBoards, type Board } from './boards';
import { ChainError, splitChain } from './compound';
import { emptyKanbanSettings, kanbanJson, parseKanban, type KanbanSettings } from './kanban';

/** The most the Worker saves (it says so in the same words). */
export const MAX_SETTINGS_BYTES = 64 * 1024;

export interface TaskCommand {
  /** The word that stands for `command`. */
  name: string;
  /** What it stands for: a command line, with or without a filter of its own. */
  command: string;
  /** A Lucide icon, by its name (`circle-x`), for the buttons that run it. */
  icon?: string;
  /** What the menus call it (`commandLabel`); the name, made words, without one. */
  label?: string;
}

/**
 * What a menu calls a task command (or an alias in the menus): its label, else its name as words, each run of `_`, `-`
 * or `.` a space and the first letter upper case (`kanban_next` → `Kanban next`), as a board control without a label
 * is shown (`boards.ts` `rangeLabel`).
 */
export function commandLabel(c: { name: string; label?: string }): string {
  const label = c.label?.trim();
  if (label) return label;
  const words = c.name.replace(/[_.-]+/g, ' ').trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * The page the app opens on: a report (`{ "report": "next" }`), a board (`{ "board": "week" }`) or a kanban
 * (`{ "kanban": "work" }`). Nothing set is `next`.
 */
export type Homepage = { report: string } | { board: string } | { kanban: string };

/** A saved `homepage` that is one of the three shapes; anything else is no choice (the app opens on `next`). */
export function parseHomepage(v: unknown): Homepage | null {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return null;
  const entries = Object.entries(v as Record<string, unknown>);
  if (entries.length !== 1) return null;
  const [[kind, value]] = entries;
  if (typeof value !== 'string' || !value.trim()) return null;
  if (kind === 'report') return { report: value.trim() };
  if (kind === 'board') return { board: value.trim() };
  if (kind === 'kanban') return { kanban: value.trim() };
  return null;
}

/** What a home page names, whichever kind it is. */
export function homepageName(h: Homepage): string {
  return 'report' in h ? h.report : 'board' in h ? h.board : h.kanban;
}

export function sameHomepage(a: Homepage | null, b: Homepage | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** A UDA shown as a progress bar: the value at which the bar is empty and the value at which it is full. */
export interface Progress {
  min: number;
  max: number;
}

/**
 * A find-and-replace run on a text field before it is drawn as Markdown (`tc_core::transform`): `match` is a regex,
 * `replace` may use `$1` and `${name}`, and `fields` names where it runs (`description`, `annotations` or a string UDA).
 */
export interface Transformation {
  match: string;
  replace: string;
  fields: string[];
  /** A Taskwarrior filter: the rule runs only on the tasks it matches (spec 0030). */
  condition?: string;
  /** With a condition: the UUIDs of the tasks it matches, filled in by the store (`store.rules`). Never saved. */
  tasks?: string[];
}

/** How the app draws things, kept in `settings.json` under `appearance`. */
export interface Appearance {
  /** The numeric UDAs shown as progress bars in reports, by name. */
  progress: Record<string, Progress>;
  /** The field transformations, in the order they run. */
  transformations: Transformation[];
  /** `idFallback`: a task with no ID (done, deleted) shows its short uuid, dimmed, rather than `task`'s `-`. */
  idFallback: boolean;
  /**
   * `cardIcons`: a card (a kanban's, a report's on the phone) shows Taskwarrior's fields by an icon rather than their
   * report label, and a UDA as a pill of its label and value.
   */
  cardIcons: boolean;
}

export const emptyAppearance = (): Appearance => ({
  progress: {},
  transformations: [],
  idFallback: false,
  cardIcons: false,
});

export const DEFAULT_PROGRESS: Progress = { min: 0, max: 100 };

/** Why `p` can't be the range of a progress bar, or null. */
export function progressProblem(p: Progress): string | null {
  if (!Number.isFinite(p.min) || !Number.isFinite(p.max)) return 'min and max must be numbers';
  if (p.min >= p.max) return 'min must be less than max';
  return null;
}

/** How far along `value` is, from 0 (at or under `min`) to 1 (at or over `max`). */
export function progressFraction(value: number, p: Progress): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, (value - p.min) / (p.max - p.min)));
}

export function sameAppearance(a: Appearance, b: Appearance): boolean {
  const names = Object.keys(a.progress).sort();
  const other = Object.keys(b.progress).sort();
  return (
    a.idFallback === b.idFallback &&
    a.cardIcons === b.cardIcons &&
    JSON.stringify(a.transformations) === JSON.stringify(b.transformations) &&
    names.join('\n') === other.join('\n') &&
    names.every((n) => a.progress[n].min === b.progress[n].min && a.progress[n].max === b.progress[n].max)
  );
}

const ALIAS_ICONS_NOT_OBJECT = 'aliasIcons must be an object: { "<alias>": "<icon>" }';
const NOT_SETTINGS = 'The settings must be a JSON object: { "taskCommands": [ … ] }';
const APPEARANCE_NOT_OBJECT = 'appearance must be an object: { "progress": { … } }';
const PROGRESS_NOT_OBJECT = 'appearance.progress must be an object: { "<uda>": { "min": 0, "max": 100 } }';
const TRANSFORMATIONS_NOT_LIST =
  'appearance.transformations must be a list: [ { "match": …, "replace": …, "fields": [ … ] } ]';

/** The valid part of a saved `appearance`, and what is wrong with the rest, one line each. */
export function parseAppearance(v: unknown): { appearance: Appearance; errors: string[] } {
  const appearance = emptyAppearance();
  const errors: string[] = [];
  if (v === undefined) return { appearance, errors };
  if (!v || typeof v !== 'object' || Array.isArray(v)) {
    return { appearance, errors: [APPEARANCE_NOT_OBJECT] };
  }
  const rules = parseTransformations((v as Record<string, unknown>).transformations);
  appearance.transformations = rules.transformations;
  errors.push(...rules.errors);
  const fallback = (v as Record<string, unknown>).idFallback;
  if (typeof fallback === 'boolean') appearance.idFallback = fallback;
  else if (fallback !== undefined) errors.push('appearance.idFallback must be true or false');
  const icons = (v as Record<string, unknown>).cardIcons;
  if (typeof icons === 'boolean') appearance.cardIcons = icons;
  else if (icons !== undefined) errors.push('appearance.cardIcons must be true or false');
  const list = (v as Record<string, unknown>).progress;
  if (list === undefined) return { appearance, errors };
  if (!list || typeof list !== 'object' || Array.isArray(list)) {
    return { appearance, errors: [PROGRESS_NOT_OBJECT] };
  }
  for (const [name, r] of Object.entries(list)) {
    const at = `appearance.progress.${name}`;
    if (!r || typeof r !== 'object' || Array.isArray(r)) {
      errors.push(`${at}: must be { "min": …, "max": … }`);
      continue;
    }
    const { min = DEFAULT_PROGRESS.min, max = DEFAULT_PROGRESS.max } = r as Record<string, unknown>;
    if (typeof min !== 'number' || typeof max !== 'number') {
      errors.push(`${at}: min and max must be numbers`);
      continue;
    }
    const bad = progressProblem({ min, max });
    if (bad) errors.push(`${at}: ${bad}`);
    else appearance.progress[name] = { min, max };
  }
  return { appearance, errors };
}

/**
 * The well-formed transformations, and what is wrong with the rest. Only the shape is checked here; whether a pattern
 * compiles and the fields exist is the engine's to say (`transformProblems`), and such a rule is kept so it can be fixed.
 */
export function parseTransformations(v: unknown): { transformations: Transformation[]; errors: string[] } {
  const transformations: Transformation[] = [];
  const errors: string[] = [];
  if (v === undefined) return { transformations, errors };
  if (!Array.isArray(v)) {
    return {
      transformations,
      errors: [TRANSFORMATIONS_NOT_LIST],
    };
  }
  v.forEach((r, i) => {
    const at = `appearance.transformations[${i}]`;
    const {
      match,
      replace = '',
      fields,
      condition,
    } = (r && typeof r === 'object' && !Array.isArray(r) ? r : {}) as Record<string, unknown>;
    if (typeof match !== 'string' || typeof replace !== 'string') {
      errors.push(`${at}: must be { "match": "…", "replace": "…", "fields": [ … ] }`);
    } else if (!Array.isArray(fields) || !fields.every((f) => typeof f === 'string')) {
      errors.push(`${at}: fields must be a list of field names`);
    } else if (condition !== undefined && typeof condition !== 'string') {
      errors.push(`${at}: condition must be a Taskwarrior filter, as text`);
    } else {
      const when = condition?.trim();
      transformations.push({ match, replace, fields: fields as string[], ...(when ? { condition: when } : {}) });
    }
  });
  return { transformations, errors };
}

export interface ParsedSettings {
  /** Every top-level key as it was, so what this page does not know about is kept when it saves. */
  raw: Record<string, unknown>;
  /** The valid task commands, in order. */
  commands: TaskCommand[];
  /** The icon chosen for each taskrc alias, by its name (one whose alias is not in the taskrc is kept, and unused). */
  aliasIcons: Record<string, string>;
  /** The valid boards, in order, as the engine tidied them. */
  boards: Board[];
  /** Where the app opens, if the settings say. */
  homepage: Homepage | null;
  /** How the app draws things (the valid part of `appearance`). */
  appearance: Appearance;
  /** What is wrong with `appearance`, one line each. */
  appearanceErrors: string[];
  /** What is wrong with the boards, one line each; a board or section with a problem is left out. */
  boardErrors: string[];
  /** The kanbans (spec 0026): whether they are on, and the valid ones. */
  kanban: KanbanSettings;
  /** What is wrong with `kanban`, one line each; a kanban with a problem is left out. */
  kanbanErrors: string[];
  /** What is wrong, one line each; a command with a problem is left out. */
  errors: string[];
}

/** A Lucide icon's name: lower case words joined by dashes (`circle-x`, `arrow-up-0-1`). */
export const ICON_NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/;

/**
 * Why `name` can't be a task command, or null. It has to be one word that can't be taken for part of a filter (a
 * number, `+tag`, `attribute:value`), because the line is read word by word. Nor can it be a word `task` would run as
 * a command (spec 0032): Taskwarrior's own, a unique abbreviation of one (`sync`), a report (`config`'s, else the default
 * ones), tasksh's `review`, or a console's own word; a label can say the same.
 */
export function nameProblem(name: string, config?: ConfigResponse['config']): string | null {
  if (!name) return 'needs a name';
  if (/\s/.test(name)) return 'a name is one word (no spaces)';
  if (/^[+\-/(]/.test(name) || /[:='"]/.test(name)) return 'a name can’t start with + - / ( or contain : = or quotes';
  if (/^[\d,.]+$/.test(name)) return 'a name can’t be a number';
  if (name === 'task' || name === '--') return `‘${name}’ can’t be a name`;
  const runs = CONSOLE_WORDS.includes(name) ? name : callHelper<string | null>('command_named', name, config ?? null);
  if (runs === null) return null;
  const label = `give it another name (its label can still say “${commandLabel({ name })}”)`;
  return runs === name ? `‘${name}’ is a command already: ${label}` : `‘${name}’ runs the command ${runs}: ${label}`;
}

/** The consoles' own words, answered before anything is sent (`clear` empties the scrollback). */
const CONSOLE_WORDS = ['clear', 'cls'];

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
  const out: ParsedSettings = {
    raw: {},
    commands: [],
    aliasIcons: {},
    boards: [],
    homepage: null,
    appearance: emptyAppearance(),
    appearanceErrors: [],
    boardErrors: [],
    kanban: emptyKanbanSettings(),
    kanbanErrors: [],
    errors: [],
  };
  if (!text.trim()) return out;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (e) {
    out.errors.push(`Not valid JSON: ${e instanceof Error ? e.message : String(e)}`);
    return out;
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    out.errors.push(NOT_SETTINGS);
    return out;
  }
  out.raw = value as Record<string, unknown>;
  out.homepage = parseHomepage(out.raw.homepage);
  ({ appearance: out.appearance, errors: out.appearanceErrors } = parseAppearance(out.raw.appearance));
  ({ kanban: out.kanban, errors: out.kanbanErrors } = parseKanban(out.raw.kanban));
  if (out.raw.boards !== undefined) {
    const n = normalizeBoards(out.raw.boards);
    out.boards = n.boards;
    out.boardErrors = n.errors;
  }
  const icons = out.raw.aliasIcons;
  if (icons !== undefined && !isObject(icons)) out.errors.push(ALIAS_ICONS_NOT_OBJECT);
  else if (icons)
    for (const [name, icon] of Object.entries(icons)) {
      if (typeof icon === 'string' && ICON_NAME.test(icon)) out.aliasIcons[name] = icon;
      else out.errors.push(`aliasIcons.${name}: must be a Lucide icon name such as "bell"`);
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
    const { name, command, icon, label } = c as Record<string, unknown>;
    if (typeof name !== 'string') return void out.errors.push(`${at}: name must be text`);
    const bad = nameProblem(name);
    if (bad) return void out.errors.push(`${at} (${name || 'no name'}): ${bad}`);
    const wrong = typeof command !== 'string' ? 'needs a command' : commandProblem(command);
    if (wrong || typeof command !== 'string') return void out.errors.push(`${at} (${name}): ${wrong}`);
    if (icon !== undefined && (typeof icon !== 'string' || !ICON_NAME.test(icon)))
      return void out.errors.push(`${at} (${name}): icon must be a Lucide icon name such as "circle-x"`);
    if (seen.has(name)) return void out.errors.push(`${at}: there is already a command named ${name}`);
    seen.add(name);
    const read: TaskCommand = icon ? { name, command: command.trim(), icon } : { name, command: command.trim() };
    // A label that isn't text is said, and the command is used without it.
    if (typeof label === 'string' && label.trim()) read.label = label;
    else if (label !== undefined && typeof label !== 'string') out.errors.push(`${at} (${name}): label must be text`);
    out.commands.push(read);
  });
  return out;
}

// Saving changes only what was edited. A save is a three-way merge (`mergeSettings`): the file as it is, what this app
// understood of it (the same save made with what was read, unchanged), and the save with the edit. A key this app
// doesn't know, at any depth, and an entry it couldn't read are in the file but not in what it understood, so they stay;
// what it understood and the edit took out goes. The phone and the terminal app merge the same way, tested against the
// same cases (`fixtures/settings-kept.json`).

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);

/** One text for a value, whatever order its keys are in. */
function canon(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(canon).join(',')}]`;
  if (isObject(v))
    return `{${Object.keys(v)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canon(v[k])}`)
      .join(',')}}`;
  return JSON.stringify(v) ?? 'null';
}

/** What an entry of a list is known by: its `id`, else its `name` (boards and sections; task commands), else nothing. */
function entryKey(v: unknown): string | null {
  if (!isObject(v)) return null;
  if (typeof v.id === 'string') return `id:${v.id}`;
  if (typeof v.name === 'string') return `name:${v.name}`;
  return null;
}

/** Whether `old` is the entry `base` was read from: what both have is the same (reading may add a default). */
function readFrom(old: unknown, base: unknown): boolean {
  if (!isObject(old) || !isObject(base)) return canon(old) === canon(base);
  return Object.keys(base).every((k) => !(k in old) || canon(old[k]) === canon(base[k]));
}

function mergeList(old: unknown[], base: unknown[], next: unknown[]): unknown[] {
  // Which entry of the file each entry understood was read from: the same id or name, else the next one like it.
  const used = new Set<number>();
  let after = 0;
  const source = base.map((b) => {
    const key = entryKey(b);
    const at =
      key !== null
        ? old.findIndex((o, i) => !used.has(i) && entryKey(o) === key)
        : old.findIndex((o, i) => i >= after && !used.has(i) && entryKey(o) === null && readFrom(o, b));
    if (at >= 0) {
      used.add(at);
      if (key === null) after = at + 1;
    }
    return at;
  });
  // Which entry understood each entry saved is: the same id or name, else the same place among those with neither.
  const keyless = base.flatMap((b, j) => (entryKey(b) === null ? [j] : []));
  let nth = 0;
  const out = next.map((n) => {
    const key = entryKey(n);
    const j = key !== null ? base.findIndex((b) => entryKey(b) === key) : (keyless[nth++] ?? -1);
    const from = j >= 0 ? source[j] : -1;
    return from >= 0 ? mergeSettings(old[from], base[j], n) : n;
  });
  // What this app couldn't read stays where it was.
  old.forEach((o, i) => {
    if (!used.has(i)) out.splice(Math.min(i, out.length), 0, o);
  });
  return out;
}

/**
 * `next` (the file as this app would write it with the edit) laid over `old` (the file as it is), where `base` is the file
 * as this app would write it without the edit. What is in `old` and not in `base` is kept: a key this app doesn't know, an
 * entry it couldn't read. What is in `base` and not in `next` was taken out. Lists of objects are matched entry by entry
 * (by `id`, else `name`, else place), so an entry keeps the keys this app doesn't know.
 */
export function mergeSettings(old: unknown, base: unknown, next: unknown): unknown {
  if (isObject(old) && isObject(next)) {
    const understood = isObject(base) ? base : {};
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(old)) {
      if (k in next) out[k] = mergeSettings(v, understood[k], next[k]);
      else if (!(k in understood)) out[k] = v;
      else {
        // Taken out: only what was understood of it goes.
        const rest = isObject(v) || Array.isArray(v) ? mergeSettings(v, understood[k], isObject(v) ? {} : []) : null;
        if (isObject(rest) ? Object.keys(rest).length : Array.isArray(rest) && rest.length) out[k] = rest;
      }
    }
    for (const [k, v] of Object.entries(next)) if (!(k in old)) out[k] = v;
    return out;
  }
  if (Array.isArray(old) && Array.isArray(next)) return mergeList(old, Array.isArray(base) ? base : [], next);
  return next;
}

const written = (v: unknown) => JSON.stringify(v, null, 2) + '\n';

/** `raw` read again, as this app reads it, for what it understood of the file. */
const reread = (raw: Record<string, unknown>) => parseSettings(JSON.stringify(raw));

// Each save is built twice, with what was read (`read`: true, which also writes the empty lists and sections that were in
// the file, since they were understood too) and with the edit.

function withCommands(raw: Record<string, unknown>, commands: TaskCommand[], read = false): Record<string, unknown> {
  const { taskCommands: _old, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (commands.length || (read && Array.isArray(raw.taskCommands)))
    next.taskCommands = commands.map(({ name, command, icon, label }) => ({
      name,
      command,
      ...(icon ? { icon } : {}),
      ...(label?.trim() ? { label: label.trim() } : {}),
    }));
  return next;
}

function withAliasIcons(
  raw: Record<string, unknown>,
  icons: Record<string, string>,
  read = false,
): Record<string, unknown> {
  const { aliasIcons: _old, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (Object.keys(icons).length || (read && isObject(raw.aliasIcons))) next.aliasIcons = { ...icons };
  return next;
}

/** The text to save: `taskCommands` and `aliasIcons` as given, and everything else in the file as it was. */
export function serializeSettings(
  raw: Record<string, unknown>,
  commands: TaskCommand[],
  aliasIcons: Record<string, string> = reread(raw).aliasIcons,
): string {
  const was = reread(raw);
  const base = withAliasIcons(withCommands(raw, was.commands, true), was.aliasIcons, true);
  return written(mergeSettings(raw, base, withAliasIcons(withCommands(raw, commands), aliasIcons)));
}

function withBoards(
  raw: Record<string, unknown>,
  boards: Board[],
  homepage?: Homepage | null,
  read = false,
): Record<string, unknown> {
  const { boards: _old, boardsVersion: _v, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (homepage !== undefined) {
    delete next.homepage;
    if (homepage) next.homepage = homepage;
  }
  if (boards.length || (read && Array.isArray(raw.boards))) {
    next.boardsVersion = 1;
    next.boards = boards;
  }
  return next;
}

/**
 * The text to save: `boards` as given (none leaves the key out), and everything else in the file as it was. A `homepage`
 * of `undefined` is left as it was; `null` takes it out, and a value sets it (a board whose slug changed must take the
 * home page with it).
 */
export function serializeBoards(raw: Record<string, unknown>, boards: Board[], homepage?: Homepage | null): string {
  const out = mergeSettings(
    raw,
    withBoards(raw, reread(raw).boards, undefined, true),
    withBoards(raw, boards, homepage),
  );
  return written(withoutLegacyRanges(homepage === undefined ? out : withHome(out, homepage), boards));
}

/**
 * The boards in `file` that were saved here without their `ranges`: reading moves a board's date ranges into its
 * `controls`, which it writes, so the old key, kept by the merge, would name them twice.
 */
export function withoutLegacyRanges(file: unknown, boards: { id: string }[]): unknown {
  if (!isObject(file) || !Array.isArray(file.boards)) return file;
  const ids = new Set(boards.map((b) => b.id));
  return {
    ...file,
    boards: file.boards.map((b) => {
      if (!isObject(b) || typeof b.id !== 'string' || !ids.has(b.id) || !('ranges' in b)) return b;
      const { ranges: _old, ...rest } = b;
      return rest;
    }),
  };
}

function withHomepage(raw: Record<string, unknown>, homepage: Homepage | null): Record<string, unknown> {
  const { homepage: _old, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (homepage) next.homepage = homepage;
  return next;
}

/** `file` with `homepage` exactly as chosen: it is one of two shapes, so nothing of an unreadable one is kept in it. */
function withHome(file: unknown, homepage: Homepage | null): unknown {
  if (!isObject(file)) return file;
  const out = { ...file };
  if (homepage) out.homepage = homepage;
  else delete out.homepage;
  return out;
}

/** The text to save: `homepage` as given (none leaves the key out), and everything else in the file as it was. */
export function serializeHomepage(raw: Record<string, unknown>, homepage: Homepage | null): string {
  return written(
    withHome(mergeSettings(raw, withHomepage(raw, reread(raw).homepage), withHomepage(raw, homepage)), homepage),
  );
}

function withAppearance(raw: Record<string, unknown>, appearance: Appearance, read = false): Record<string, unknown> {
  const { appearance: was, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  const out: Partial<Appearance> = {};
  const there = read && isObject(was);
  if (Object.keys(appearance.progress).length || (there && isObject(was.progress))) out.progress = appearance.progress;
  if (appearance.transformations.length || (there && Array.isArray(was.transformations)))
    out.transformations = appearance.transformations;
  if (appearance.idFallback || (there && typeof was.idFallback === 'boolean')) out.idFallback = appearance.idFallback;
  if (appearance.cardIcons || (there && typeof was.cardIcons === 'boolean')) out.cardIcons = appearance.cardIcons;
  if (Object.keys(out).length || there) next.appearance = out;
  return next;
}

/** The text to save: `appearance` as given (nothing chosen leaves the key out), and everything else in the file as it was. */
export function serializeAppearance(raw: Record<string, unknown>, appearance: Appearance): string {
  return written(
    mergeSettings(raw, withAppearance(raw, reread(raw).appearance, true), withAppearance(raw, appearance)),
  );
}

function withKanban(raw: Record<string, unknown>, kanban: KanbanSettings, read = false): Record<string, unknown> {
  const { kanban: was, ...rest } = raw;
  const next: Record<string, unknown> = { ...rest };
  if (kanban.enabled || kanban.setup || kanban.kanbans.length || (read && isObject(was)))
    next.kanban = kanbanJson(kanban);
  return next;
}

/** The text to save: `kanban` as given, and everything else in the file as it was. */
export function serializeKanban(raw: Record<string, unknown>, kanban: KanbanSettings): string {
  return written(mergeSettings(raw, withKanban(raw, reread(raw).kanban, true), withKanban(raw, kanban)));
}

/** What is wrong with a `settings.json`, as a linter says it: an error stops it being saved, a warning doesn't. */
export interface Lint {
  errors: string[];
  warnings: string[];
}

/** The top-level keys that are objects, and those that are lists. */
const OBJECT_KEYS = ['homepage', 'appearance', 'kanban', 'keys', 'web', 'mobile', 'tui', 'aliasIcons'];
const LIST_KEYS = ['taskCommands', 'boards'];

/**
 * Lint `text` as `settings.json`. Errors: not JSON, not an object, over 64 KB, or a known key in the wrong shape (a
 * section that is not an object, a list that is not a list, a board that is not an object or has no `sections` list).
 * Warnings: everything this app can't use, which is kept in the file (a task command with a bad name, a malformed board,
 * a progress bar with no range, and what `more` says, such as the keys' problems).
 */
export function lintSettings(text: string, more: (raw: Record<string, unknown>) => string[] = () => []): Lint {
  const errors: string[] = [];
  const size = new Blob([text]).size;
  if (size > MAX_SETTINGS_BYTES)
    errors.push(
      `The file is ${Math.ceil(size / 1024)} KB; settings.json holds at most ${MAX_SETTINGS_BYTES / 1024} KB.`,
    );
  if (text.trim()) {
    let value: unknown;
    try {
      value = JSON.parse(text);
    } catch (e) {
      return { errors: [...errors, `Not valid JSON: ${e instanceof Error ? e.message : String(e)}`], warnings: [] };
    }
    if (!isObject(value)) return { errors: [...errors, NOT_SETTINGS], warnings: [] };
  }
  const read = parseSettings(text);
  const raw = read.raw;
  for (const k of OBJECT_KEYS)
    // A device's own `"homepage": null` is next, over everyone's home page.
    if (k in raw && !isObject(raw[k]) && !(k === 'homepage' && raw[k] === null))
      errors.push(
        k === 'appearance'
          ? APPEARANCE_NOT_OBJECT
          : k === 'aliasIcons'
            ? ALIAS_ICONS_NOT_OBJECT
            : `${k} must be an object`,
      );
  for (const k of LIST_KEYS) if (k in raw && !Array.isArray(raw[k])) errors.push(`${k} must be a list`);
  if (isObject(raw.appearance)) {
    const a = raw.appearance;
    if ('progress' in a && !isObject(a.progress)) errors.push(PROGRESS_NOT_OBJECT);
    if ('transformations' in a && !Array.isArray(a.transformations)) errors.push(TRANSFORMATIONS_NOT_LIST);
  }
  if (Array.isArray(raw.boards))
    raw.boards.forEach((b, i) => {
      if (!isObject(b)) errors.push(`boards[${i}] must be an object`);
      else if (!Array.isArray(b.sections))
        errors.push(`boards[${i}]${typeof b.id === 'string' ? ` (${b.id})` : ''}: needs a sections list`);
    });
  // What can't be used, less what is already an error.
  const said = new Set(errors);
  const shapes =
    /^(boards must be a list|board \d+: not an object|taskCommands must be a list|kanban must be an object.*|kanban\.(kanbans|columns) must be a list)$/;
  if (isObject(raw.kanban) && 'kanbans' in raw.kanban && !Array.isArray(raw.kanban.kanbans))
    errors.push('kanban.kanbans must be a list');
  if (isObject(raw.kanban) && 'columns' in raw.kanban && !Array.isArray(raw.kanban.columns))
    errors.push('kanban.columns must be a list');
  const warnings = [
    ...read.errors,
    ...read.appearanceErrors,
    ...read.boardErrors,
    ...read.kanbanErrors,
    ...more(raw),
  ].filter((w) => !said.has(w) && !shapes.test(w));
  return { errors, warnings };
}

// Tab completion for the console: commands and reports, attributes and their modifiers,
// tags, task ids, and values (projects, priorities, UDA values, date words). Everything is
// built from what the server has told us, and each option can carry a short hint.

import { SORTABLE } from './sortSpec';
import type { UdaDef } from './types';

export interface TaskRef {
  id: number | null;
  uuid: string;
  description: string;
}

export interface Vocab {
  /** Report names, with their descriptions as hints. */
  reports: { name: string; description?: string | null }[];
  projects: string[];
  tags: string[];
  udas: UdaDef[];
  tasks: TaskRef[];
  contexts: string[];
}

export interface Option {
  value: string;
  hint?: string;
}

export interface Completion {
  /** Replace line[start:end] with `common` (or with one option's value). */
  start: number;
  end: number;
  options: Option[];
  /** Longest common prefix of the option values. */
  common: string;
}

export const COMMANDS: Record<string, string> = {
  add: 'create a task',
  modify: 'change tasks',
  done: 'complete tasks',
  delete: 'delete tasks',
  start: 'start working on tasks',
  stop: 'stop working on tasks',
  annotate: 'add a note',
  denotate: 'remove a note',
  append: 'add words to the end of the description',
  prepend: 'add words to the start of the description',
  undo: 'revert the last change',
  info: 'show everything about tasks',
  count: 'count matching tasks',
  projects: 'list projects',
  tags: 'list tags',
  udas: 'list user defined attributes',
  columns: 'list report columns',
  reports: 'list reports',
  contexts: 'list contexts',
  show: 'show imported settings',
  export: 'tasks as JSON',
  ids: 'ids of matching tasks',
  uuids: 'uuids of matching tasks',
  help: 'how to use this',
};

const ATTRS: Record<string, string> = {
  project: 'project name',
  priority: 'H, M or L',
  due: 'due date',
  wait: 'hide until',
  scheduled: 'scheduled date',
  until: 'expires',
  start: 'when started',
  end: 'when finished',
  entry: 'when created',
  depends: 'tasks this depends on',
  description: 'the text',
  status: 'pending, completed, deleted…',
  urgency: 'computed score',
  tags: 'tag list',
  limit: 'page, none or a number',
};

const DATE_ATTRS = new Set(['due', 'wait', 'scheduled', 'until', 'start', 'end', 'entry', 'modified']);
const NUMERIC_ATTRS = new Set(['urgency', 'id']);

const DATE_WORDS: Option[] = [
  { value: 'now' }, { value: 'today' }, { value: 'tomorrow' }, { value: 'yesterday' },
  { value: 'sod', hint: 'start of day' }, { value: 'eod', hint: 'end of day' },
  { value: 'sow', hint: 'start of week' }, { value: 'eow', hint: 'end of week' },
  { value: 'som', hint: 'start of month' }, { value: 'eom', hint: 'end of month' },
  { value: 'soy', hint: 'start of year' }, { value: 'eoy', hint: 'end of year' },
  { value: 'monday' }, { value: 'tuesday' }, { value: 'wednesday' }, { value: 'thursday' },
  { value: 'friday' }, { value: 'saturday' }, { value: 'sunday' },
  { value: '1d', hint: 'in a day' }, { value: '3d', hint: 'in 3 days' },
  { value: '1w', hint: 'in a week' }, { value: '2w', hint: 'in 2 weeks' }, { value: '1mo', hint: 'in a month' },
];

export const VIRTUAL_TAGS = [
  'ACTIVE', 'ANNOTATED', 'BLOCKED', 'BLOCKING', 'CHILD', 'COMPLETED', 'DELETED', 'DUE', 'DUETODAY',
  'INSTANCE', 'MONTH', 'ORPHAN', 'OVERDUE', 'PARENT', 'PENDING', 'PRIORITY', 'PROJECT', 'QUARTER',
  'READY', 'SCHEDULED', 'TAGGED', 'TEMPLATE', 'TODAY', 'TOMORROW', 'UDA', 'UNBLOCKED', 'UNTIL',
  'WAITING', 'WEEK', 'YEAR', 'YESTERDAY',
];

const MOD_HINTS: Record<string, string> = {
  is: 'exactly', isnt: 'not exactly', not: 'not (prefix)', has: 'contains', hasnt: 'does not contain',
  startswith: 'starts with', endswith: 'ends with', word: 'as a whole word', noword: 'not as a word',
  before: 'earlier than', after: 'later than', by: 'at or before', none: 'is unset', any: 'is set',
};
const MODS_STRING = ['is', 'isnt', 'not', 'has', 'hasnt', 'startswith', 'endswith', 'word', 'noword', 'none', 'any'];
const MODS_DATE = ['before', 'after', 'by', 'is', 'isnt', 'not', 'none', 'any'];
const MODS_NUMBER = ['before', 'after', 'by', 'is', 'isnt', 'not', 'none', 'any'];

const MAX_OPTIONS = 40;

function commonPrefix(xs: string[]): string {
  if (xs.length === 0) return '';
  let p = xs[0];
  for (const x of xs) while (!x.startsWith(p)) p = p.slice(0, -1);
  return p;
}

function matching(stem: string, pool: Option[], ci = false): Option[] {
  const s = ci ? stem.toLowerCase() : stem;
  const seen = new Set<string>();
  return pool
    .filter((o) => (ci ? o.value.toLowerCase() : o.value).startsWith(s))
    .filter((o) => !seen.has(o.value) && !!seen.add(o.value))
    .sort((a, b) => (a.value < b.value ? -1 : a.value > b.value ? 1 : 0))
    .slice(0, MAX_OPTIONS);
}

const opt = (values: string[], hint?: string): Option[] => values.map((value) => ({ value, hint }));

function attrKind(name: string, v: Vocab): 'date' | 'number' | 'string' {
  if (DATE_ATTRS.has(name)) return 'date';
  if (NUMERIC_ATTRS.has(name)) return 'number';
  const u = v.udas.find((x) => x.name === name);
  if (u?.type === 'date') return 'date';
  if (u?.type === 'numeric') return 'number';
  return 'string';
}

function taskOptions(v: Vocab): Option[] {
  return v.tasks.flatMap((t) => [
    ...(t.id != null ? [{ value: String(t.id), hint: t.description }] : []),
    { value: t.uuid.slice(0, 8), hint: t.description },
  ]);
}

export function complete(line: string, caret: number, v: Vocab): Completion {
  const before = line.slice(0, caret);
  const start = before.lastIndexOf(' ') + 1;
  const word = before.slice(start);
  const none: Completion = { start, end: caret, options: [], common: word };
  if (!word) return none;

  const done = (prefix: string, options: Option[]): Completion => {
    const full = options.map((o) => ({ ...o, value: prefix + o.value }));
    return { start, end: caret, options: full, common: commonPrefix(full.map((o) => o.value)) || word };
  };

  // +tag / -tag, including virtual tags.
  if (word[0] === '+' || word[0] === '-') {
    const stem = word.slice(1);
    const pool = [...opt(v.tags), ...opt(VIRTUAL_TAGS, 'virtual')];
    return done(word[0], matching(stem, pool, true));
  }

  // rc.report.<name>.<setting>:<value> and rc.context:<name> (per-command overrides).
  if (word.startsWith('rc.')) return completeOverride(word, start, caret, v);

  // A bare number is a task id; list ids whose digits start with it.
  if (/^\d+$/.test(word)) {
    const ids = taskOptions(v).filter((o) => /^\d+$/.test(o.value));
    return done('', matching(word, ids));
  }

  const sep = word.search(/[:=]/);
  if (sep > 0) {
    // name[.modifier]:value
    const lhs = word.slice(0, sep + 1);
    const stem = word.slice(sep + 1);
    const name = lhs.slice(0, -1).split('.')[0];
    const uda = v.udas.find((u) => u.name === name);
    let pool: Option[] = [];
    if (name === 'project') pool = opt(v.projects);
    else if (name === 'priority') pool = opt(['H', 'M', 'L']);
    else if (name === 'status') pool = opt(['pending', 'completed', 'deleted', 'recurring', 'waiting']);
    else if (name === 'limit') pool = [{ value: 'page' }, { value: 'none' }, { value: '10' }, { value: '25' }];
    else if (name === 'depends') {
      // A comma list: complete the last item, keep the rest.
      const lastComma = stem.lastIndexOf(',');
      const kept = lhs + stem.slice(0, lastComma + 1);
      const c = done(kept, matching(stem.slice(lastComma + 1).replace(/^[-+]/, ''), taskOptions(v)));
      return c;
    } else if (DATE_ATTRS.has(name) || uda?.type === 'date') pool = DATE_WORDS;
    else if (uda?.values.length) pool = opt(uda.values.filter(Boolean));
    return done(lhs, matching(stem, pool));
  }

  // name.modifier (no colon yet): offer the modifiers that make sense for the attribute.
  const dot = word.indexOf('.');
  if (dot > 0) {
    const name = word.slice(0, dot);
    const known = name in ATTRS || v.udas.some((u) => u.name === name);
    if (known) {
      const kind = attrKind(name, v);
      const mods = kind === 'date' ? MODS_DATE : kind === 'number' ? MODS_NUMBER : MODS_STRING;
      const pool = mods.map((m) => ({ value: m + ':', hint: MOD_HINTS[m] }));
      return done(name + '.', matching(word.slice(dot + 1), pool));
    }
    return none;
  }

  // Command, report, or attribute name.
  const pool: Option[] = [
    ...Object.entries(COMMANDS).map(([value, hint]) => ({ value, hint })),
    ...v.reports.map((r) => ({ value: r.name, hint: r.description ?? 'report' })),
    ...Object.entries(ATTRS).map(([n, hint]) => ({ value: n + ':', hint })),
    ...v.udas.map((u) => ({ value: u.name + ':', hint: u.label ?? 'UDA' })),
  ];
  const options = matching(word, pool);
  return { start, end: caret, options, common: commonPrefix(options.map((o) => o.value)) || word };
}

const REPORT_SETTINGS: Record<string, string> = {
  sort: 'sort order, e.g. due+,priority-',
  filter: 'extra filter for this report',
  columns: 'columns to show',
  labels: 'column headings',
};

function completeOverride(word: string, start: number, caret: number, v: Vocab): Completion {
  const make = (prefix: string, options: Option[]): Completion => {
    const full = options.map((o) => ({ ...o, value: prefix + o.value }));
    return { start, end: caret, options: full, common: commonPrefix(full.map((o) => o.value)) || word };
  };
  const rest = word.slice(3); // after "rc."

  const ctx = /^context[:=](.*)$/.exec(rest);
  if (ctx) return make(word.slice(0, word.length - ctx[1].length), matching(ctx[1], opt(v.contexts, 'context')));

  // rc.report.<name>.sort:<keys>  — complete the last key, e.g. `du` -> `due+` / `due-`.
  const sort = /^report\.([^.:=]+)\.sort[:=](.*)$/.exec(rest);
  if (sort) {
    const lastComma = sort[2].lastIndexOf(',');
    const stem = sort[2].slice(lastComma + 1);
    const prefix = word.slice(0, word.length - stem.length);
    const cols = [...SORTABLE, ...v.udas.map((u) => u.name)];
    const pool = cols.flatMap((c) => [{ value: `${c}+`, hint: 'ascending' }, { value: `${c}-`, hint: 'descending' }]);
    return make(prefix, matching(stem, pool));
  }

  // rc.report.<name>.<setting>
  const setting = /^report\.([^.:=]+)\.([a-z]*)$/.exec(rest);
  if (setting) {
    const pool = Object.entries(REPORT_SETTINGS).map(([k, hint]) => ({ value: `${k}:`, hint }));
    return make(`rc.report.${setting[1]}.`, matching(setting[2], pool));
  }

  // rc.report.<name>
  const name = /^report\.([^.:=]*)$/.exec(rest);
  if (name) {
    const pool = v.reports.map((r) => ({ value: `${r.name}.`, hint: r.description ?? 'report' }));
    return make('rc.report.', matching(name[1], pool));
  }

  // rc.<what>
  const pool: Option[] = [
    { value: 'report.', hint: 'change a report for this command' },
    { value: 'context:', hint: 'use a context for this command' },
  ];
  return make('rc.', matching(rest, pool));
}

/** Insert `value` in place of the completed word; add a space unless it ends mid-token. */
export function insert(line: string, c: Completion, value: string, space = true): { line: string; caret: number } {
  // No space after a separator, or after a sort direction (`due+`), which a `,` usually follows.
  const tail = space && !/[:=.,+-]$/.test(value) ? ' ' : '';
  const next = line.slice(0, c.start) + value + tail + line.slice(c.end);
  return { line: next, caret: c.start + value.length + tail.length };
}

/**
 * Tab behaviour: complete the shared prefix; when that makes no progress, the caller opens a
 * menu of `options`. A single option is completed in full (plus a space).
 */
export function apply(line: string, c: Completion): { line: string; caret: number; progressed: boolean } {
  if (c.options.length === 0) return { line, caret: c.end, progressed: false };
  const typed = line.slice(c.start, c.end);
  const unique = c.options.length === 1;
  const r = insert(line, c, c.common, unique);
  return { ...r, progressed: unique || c.common.length > typed.length };
}

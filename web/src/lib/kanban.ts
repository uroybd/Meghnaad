// Kanbans (spec 0026): ordered columns, each a filter that picks its tasks and a command line run on a task moved into
// it. A kanban is drawn from a report (its filter, sort and columns, which are what a card shows) and adds its own
// filter; the page can change the report, the filter and Completed since while it is open. Everything here is a command
// line sent to the engine: which task is in which column is what the engine answers, never decided here.
//
// The UDA, the report, the filter, Completed since and the columns are Kanban's own, for every kanban; a kanban has a
// name and an address and may have its own report, filter, Completed since or columns in place of those:
//
//   "kanban": { "enabled": true, "setup": "automatic", "uda": "progressState",
//               "report": "all", "filter": "( +PENDING or +COMPLETED )", "completedSince": "today-14d",
//               "columns": [ { "name": "To do", "filter": "status:pending progressState:",
//                              "move": "modify status:pending progressState:" } ],
//               "kanbans": [ { "id": "kanban", "name": "Kanban" },
//                            { "id": "work", "name": "Work", "filter": "project:Work" } ] }

import { commandProblem, type TaskCommand } from './appsettings';
import { freshId, SLUG } from './boards';
import { shellQuote, splitWords } from './cmdline';
import { splitChain, type Joiner } from './compound';
import { planChain } from './taskCommands';
import type { ConfigResponse, Row } from './types';

export interface KanbanColumn {
  name: string;
  /** Picks the column's tasks; a task is in the first column whose filter matches it. */
  filter: string;
  /** Run on a task dropped into the column, written as after a filter. Empty: the column takes no drops. */
  move: string;
}

/** What every kanban is drawn with, unless it has its own. */
export interface KanbanDefaults {
  /** The report it opens on: its filter, sort and columns. */
  report: string;
  /** Added to every column (empty: none). */
  filter: string;
  /** Completed tasks that ended before this date are left out (empty: none are). */
  completedSince: string;
  columns: KanbanColumn[];
}

/** A kanban as it is saved: what it doesn't have of its own is Kanban's. */
export interface KanbanEntry extends Partial<KanbanDefaults> {
  /** Its slug, the last part of its address (`/kanban/<id>`). */
  id: string;
  name: string;
}

/** A kanban as it is drawn: its own settings, else Kanban's. */
export interface Kanban extends KanbanDefaults {
  id: string;
  name: string;
}

export type Setup = 'automatic' | 'manual';

export interface KanbanSettings extends KanbanDefaults {
  enabled: boolean;
  /** How it was set up, when it was. */
  setup: Setup | null;
  /** The UDA the automatic setup defines and its columns use. */
  uda: string;
  kanbans: KanbanEntry[];
}

/** The settings a kanban can have of its own. */
export const OWN = ['report', 'filter', 'completedSince', 'columns'] as const;

export const PROGRESS_UDA = 'progressState';

export const emptyKanbanSettings = (): KanbanSettings => ({
  enabled: false,
  setup: null,
  uda: PROGRESS_UDA,
  report: 'next',
  filter: '',
  completedSince: '',
  columns: [],
  kanbans: [],
});

/** `k` as it is drawn: each setting its own, else Kanban's. */
export function resolveKanban(s: KanbanDefaults, k: KanbanEntry): Kanban {
  return {
    id: k.id,
    name: k.name,
    report: k.report ?? s.report,
    filter: k.filter ?? s.filter,
    completedSince: k.completedSince ?? s.completedSince,
    columns: k.columns ?? s.columns,
  };
}

// The automatic setup: the UDA, and Kanban's report, filter and columns using it.

export const PROGRESS_VALUES = ['inprogress', 'review'];
export const AUTO_REPORT = 'all';
/** Leaves out deleted and waiting tasks and recurring templates, which `all` (no filter of its own) shows. */
export const AUTO_FILTER = '( +PENDING or +COMPLETED )';
export const AUTO_COMPLETED_SINCE = 'today-14d';

/** Taskwarrior's own attributes, which a UDA can't be named. */
const ATTRIBUTES = new Set(
  'annotations depends description due end entry id imask last mask modified parent priority project recur scheduled start status tags until urgency uuid wait'.split(
    ' ',
  ),
);

/** Why `name` can't be the kanban's UDA, or null. */
export function udaNameProblem(name: string): string | null {
  if (!name) return 'needs a UDA name';
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) return 'a UDA name is letters, digits and _, starting with a letter';
  if (ATTRIBUTES.has(name)) return `${name} is one of Taskwarrior's own attributes`;
  return null;
}

/** `ok`: as the kanban needs it. `different`: there, but not of type string or without the values it uses. */
export type Part = 'ok' | 'missing' | 'different';

export interface UdaStatus {
  part: Part;
  type: string | null;
  values: string[];
}

export function udaStatus(config: ConfigResponse | null, name: string): UdaStatus {
  const uda = config?.config.udas?.[name];
  if (!uda) return { part: 'missing', type: null, values: [] };
  // No values allows any; values that leave one of ours out refuse it.
  const fits = uda.type === 'string' && (!uda.values.length || PROGRESS_VALUES.every((v) => uda.values.includes(v)));
  return { part: fits ? 'ok' : 'different', type: uda.type, values: uda.values };
}

/** The `config` lines (name, value) that define the UDA as the kanban needs it, leaving alone what is already so. */
export function setupLines(config: ConfigResponse | null, name: string): [string, string][] {
  const s = udaStatus(config, name);
  if (s.part === 'ok') return [];
  const lines: [string, string][] = [];
  if (s.type !== 'string') lines.push([`uda.${name}.type`, 'string']);
  if (!config?.config.udas?.[name]?.label) lines.push([`uda.${name}.label`, 'Progress']);
  // Values already there are kept, ours added after them.
  const values = [...s.values.filter((v) => v !== ''), ...PROGRESS_VALUES.filter((v) => !s.values.includes(v))];
  if (s.values.length || s.type !== 'string' || s.part === 'missing')
    lines.push([`uda.${name}.values`, values.join(',')]);
  return lines;
}

/**
 * The report the automatic setup can add to the taskrc, so a kanban needs no filter of its own: open tasks and finished
 * ones (no deleted or waiting tasks, no recurring templates), the most urgent first. Completed since still applies.
 */
export const KANBAN_REPORT = 'kanban';
export const KANBAN_REPORT_LINES: [string, string][] = [
  [`report.${KANBAN_REPORT}.description`, 'Open tasks and the ones finished lately, for Kanban'],
  [`report.${KANBAN_REPORT}.columns`, 'id,priority,project,tags,due.relative,scheduled.countdown,description'],
  [`report.${KANBAN_REPORT}.labels`, 'ID,P,Project,Tags,Due,Sch,Description'],
  [`report.${KANBAN_REPORT}.filter`, AUTO_FILTER],
  [`report.${KANBAN_REPORT}.sort`, 'urgency-'],
];

/** The lines that add the `kanban` report, or none when the taskrc has a report of that name already (it is used as it is). */
export function reportLines(config: ConfigResponse | null): [string, string][] {
  return config?.config.reports?.[KANBAN_REPORT] ? [] : KANBAN_REPORT_LINES;
}

/** Whether tasksh's review is set up enough for a task to go back in its queue: the `reviewed` date UDA is defined. */
export function hasReview(config: ConfigResponse | null): boolean {
  return config?.config.udas?.reviewed?.type === 'date';
}

/** The automatic columns, on the UDA `u`. With tasksh's review set up, a task moved to Review goes back in its queue. */
export function automaticColumns(review: boolean, u = PROGRESS_UDA): KanbanColumn[] {
  return [
    { name: 'To do', filter: `status:pending ${u}:`, move: `modify status:pending ${u}:` },
    { name: 'In progress', filter: `status:pending ${u}:inprogress`, move: `modify status:pending ${u}:inprogress` },
    {
      name: 'Review',
      filter: `status:pending ${u}:review`,
      move: `modify status:pending ${u}:review${review ? ' reviewed:' : ''}`,
    },
    { name: 'Done', filter: 'status:completed', move: 'done' },
  ];
}

/**
 * Kanban as the automatic setup leaves it: on, its settings on the UDA `uda`, and a first kanban with none of its own. With
 * `ownReport` it is drawn from the `kanban` report, whose filter is enough; else from `all` with a filter of its own.
 */
export function automaticSettings(s: KanbanSettings, review: boolean, uda: string, ownReport = false): KanbanSettings {
  return {
    ...s,
    enabled: true,
    setup: 'automatic',
    uda,
    report: ownReport ? KANBAN_REPORT : AUTO_REPORT,
    filter: ownReport ? '' : AUTO_FILTER,
    completedSince: AUTO_COMPLETED_SINCE,
    columns: automaticColumns(review, uda),
    kanbans: s.kanbans.length ? s.kanbans : [{ id: 'kanban', name: 'Kanban' }],
  };
}

/** A new kanban named `name`, with nothing of its own. */
export function newKanban(name: string, taken: Iterable<string>): KanbanEntry {
  return { id: freshId(name || 'kanban', taken), name };
}

/**
 * `k` renamed to `name`. A kanban not saved yet whose address is still the one its name made follows the new name, as a
 * board's is made from its name; a saved one, or an address typed by hand, stays. `others` are the other kanbans' slugs.
 */
export function renamedKanban(k: KanbanEntry, name: string, saved: boolean, others: string[]): KanbanEntry {
  const follows = !saved && k.id === newKanban(k.name, others).id;
  return { ...k, name, ...(follows ? { id: newKanban(name, others).id } : {}) };
}

// Reading `settings.json`.

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const text = (v: unknown) => (typeof v === 'string' ? v : '');

/** Why `slug` can't be the address of a kanban, or null; `others` are the other kanbans' slugs. */
export function kanbanSlugProblem(slug: string, others: Iterable<string>): string | null {
  if (!slug) return 'needs an address';
  if (!SLUG.test(slug)) return 'an address is lower case letters a to z, digits, - and _, up to 40';
  for (const o of others) if (o === slug) return `there is already a kanban at /kanban/${slug}`;
  return null;
}

/** What is wrong with a column, or null. */
export function columnProblem(c: KanbanColumn): string | null {
  if (!c.name.trim()) return 'a column needs a name';
  if (!c.filter.trim()) return 'a column needs a filter';
  return c.move.trim() ? commandProblem(c.move) : null;
}

/** What is wrong with a kanban as it is drawn (not its columns), or null; `others` are the other kanbans' slugs. */
export function kanbanProblem(k: Kanban, others: Iterable<string>): string | null {
  if (!k.name.trim()) return 'needs a name';
  if (!k.report.trim()) return "needs a report, its own or Kanban's";
  return kanbanSlugProblem(k.id, others);
}

/** The columns in `v`, or what is wrong with the first that can't be used. */
function readColumns(v: unknown[]): { columns: KanbanColumn[] } | { error: string } {
  const columns: KanbanColumn[] = [];
  for (const [j, c] of v.entries()) {
    if (!isObject(c)) return { error: `column ${j + 1} must be an object` };
    const col = { name: text(c.name), filter: text(c.filter), move: text(c.move) };
    const wrong = columnProblem(col);
    if (wrong) return { error: `column ${j + 1}${col.name ? ` (${col.name})` : ''}: ${wrong}` };
    columns.push(col);
  }
  return { columns };
}

/**
 * The valid part of a saved `kanban`, and what is wrong with the rest, one line each. A kanban with a problem is left
 * out, and Kanban's columns are none when one of them has one (they stay in the file).
 */
export function parseKanban(v: unknown): { kanban: KanbanSettings; errors: string[] } {
  const kanban = emptyKanbanSettings();
  const errors: string[] = [];
  if (v === undefined) return { kanban, errors };
  if (!isObject(v)) return { kanban, errors: ['kanban must be an object: { "enabled": true, "kanbans": [ … ] }'] };
  if (typeof v.enabled === 'boolean') kanban.enabled = v.enabled;
  else if (v.enabled !== undefined) errors.push('kanban.enabled must be true or false');
  if (v.setup === 'automatic' || v.setup === 'manual') kanban.setup = v.setup;
  else if (v.setup !== undefined) errors.push('kanban.setup must be "automatic" or "manual"');
  if (v.uda !== undefined) {
    const bad = typeof v.uda === 'string' ? udaNameProblem(v.uda) : 'must be a UDA name';
    if (bad) errors.push(`kanban.uda: ${bad}`);
    else kanban.uda = v.uda as string;
  }
  for (const key of ['report', 'filter', 'completedSince'] as const) {
    if (typeof v[key] === 'string') kanban[key] = v[key];
    else if (v[key] !== undefined) errors.push(`kanban.${key} must be text`);
  }
  if (Array.isArray(v.columns)) {
    const read = readColumns(v.columns);
    if ('error' in read) errors.push(`kanban.columns: ${read.error}`);
    else kanban.columns = read.columns;
  }
  if (v.kanbans === undefined) return { kanban, errors };
  if (!Array.isArray(v.kanbans)) return { kanban, errors: [...errors, 'kanban.kanbans must be a list'] };
  const seen: string[] = [];
  v.kanbans.forEach((k, i) => {
    const at = `kanban.kanbans[${i}]`;
    if (!isObject(k)) return void errors.push(`${at}: must be an object`);
    const one: KanbanEntry = { id: text(k.id), name: text(k.name) };
    const named = `${at}${one.id ? ` (${one.id})` : ''}`;
    for (const key of ['report', 'filter', 'completedSince'] as const) {
      if (typeof k[key] === 'string') one[key] = k[key];
      else if (k[key] !== undefined) return void errors.push(`${named}: ${key} must be text`);
    }
    if (k.columns !== undefined) {
      if (!Array.isArray(k.columns)) return void errors.push(`${named}: columns must be a list`);
      const read = readColumns(k.columns);
      if ('error' in read) return void errors.push(`${named}: ${read.error}`);
      one.columns = read.columns;
    }
    const bad = kanbanProblem(resolveKanban(kanban, one), seen);
    if (bad) return void errors.push(`${named}: ${bad}`);
    seen.push(one.id);
    kanban.kanbans.push(one);
  });
  return { kanban, errors };
}

const columnsJson = (columns: KanbanColumn[]) => columns.map((c) => ({ name: c.name, filter: c.filter, move: c.move }));

/** `kanban` as it is written to `settings.json`. */
export function kanbanJson(k: KanbanSettings): Record<string, unknown> {
  const out: Record<string, unknown> = { enabled: k.enabled };
  if (k.setup) out.setup = k.setup;
  out.uda = k.uda;
  out.report = k.report;
  out.filter = k.filter;
  out.completedSince = k.completedSince;
  out.columns = columnsJson(k.columns);
  out.kanbans = k.kanbans.map((b) => {
    const one: Record<string, unknown> = { id: b.id, name: b.name };
    for (const key of ['report', 'filter', 'completedSince'] as const) if (b[key] !== undefined) one[key] = b[key];
    if (b.columns) one.columns = columnsJson(b.columns);
    return one;
  });
  return out;
}

/**
 * The Lucide icon a card shows for one of Taskwarrior's own fields, by the field's name (`due`, whatever the report's
 * format or label), when Appearance's `cardIcons` is on. None for a UDA or a field without one: that is a badge, its label then
 * its value.
 */
export const FIELD_ICONS: Readonly<Record<string, string>> = {
  id: 'hash',
  uuid: 'fingerprint-pattern',
  status: 'circle-dot',
  priority: 'flag',
  project: 'folder',
  tags: 'tag',
  due: 'clock',
  scheduled: 'calendar',
  wait: 'hourglass',
  until: 'calendar-x',
  entry: 'calendar-plus',
  modified: 'pencil',
  start: 'play',
  end: 'circle-check',
  recur: 'repeat',
  depends: 'link',
  urgency: 'zap',
  parent: 'git-branch',
};

export const fieldIcon = (name: string): string | null => FIELD_ICONS[name] ?? null;

/** A free-text value up to this long, on one line and plain, is a pill like any other UDA's. */
export const SHORT_TEXT = 20;

/**
 * How a card shows a field: one of Taskwarrior's as it is (`field`); a UDA as a pill of its label and value (`pill`),
 * unless it is a string free to hold anything holding more than a few words (`text`): several lines, `SHORT_TEXT`
 * characters or more, or Markdown (`markdown`, so its checkboxes can be ticked), which is a paragraph of its own. A
 * string UDA with `values` is one of a few words, so a pill.
 */
export function cardShape(
  name: string,
  udas: Record<string, { type: string; values: string[] }>,
  text = '',
  markdown = false,
): 'field' | 'pill' | 'text' {
  const def = udas[name];
  if (!def) return 'field';
  if (def.type !== 'string' || def.values.length > 0) return 'pill';
  const long = markdown || text.includes('\n') || [...text.trim()].length >= SHORT_TEXT;
  return long ? 'text' : 'pill';
}

// Reading a kanban's tasks.

/** What the page shows a kanban with: the saved report, filter and Completed since, or what was typed over them. */
export interface KanbanView {
  report: string;
  filter: string;
  completedSince: string;
}

/** The filter every column adds to: the kanban's (or the page's) filter and Completed since, each in brackets. */
export function baseFilter(v: KanbanView): string[] {
  const parts: string[] = [];
  if (v.filter.trim()) parts.push(`( ${v.filter.trim()} )`);
  if (v.completedSince.trim()) parts.push(`( -COMPLETED or end.after:${shellQuote(v.completedSince.trim())} )`);
  return parts;
}

/**
 * The command line that reads a column (or, with `column` null, every task the kanban's report and filter pick, for
 * Other): the filters, each in brackets so an `or` in one can't spill into another, then the report.
 */
export function columnArgs(v: KanbanView, column: KanbanColumn | null): string[] {
  const parts = baseFilter(v);
  if (column?.filter.trim()) parts.push(`( ${column.filter.trim()} )`);
  return [...splitWords(parts.join(' ')), v.report];
}

/**
 * Each task in the first column that has it, in the report's order; `all` (every task the kanban picks) less those is
 * Other.
 */
export function placeTasks(columns: Row[][], all: Row[]): { columns: Row[][]; other: Row[] } {
  const placed = new Set<string>();
  const out = columns.map((rows) =>
    rows.filter((r) => {
      if (placed.has(r.uuid)) return false;
      placed.add(r.uuid);
      return true;
    }),
  );
  return { columns: out, other: all.filter((r) => !placed.has(r.uuid)) };
}

/**
 * Swimlanes, from the report's own groups (a `/` in its sort, `project+/`): `heads` is the group heading above each of
 * `all`'s rows, in the report's order (`groupHeads`), null where a row continues the group above. A lane for each heading,
 * in the order they first come (a heading met again, when the groups aren't the first sort key, is the same lane), and
 * the lane each task is in. No lanes when the report has no groups.
 */
export function lanesOf(all: Row[], heads: (string | null)[]): { lanes: string[]; laneOf: Map<string, number> } {
  const lanes: string[] = [];
  const laneOf = new Map<string, number>();
  if (!heads.length) return { lanes, laneOf };
  let at = -1;
  all.forEach((r, i) => {
    const h = heads[i];
    if (h != null) {
      at = lanes.indexOf(h);
      if (at < 0) at = lanes.push(h) - 1;
    }
    laneOf.set(r.uuid, Math.max(at, 0));
  });
  return { lanes, laneOf };
}

/** Which column has the task (`-1`: Other or none). */
export function columnOf(columns: Row[][], uuid: string): number {
  return columns.findIndex((rows) => rows.some((r) => r.uuid === uuid));
}

/**
 * The statements a move runs on one task: the column's line with the task's uuid in front of each statement, and the
 * task commands in it written out (a task command keeps the uuid in front of each of its own statements).
 */
/** A move changes the card's task only: a recurring task's instance, without asking about the rest of its series. */
export const ONLY_THIS_TASK = 'rc.recurrence.confirmation:no';

export function moveSteps(
  move: string,
  uuid: string,
  commands: TaskCommand[],
): { args: string[]; join: Joiner | null }[] {
  const line = splitChain(move)
    .map((s, i) => (i ? ` ${s.join} ` : '') + `${ONLY_THIS_TASK} ${uuid} ${s.line}`)
    .join('');
  return planChain(line, commands).map((s) => ({ args: splitWords(s.line), join: s.join }));
}

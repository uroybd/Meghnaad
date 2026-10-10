// Pure helpers for the bulk actions: how tasks are named to the engine, and the fields the Modify editor offers.

import type { UdaDef } from './types';

/** A task as the bulk actions need it. */
export interface Pick {
  uuid: string;
  id: number | null;
}

/**
 * Task numbers as a short list (`1,3-5,9`). Taskwarrior reads the same list back, and a long selection stays short
 * enough to see.
 */
export function idList(ids: number[]): string {
  const sorted = [...new Set(ids)].sort((a, b) => a - b);
  const parts: string[] = [];
  for (let i = 0; i < sorted.length;) {
    let j = i;
    while (j + 1 < sorted.length && sorted[j + 1] === sorted[j] + 1) j++;
    parts.push(j - i >= 2 ? `${sorted[i]}-${sorted[j]}` : sorted.slice(i, j + 1).join(','));
    i = j + 1;
  }
  return parts.join(',');
}

/**
 * The words that name these tasks at the prompt: their numbers as one list, then the whole uuid of each one without a
 * number (a finished task has none). Side by side they pick all of them, as in Taskwarrior.
 */
export function namedWords(tasks: Pick[]): string[] {
  const ids = tasks.flatMap((t) => (t.id != null ? [t.id] : []));
  const words = ids.length ? [idList(ids)] : [];
  for (const t of tasks) if (t.id == null) words.push(t.uuid);
  return words;
}

/**
 * The filter for a command run by a button. The whole uuid of every task: the numbers can change between the click
 * and the command (another device finished a task), a uuid cannot.
 */
export function filterWords(tasks: Pick[]): string[] {
  return tasks.map((t) => t.uuid);
}

/** What the prompt starts with after **Command**: the tasks named, and a space to carry on from. */
export function promptFor(tasks: Pick[]): string {
  return `${namedWords(tasks).join(' ')} `;
}

export type FieldKind = 'text' | 'priority' | 'date' | 'project' | 'tags' | 'uda';

export interface Field {
  /** What is sent: `project`, `due`, ... for a value; `+` and `-` for tags added and taken off. */
  key: string;
  label: string;
  kind: FieldKind;
  hint: string;
  /** A value is needed (an empty one is not allowed): tags to add or remove. */
  required?: boolean;
  /** Values the field takes, for a choice. */
  values?: string[];
}

const DATE_HINT = 'a date: tomorrow, eow, 2026-12-25, 3d (empty clears it)';

/** The fields the Modify editor offers, then the taskrc's UDAs. */
export function fieldsFor(udas: Record<string, UdaDef>): Field[] {
  const core: Field[] = [
    { key: 'project', label: 'Project', kind: 'project', hint: 'a project name (empty clears it)' },
    {
      key: 'priority',
      label: 'Priority',
      kind: 'priority',
      hint: 'H, M or L (empty clears it)',
      values: ['H', 'M', 'L'],
    },
    { key: 'due', label: 'Due', kind: 'date', hint: DATE_HINT },
    { key: 'wait', label: 'Wait', kind: 'date', hint: DATE_HINT },
    { key: 'scheduled', label: 'Scheduled', kind: 'date', hint: DATE_HINT },
    { key: 'until', label: 'Until', kind: 'date', hint: DATE_HINT },
    { key: 'recur', label: 'Repeat', kind: 'text', hint: 'daily, weekly, 3d, ... (empty clears it)' },
    { key: '+', label: 'Add tags', kind: 'tags', hint: 'tags to add, separated by spaces', required: true },
    { key: '-', label: 'Remove tags', kind: 'tags', hint: 'tags to take off, separated by spaces', required: true },
    { key: 'depends', label: 'Depends on', kind: 'text', hint: 'task numbers or uuids (empty clears them)' },
  ];
  const custom: Field[] = Object.values(udas).map((u) => ({
    key: u.name,
    label: u.label || u.name,
    kind: 'uda',
    hint: u.values.length ? `one of ${u.values.join(', ')} (empty clears it)` : `${u.type} (empty clears it)`,
    values: u.values.length ? u.values : undefined,
  }));
  return [...core, ...custom];
}

export interface Entry {
  key: string;
  value: string;
}

/** Why an entry cannot be applied, or `null` when it can. */
export function problem(e: Entry, fields: Field[]): string | null {
  const f = fields.find((x) => x.key === e.key);
  if (!f) return 'Choose a field.';
  if (f.required && !e.value.trim()) return `${f.label}: say which.`;
  return null;
}

/** The words `modify` takes for these entries, in order. A tag entry makes one word per tag. */
export function modifyWords(entries: Entry[]): string[] {
  const words: string[] = [];
  for (const e of entries) {
    if (e.key === '+' || e.key === '-') {
      for (const t of e.value.split(/[\s,]+/).filter(Boolean)) words.push(`${e.key}${t.replace(/^[+-]/, '')}`);
    } else {
      words.push(`${e.key}:${e.value.trim()}`);
    }
  }
  return words;
}

/** What Apply will do, in words: "set project to Home, clear due". */
export function describe(entries: Entry[], fields: Field[]): string {
  const label = (key: string) => fields.find((f) => f.key === key)?.label.toLowerCase() ?? key;
  return entries
    .map((e) => {
      const v = e.value.trim();
      if (e.key === '+')
        return `add ${v
          .split(/[\s,]+/)
          .filter(Boolean)
          .map((t) => `+${t.replace(/^[+-]/, '')}`)
          .join(' ')}`;
      if (e.key === '-')
        return `remove ${v
          .split(/[\s,]+/)
          .filter(Boolean)
          .map((t) => `+${t.replace(/^[+-]/, '')}`)
          .join(' ')}`;
      return v ? `set ${label(e.key)} to ${v}` : `clear ${label(e.key)}`;
    })
    .join(', ');
}

/**
 * Tasks to a command at a time. The Worker takes a command of at most 200 words and 8 KB, and a uuid is 36
 * characters, so a long selection goes in several commands rather than being refused.
 */
export const BATCH = 120;

export function batches<T>(items: T[], size = BATCH): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size));
  return out;
}

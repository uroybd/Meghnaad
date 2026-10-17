// What a line typed into the quick add would make, and the same broken into the fields of the full form. The reading of the
// line is the engine's (`tc_core::preview`, through src/engine/helpers.ts): the same `parse_mods` and `plan` that `add` runs,
// so the card shows what will be made and the form starts from it.

import { callHelper } from '../engine/helpers';
import { tzOffsetSeconds } from './api';
import { fromParts, toParts } from './dates';
import { splitWords } from './cmdline';
import type { ConfigResponse, UdaDef } from './types';

/** `tc_core::preview::Preview`: the task a line would add. Dates are seconds since 1970, by attribute name. */
export interface AddPreview {
  description: string;
  project: string | null;
  priority: string | null;
  tags: string[];
  dates: Record<string, number>;
  recur: string | null;
  /** Task ids and uuids as typed. */
  depends: string[];
  /** By UDA name; one of type date is a timestamp. */
  udas: Record<string, string>;
  /** What is wrong with the line, one message for each part left out. */
  errors: string[];
}

/** The task `line` would add (what comes after `task add`), now and in the user's zone. */
export function previewAdd(line: string, config: ConfigResponse['config'] | undefined | null): AddPreview {
  return callHelper<AddPreview>('add_preview', line, config ?? {}, Math.floor(Date.now() / 1000), tzOffsetSeconds());
}

/** The arguments `task add` is run with for a line: its words, the way the engine reads them. */
export const addArgs = (line: string): string[] => ['add', ...splitWords(line)];

/** The fields of the full form, as it holds them (dates as `YYYY-MM-DD[THH:MM]`, tags as one text). */
export interface FormSeed {
  description: string;
  project: string;
  priority: string;
  due: string;
  wait: string;
  scheduled: string;
  until: string;
  tags: string;
  recur: string;
  /** Task ids, space separated. */
  depends: string;
  udas: Record<string, string>;
  /** Attributes the form has no field for (`start:…`), sent as they are. */
  carried: string[];
}

const wire = (ts: number | undefined) => fromParts(toParts(ts ?? null));
const FIELDS = new Set(['due', 'wait', 'scheduled', 'until']);

/** The form's fields for a preview; with none, for a bare description (or nothing). */
export function seedOf(
  p: AddPreview | null | undefined,
  description = '',
  defs: Record<string, UdaDef> = {},
): FormSeed {
  const udas: Record<string, string> = {};
  for (const [name, value] of Object.entries(p?.udas ?? {})) {
    udas[name] = defs[name]?.type === 'date' ? wire(Number(value)) : value;
  }
  return {
    description: p?.description ?? description,
    project: p?.project ?? '',
    priority: p?.priority ?? '',
    due: wire(p?.dates.due),
    wait: wire(p?.dates.wait),
    scheduled: wire(p?.dates.scheduled),
    until: wire(p?.dates.until),
    tags: (p?.tags ?? []).join(' '),
    recur: p?.recur ?? '',
    depends: (p?.depends ?? []).join(' '),
    udas,
    carried: Object.entries(p?.dates ?? {})
      .filter(([name]) => !FIELDS.has(name))
      .map(([name, ts]) => `${name}:${wire(ts)}`),
  };
}

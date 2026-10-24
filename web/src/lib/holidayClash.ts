// A due or scheduled date that falls on a holiday: what is the engine's (`tc_core::clash`, through src/engine/helpers.ts), the
// same on every front end. A date is resolved first (`eod`, `som`, `eow-2d`), in the user's zone, then looked up in the
// `holiday.*` settings.

import { callHelper } from '../engine/helpers';
import { tzOffsetSeconds } from './api';
import { store } from './store.svelte';

/** A date on a holiday: the moment it resolves to, the holidays of that day, and what to say about it. */
export interface Clash {
  date: number;
  names: string[];
  message: string;
}

/** A date word of a command line, `[start, end)` in UTF-16 units (as the highlighter counts), that is on a holiday. */
export interface ClashMark extends Clash {
  start: number;
  end: number;
  attr: 'due' | 'scheduled';
}

/** Holidays are defined: nothing is asked of the engine (the settings go with each question) when there are none. */
function defined(): boolean {
  const settings = store.config?.config.settings;
  return !!settings && Object.keys(settings).some((k) => k.startsWith('holiday.'));
}

const now = () => Math.floor(Date.now() / 1000);

/** Whether `attr` (`due` or `scheduled`) set to `value` (a date, or an expression) is on a holiday. */
export function clashOf(attr: 'due' | 'scheduled', value: string): Clash | null {
  if (!value.trim() || !defined()) return null;
  return callHelper<Clash | null>('holiday_clash', attr, value, store.config?.config ?? {}, now(), tzOffsetSeconds());
}

/**
 * The date words of a command line that are on a holiday: what `add`, `log`, `modify` and `duplicate` would set. With
 * `assumeAdd` the line is what comes after `add` (the quick add's).
 */
export function clashMarks(line: string, commands: ReadonlySet<string>, assumeAdd = false): ClashMark[] {
  if (!line.includes(':') || !defined()) return [];
  return callHelper<ClashMark[]>(
    'holiday_marks',
    line,
    [...commands],
    assumeAdd,
    store.config?.config ?? {},
    now(),
    tzOffsetSeconds(),
  );
}

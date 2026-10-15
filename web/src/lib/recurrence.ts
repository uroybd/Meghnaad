// Recurrence periods as Taskwarrior writes them (`recur:weekly`, `recur:3d`, ISO `P2W`).
// The server validates; this only offers sensible choices and describes what is stored.

import { callHelper } from '../engine/helpers';

export interface Preset {
  value: string;
  label: string;
}

export const PRESETS: Preset[] = [
  { value: 'daily', label: 'Every day' },
  { value: 'weekdays', label: 'Every weekday' },
  { value: 'weekly', label: 'Every week' },
  { value: 'biweekly', label: 'Every 2 weeks' },
  { value: 'monthly', label: 'Every month' },
  { value: 'quarterly', label: 'Every 3 months' },
  { value: 'yearly', label: 'Every year' },
];

/** "every 3 days" for `3d`; unknown spellings come back as typed. The words are the engine's (`tc_core::helpers::describe_recur`). */
export function describeRecur(period: string | null | undefined): string {
  return callHelper<string>('describe_recur', period ?? '');
}

/** The preset a stored period corresponds to exactly, or '' when it needs the custom field. */
export function presetFor(period: string | null | undefined): string {
  const p = (period ?? '').trim();
  return PRESETS.some((x) => x.value === p) ? p : '';
}

/** `recur:` completion candidates, with the plain-English meaning as a hint. */
export const RECUR_WORDS = [
  ...PRESETS.map((p) => ({ value: p.value, hint: p.label.toLowerCase() })),
  { value: '3d', hint: 'every 3 days' },
  { value: '2w', hint: 'every 2 weeks' },
  { value: '2m', hint: 'every 2 months' },
];

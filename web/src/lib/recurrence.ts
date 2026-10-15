// Recurrence periods as Taskwarrior writes them (`recur:weekly`, `recur:3d`, ISO `P2W`).
// The server validates; this only offers sensible choices and describes what is stored.

import { callHelper } from '../engine/helpers';

export interface Preset {
  value: string;
  label: string;
}

/** The periods a screen offers, with their labels: the engine's list (`tc_core::complete::RECUR_PRESETS`), which `recur:` completes too. */
export const PRESETS: Preset[] = callHelper<Preset[]>('recur_presets');

/** "every 3 days" for `3d`; unknown spellings come back as typed. The words are the engine's (`tc_core::helpers::describe_recur`). */
export function describeRecur(period: string | null | undefined): string {
  return callHelper<string>('describe_recur', period ?? '');
}

/** The preset a stored period corresponds to exactly, or '' when it needs the custom field. */
export function presetFor(period: string | null | undefined): string {
  const p = (period ?? '').trim();
  return PRESETS.some((x) => x.value === p) ? p : '';
}

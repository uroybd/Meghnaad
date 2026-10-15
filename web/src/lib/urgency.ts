// Urgency settings as the Urgency dialog shows them. The key spelling and what each key applies to follow Taskwarrior's
// `urgency.*` settings, and the rules for a name are the ones the engine applies when it saves (`tc_core::urgency_settings`,
// through src/engine/helpers.ts), so a screen cannot offer what would be refused.

import { callHelper } from '../engine/helpers';

export type Kind = 'term' | 'project' | 'tag' | 'keyword' | 'uda';

export interface Described {
  key: string;
  kind: Kind;
  /** The part of the key the user chose: a project, tag, keyword or UDA (and value). */
  name: string;
  label: string;
  hint: string;
}

/** The built-in terms, in the order Taskwarrior lists them. */
export const TERMS: { term: string; label: string; hint: string }[] = callHelper('urgency_terms');

export const AGE_MAX = 'urgency.age.max';
export const INHERIT_KEY = 'urgency.inherit';

export function describe(key: string): Described {
  return callHelper<Described>('describe_urgency_key', key);
}

/** What can be added beyond the built-in terms. */
export const ADDABLE: { kind: Exclude<Kind, 'term'>; label: string; placeholder: string }[] =
  callHelper('urgency_addable');

/** The rule the engine applies to names: no spaces, `=`, `#`, and not containing ".coefficient". */
export function validName(name: string): boolean {
  return callHelper<boolean>('valid_urgency_name', name);
}

export function buildKey(kind: Exclude<Kind, 'term'>, name: string): string {
  return callHelper<string>('build_urgency_key', kind, name);
}

/** UDA names are plain identifiers; the value after the first dot is free text. */
export function validUda(name: string): boolean {
  return callHelper<boolean>('valid_urgency_uda', name);
}

/** The order rows appear in: built-in terms first, then custom ones grouped by kind. */
export function sortKeys(keys: Iterable<string>): string[] {
  return callHelper<string[]>('sort_urgency_keys', [...keys]);
}

/** The value in force: the saved override, else Taskwarrior's built-in one. */
export function effective(
  key: string,
  overrides: Record<string, number>,
  defaults: Record<string, number>,
): number | undefined {
  return overrides[key] ?? defaults[key];
}

/**
 * Record `value` for `key`. A value equal to the built-in one is not an override, so it is dropped
 * and the setting follows the default again.
 */
export function withValue(
  overrides: Record<string, number>,
  defaults: Record<string, number>,
  key: string,
  value: number,
): Record<string, number> {
  return callHelper<Record<string, number>>('urgency_with_value', overrides, defaults, key, value);
}

export function sameOverrides(a: Record<string, number>, b: Record<string, number>): boolean {
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => k in b && a[k] === b[k]);
}

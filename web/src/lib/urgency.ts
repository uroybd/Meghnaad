// Urgency settings as the Urgency dialog shows them. The key spelling and what each key applies to
// follow Taskwarrior's `urgency.*` settings; the Worker re-checks every key before saving.

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
export const TERMS: { term: string; label: string; hint: string }[] = [
  { term: 'project', label: 'Has a project', hint: 'The task belongs to a project.' },
  { term: 'active', label: 'Is started', hint: 'The task has been started and is active.' },
  { term: 'scheduled', label: 'Scheduled date has passed', hint: 'The scheduled date is in the past.' },
  { term: 'waiting', label: 'Is waiting', hint: 'The wait date is still in the future.' },
  { term: 'blocked', label: 'Is blocked', hint: 'Depends on a task that is not done yet.' },
  { term: 'annotations', label: 'Has annotations', hint: 'Scales with the count: 1 → 0.8, 2 → 0.9, 3 or more → 1.0.' },
  { term: 'tags', label: 'Has tags', hint: 'Scales with the count: 1 → 0.8, 2 → 0.9, 3 or more → 1.0.' },
  { term: 'due', label: 'Due date', hint: 'Rises as the date nears: 0.2 two weeks out, 1.0 a week overdue.' },
  { term: 'blocking', label: 'Blocks another task', hint: 'Another pending task depends on this one.' },
  { term: 'age', label: 'Age', hint: 'Grows from 0 to 1 over the days set by the age cap below.' },
];

export const AGE_MAX = 'urgency.age.max';
export const INHERIT_KEY = 'urgency.inherit';

const SUFFIX = '.coefficient';

export function describe(key: string): Described {
  const rest = key.replace(/^urgency\./, '');
  if (key === AGE_MAX) {
    return { key, kind: 'term', name: 'age.max', label: 'Age cap (days)', hint: 'A task this old or older gets the full age value.' };
  }
  const body = rest.endsWith(SUFFIX) ? rest.slice(0, -SUFFIX.length) : rest;
  const term = TERMS.find((t) => t.term === body);
  if (term) return { key, kind: 'term', name: term.term, label: term.label, hint: term.hint };

  const user = (prefix: string) => (body.startsWith(prefix) ? body.slice(prefix.length) : null);
  const project = user('user.project.');
  if (project !== null) {
    return {
      key, kind: 'project', name: project, label: `Project ${project}`,
      hint: `Applies to ${project} and every sub-project (${project}.*).`,
    };
  }
  const tag = user('user.tag.');
  if (tag !== null) {
    return { key, kind: 'tag', name: tag, label: `Tag +${tag}`, hint: `Tasks tagged ${tag}. Virtual tags such as OVERDUE work too.` };
  }
  const keyword = user('user.keyword.');
  if (keyword !== null) {
    return { key, kind: 'keyword', name: keyword, label: `Keyword “${keyword}”`, hint: 'The description contains this text (case-sensitive).' };
  }
  const uda = user('uda.') ?? (body.startsWith('uda.') ? body.slice(4) : null);
  if (uda !== null) {
    const [name, ...value] = uda.split('.');
    return value.length
      ? { key, kind: 'uda', name: uda, label: `${name} = ${value.join('.')}`, hint: `The ${name} attribute is exactly ${value.join('.')}.` }
      : { key, kind: 'uda', name: uda, label: `${name} has a value`, hint: `The ${name} attribute is set, to anything.` };
  }
  return { key, kind: 'term', name: rest, label: key, hint: '' };
}

/** What can be added beyond the built-in terms. */
export const ADDABLE: { kind: Exclude<Kind, 'term'>; label: string; placeholder: string }[] = [
  { kind: 'project', label: 'Project', placeholder: 'Home' },
  { kind: 'tag', label: 'Tag', placeholder: 'next' },
  { kind: 'keyword', label: 'Keyword', placeholder: 'urgent' },
  { kind: 'uda', label: 'UDA (any value)', placeholder: 'estimate' },
  { kind: 'uda', label: 'UDA = value', placeholder: 'estimate.huge' },
];

/** The same rule the Worker applies to names: no spaces, `=`, `#`, and not containing ".coefficient". */
export function validName(name: string): boolean {
  return name.length > 0 && !name.includes(SUFFIX) && !/[\s=#\u0000-\u001f]/.test(name);
}

export function buildKey(kind: Exclude<Kind, 'term'>, name: string): string {
  const prefix = { project: 'urgency.user.project.', tag: 'urgency.user.tag.', keyword: 'urgency.user.keyword.', uda: 'urgency.uda.' }[kind];
  return `${prefix}${name}${SUFFIX}`;
}

/** UDA names are plain identifiers; the value after the first dot is free text. */
export function validUda(name: string): boolean {
  const [uda, ...value] = name.split('.');
  return /^[A-Za-z0-9_]+$/.test(uda) && (value.length === 0 || validName(value.join('.')));
}

/** The order rows appear in: built-in terms first, then custom ones grouped by kind. */
export function sortKeys(keys: Iterable<string>): string[] {
  const rank = (k: string) => {
    const d = describe(k);
    if (k === AGE_MAX) return TERMS.length;
    if (d.kind === 'term') return TERMS.findIndex((t) => t.term === d.name);
    return 100 + ['project', 'tag', 'keyword', 'uda'].indexOf(d.kind);
  };
  return [...keys].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
}

/** The value in force: the saved override, else Taskwarrior's built-in one. */
export function effective(key: string, overrides: Record<string, number>, defaults: Record<string, number>): number | undefined {
  return overrides[key] ?? defaults[key];
}

/**
 * Record `value` for `key`. A value equal to the built-in one is not an override, so it is dropped
 * and the setting follows the default again.
 */
export function withValue(overrides: Record<string, number>, defaults: Record<string, number>, key: string, value: number): Record<string, number> {
  const next = { ...overrides };
  if (key in defaults && defaults[key] === value) delete next[key];
  else next[key] = value;
  return next;
}

export function sameOverrides(a: Record<string, number>, b: Record<string, number>): boolean {
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => k in b && a[k] === b[k]);
}

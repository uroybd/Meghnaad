// Recurrence periods as Taskwarrior writes them (`recur:weekly`, `recur:3d`, ISO `P2W`).
// The server validates; this only offers sensible choices and describes what is stored.

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

/** Names Taskwarrior accepts that aren't in the presets, for describing and completing. */
const NAMED: Record<string, string> = {
  day: 'every day',
  daily: 'every day',
  weekdays: 'every weekday',
  week: 'every week',
  weekly: 'every week',
  biweekly: 'every 2 weeks',
  fortnight: 'every 2 weeks',
  month: 'every month',
  monthly: 'every month',
  bimonthly: 'every 2 months',
  quarter: 'every 3 months',
  quarterly: 'every 3 months',
  semiannual: 'every 6 months',
  year: 'every year',
  yearly: 'every year',
  annual: 'every year',
  biannual: 'every 2 years',
  biyearly: 'every 2 years',
};

const UNITS: [RegExp, string][] = [
  [/^(d|day|days)$/, 'day'],
  [/^(w|wk|wks|week|weeks)$/, 'week'],
  [/^(mo|mth|mths|month|months)$/, 'month'],
  [/^(q|qtr|qtrs|quarter|quarters)$/, 'quarter'],
  [/^(y|yr|yrs|year|years)$/, 'year'],
];

const ISO = /^P(?:(\d+)Y)?(?:(\d+)M)?(?:(\d+)W)?(?:(\d+)D)?$/i;

/** "every 3 days" for `3d`; unknown spellings come back as typed. */
export function describeRecur(period: string | null | undefined): string {
  const p = (period ?? '').trim();
  if (!p) return '';
  const lower = p.toLowerCase();
  if (NAMED[lower]) return NAMED[lower];
  const m = /^(\d+)\s*([a-z]+)$/.exec(lower);
  if (m) {
    const n = Number(m[1]);
    const unit = UNITS.find(([re]) => re.test(m[2]))?.[1];
    if (unit) return n === 1 ? `every ${unit}` : `every ${n} ${unit}s`;
  }
  const iso = ISO.exec(p);
  if (iso && p.length > 1) {
    const parts = (['Y', 'M', 'W', 'D'] as const).flatMap((u, i) => {
      const n = iso[i + 1];
      if (!n) return [];
      const name = { Y: 'year', M: 'month', W: 'week', D: 'day' }[u];
      return [`${n} ${name}${n === '1' ? '' : 's'}`];
    });
    if (parts.length) return `every ${parts.join(' ')}`;
  }
  return p;
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

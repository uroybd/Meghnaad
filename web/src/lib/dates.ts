// Date/time helpers for inputs that take a date and an *optional* time.
//
// The wire format is the one the server's date parser accepts and the console accepts too:
// `YYYY-MM-DD` (a whole day) or `YYYY-MM-DDTHH:MM` (a moment), both in the user's local zone
// (the request carries the zone's offset).

const pad = (n: number) => String(n).padStart(2, '0');

export interface DateParts {
  date: string; // YYYY-MM-DD or ''
  time: string; // HH:MM or ''
}

/** Epoch seconds -> local date and (only if not midnight) time. */
export function toParts(epoch: number | null | undefined, tzOffsetSec?: number): DateParts {
  if (epoch == null) return { date: '', time: '' };
  const shifted = new Date((epoch + (tzOffsetSec ?? -new Date(epoch * 1000).getTimezoneOffset() * 60)) * 1000);
  const date = `${shifted.getUTCFullYear()}-${pad(shifted.getUTCMonth() + 1)}-${pad(shifted.getUTCDate())}`;
  const h = shifted.getUTCHours();
  const m = shifted.getUTCMinutes();
  const hasTime = h !== 0 || m !== 0 || shifted.getUTCSeconds() !== 0;
  return { date, time: hasTime ? `${pad(h)}:${pad(m)}` : '' };
}

/** Date + optional time -> wire string; '' means "no value" (clears the field). */
export function fromParts(p: DateParts): string {
  if (!p.date) return '';
  return p.time ? `${p.date}T${p.time}` : p.date;
}

export function isValidDate(s: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return false;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const dt = new Date(Date.UTC(y, mo - 1, d));
  return dt.getUTCFullYear() === y && dt.getUTCMonth() === mo - 1 && dt.getUTCDate() === d;
}

export function isValidTime(s: string): boolean {
  return /^([01]\d|2[0-3]):[0-5]\d$/.test(s);
}

/** Display: date, plus the time only when there is one. */
export function formatMoment(epoch: number | null | undefined, tzOffsetSec?: number): string {
  const p = toParts(epoch, tzOffsetSec);
  if (!p.date) return '';
  return p.time ? `${p.date} ${p.time}` : p.date;
}

/** Date and time with seconds, for journal sessions: 2026-10-07 12:30:05 (local). */
export function formatStamp(epoch: number | null | undefined, tzOffsetSec?: number): string {
  if (epoch == null) return '';
  const tz = tzOffsetSec ?? -new Date(epoch * 1000).getTimezoneOffset() * 60;
  const d = new Date((epoch + tz) * 1000);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getUTCFullYear()}-${p(d.getUTCMonth() + 1)}-${p(d.getUTCDate())} ${p(d.getUTCHours())}:${p(d.getUTCMinutes())}:${p(d.getUTCSeconds())}`;
}

/** Exact elapsed time for time tracking: 1h 12m, 45s, 2d 3h. */
export function formatSeconds(total: number): string {
  const s = Math.max(0, Math.floor(total));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (d) return `${d}d ${h}h`;
  if (h) return `${h}h ${m}m`;
  if (m) return `${m}m ${sec}s`;
  return `${sec}s`;
}

/** Compact "vague" duration like Taskwarrior's: 45s, 12min, 5h, 6d, 3w, 4mo, 2y. */
export function compactDuration(seconds: number): string {
  const s = Math.abs(Math.round(seconds));
  const day = 86400;
  let out: string;
  if (s < 60) out = `${s}s`;
  else if (s < 3600) out = `${Math.floor(s / 60)}min`;
  else if (s < day) out = `${Math.floor(s / 3600)}h`;
  else if (s < 14 * day) out = `${Math.floor(s / day)}d`;
  else if (s < 60 * day) out = `${Math.floor(s / (7 * day))}w`;
  else if (s < 365 * day) out = `${Math.floor(s / (30 * day))}mo`;
  else out = `${Math.floor(s / (365 * day))}y`;
  return seconds < 0 ? `-${out}` : out;
}

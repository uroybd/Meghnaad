// When should the app tell you something? Pure logic, no browser APIs, so it can be tested.
//
// It only works while the tab is open, off the tasks it has polled; there is no server push.
// Each notification is identified by a key (task + kind + the time it is about), and is shown at
// most once, so re-polling and reloading never repeat it.

import { callHelper } from '../engine/helpers';
import type { Row } from './types';

/**
 * What the reminders and the timer read of a task, which is all that `_remind` sends (a full `Row` costs the Worker a
 * list of virtual tags, a urgency and a colour for every task).
 */
export type Reminder = Pick<
  Row,
  'uuid' | 'status' | 'description' | 'due' | 'wait' | 'scheduled' | 'start' | 'active_seconds'
> & {
  /** The task's `remind_in` UDA: times before `due` and `scheduled` to remind, as typed (`10,1h`). */
  remind_in?: string | null;
};

export interface NotifySettings {
  enabled: boolean;
  /** Minutes before a timed due or scheduled date to give a heads-up (0 = none), for a task without `remind_in`. */
  leadMinutes: number;
  /** Local hour of day to notify about tasks due on a date with no time (all-day). */
  allDayHour: number;
  /** Notify when a task's `wait` date passes and it reappears. */
  waitOver: boolean;
}

export const DEFAULT_SETTINGS: NotifySettings = { enabled: false, leadMinutes: 15, allDayHour: 9, waitOver: true };

export type Kind = 'soon' | 'scheduled' | 'due' | 'allday' | 'wait' | 'digest';

export interface Notice {
  /** Dedup key. */
  key: string;
  kind: Kind;
  title: string;
  body: string;
  /** Task to open when clicked (none for a digest). */
  uuid: string | null;
}

const HOUR = 3600;
const DAY = 86400;
/** Don't announce "due now" for something that became due long ago; the digest covers those. */
const FRESH = HOUR;

const localDay = (ts: number, tz: number) => Math.floor((ts + tz) / DAY);
export const isAllDay = (due: number, tz: number) => (due + tz) % DAY === 0;

function until(seconds: number): string {
  const m = Math.round(seconds / 60);
  return m <= 1 ? 'in a minute' : m < 60 ? `in ${m} minutes` : `in ${Math.round(m / 60)} hours`;
}

/**
 * The times before `due` and `scheduled` a task's `remind_in` asks for, in seconds, latest first (the engine reads it:
 * minutes, or Taskwarrior durations). `null` means the setting applies; an empty list, no reminders before those dates.
 */
export function remindOffsets(value: string | null | undefined): number[] | null {
  if (value == null) return null;
  return callHelper<number[] | null>('remind_offsets', value);
}

/** Of the moments `offsets` before `at`, the closest one to `at` that has come (as its offset), or null. */
export function reached(offsets: number[], at: number, now: number): number | null {
  if (now >= at) return null;
  let hit: number | null = null;
  for (const o of offsets) if (now >= at - o && (hit == null || o < hit)) hit = o;
  return hit;
}

/** "today", "tomorrow" or "in 3 days" for a date with no time. */
function dayWord(date: number, now: number, tz: number): string {
  const d = localDay(date, tz) - localDay(now, tz);
  return d <= 0 ? 'today' : d === 1 ? 'tomorrow' : `in ${d} days`;
}

/**
 * Notices that should be shown right now.
 * @param rows pending tasks (anything else is ignored)
 * @param seen keys already shown; the caller adds the keys of what it shows
 * @param tz viewer's UTC offset in seconds
 */
export function dueNotices(
  rows: Reminder[],
  now: number,
  s: NotifySettings,
  seen: ReadonlySet<string>,
  tz: number,
  offsetsOf: (value: string | null | undefined) => number[] | null = remindOffsets,
): Notice[] {
  const out: Notice[] = [];
  const add = (n: Notice) => {
    if (!seen.has(n.key)) out.push(n);
  };
  let overdue = 0;

  for (const r of rows) {
    if (r.status !== 'pending') continue;
    const waiting = r.wait != null && r.wait > now;
    // The task's own times before its dates, or the setting's lead. On a date with no time only the task's own count,
    // back from the hour for such dates: that hour's notice is the setting's.
    const own = offsetsOf(r.remind_in);
    const offsets = own ?? (s.leadMinutes > 0 ? [s.leadMinutes * 60] : []);
    const before = (what: 'soon' | 'scheduled', date: number) => {
      const allDay = isAllDay(date, tz);
      if (allDay && own == null) return;
      const at = allDay ? date + s.allDayHour * HOUR : date;
      const o = reached(offsets, at, now);
      if (o == null) return;
      const word = what === 'soon' ? 'Due' : 'Scheduled';
      add({
        key: `${r.uuid}:${what}:${date}:${o}`,
        kind: what,
        title: `${word} ${allDay ? dayWord(date, now, tz) : until(date - now)}`,
        body: r.description,
        uuid: r.uuid,
      });
    };
    if (r.due != null && !waiting) before('soon', r.due);
    if (r.scheduled != null && !waiting) before('scheduled', r.scheduled);

    if (r.due != null && !waiting) {
      const due = r.due;
      if (isAllDay(due, tz)) {
        // A date without a time: remind at the chosen hour on that day, not at midnight.
        const at = due + s.allDayHour * HOUR;
        if (now >= at && now < due + DAY) {
          add({
            key: `${r.uuid}:allday:${due}`,
            kind: 'allday',
            title: 'Due today',
            body: r.description,
            uuid: r.uuid,
          });
        }
        if (now >= due + DAY) overdue++;
      } else {
        if (now >= due && now < due + FRESH) {
          add({ key: `${r.uuid}:due:${due}`, kind: 'due', title: 'Due now', body: r.description, uuid: r.uuid });
        }
        if (now >= due + FRESH) overdue++;
      }
    }

    if (s.waitOver && r.wait != null && now >= r.wait && now < r.wait + FRESH) {
      add({
        key: `${r.uuid}:wait:${r.wait}`,
        kind: 'wait',
        title: 'Back on your list',
        body: r.description,
        uuid: r.uuid,
      });
    }
  }

  // One digest per day for what is already overdue, instead of a notification per task.
  if (overdue > 0) {
    const day = localDay(now, tz);
    add({
      key: `digest:${day}`,
      kind: 'digest',
      title: `${overdue} overdue ${overdue === 1 ? 'task' : 'tasks'}`,
      body: 'Open the list to catch up.',
      uuid: null,
    });
  }
  return out;
}

/** Drop dedup keys older than `maxAge` so the stored set doesn't grow forever. */
export function prune(seen: Record<string, number>, now: number, maxAge = 3 * DAY): Record<string, number> {
  return Object.fromEntries(Object.entries(seen).filter(([, at]) => now - at < maxAge));
}

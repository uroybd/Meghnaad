// The reminders of a locked tab (spec 0017). The static flavour locks by reloading, which leaves nothing of the tasks in
// the page; the reminder list alone is carried across in that tab's sessionStorage (gone when the tab closes, never
// handed to another tab), and the locked page keeps reminding from it until it is unlocked. It imports nothing of the
// app (the store, the engine), since the locked page has none of it.

import { DEFAULT_SETTINGS, dueNotices, prune, type Notice, type NotifySettings, type Reminder } from './notify';

/** Where the reminders' settings and what was already said are kept: shared with the notifier, so nothing is said twice. */
export const SETTINGS_KEY = 'tw-notify';
export const SEEN_KEY = 'tw-notified';
const LIST = 'meghnaad-locked-reminders';
const EVERY_MS = 15_000;

function readJson<T>(storage: () => Storage, key: string, fallback: T): T {
  try {
    const v = storage().getItem(key);
    return v ? (JSON.parse(v) as T) : fallback;
  } catch {
    return fallback;
  }
}

export function reminderSettings(): NotifySettings {
  return { ...DEFAULT_SETTINGS, ...readJson<Partial<NotifySettings>>(() => localStorage, SETTINGS_KEY, {}) };
}

/** The last list the notifier polled, while unlocked. */
let latest: Reminder[] = [];

export function remember(rows: Reminder[]): void {
  latest = rows;
}

/** Only what a reminder reads of a task, whatever else a row carries. */
function slim(r: Reminder): Reminder {
  return {
    uuid: r.uuid,
    status: r.status,
    description: r.description,
    due: r.due ?? null,
    wait: r.wait ?? null,
    scheduled: r.scheduled ?? null,
    start: r.start ?? null,
    active_seconds: r.active_seconds ?? null,
    remind_in: r.remind_in ?? null,
  };
}

/** Before the lock's reload: keep this tab's list, when reminders are on. */
export function keepForLock(): void {
  if (!reminderSettings().enabled || latest.length === 0) return;
  try {
    sessionStorage.setItem(LIST, JSON.stringify(latest.map(slim)));
  } catch {
    // Not kept: this tab reminds again once it is unlocked.
  }
}

/** The list this tab kept when it locked. */
export function keptList(): Reminder[] {
  const list = readJson<unknown>(() => sessionStorage, LIST, []);
  return Array.isArray(list) ? (list as Reminder[]) : [];
}

/** Unlocked or forgotten: the list goes. */
export function dropList(): void {
  try {
    sessionStorage.removeItem(LIST);
  } catch {
    // Nothing kept.
  }
}

/** The task a reminder clicked while locked was for, opened once unlocked. */
let clicked: string | null = null;

export function takeClicked(): string | null {
  const u = clicked;
  clicked = null;
  return u;
}

/** Check the list now: the notices due, each said once (with the notifier's own record of what was said). */
export function dueNow(list: Reminder[], now = Math.floor(Date.now() / 1000)): Notice[] {
  const settings = reminderSettings();
  if (!settings.enabled) return [];
  let seen = readJson<Record<string, number>>(() => localStorage, SEEN_KEY, {});
  const tz = -new Date(now * 1000).getTimezoneOffset() * 60;
  const notices = dueNotices(list, now, settings, new Set(Object.keys(seen)), tz);
  if (notices.length === 0) return [];
  for (const n of notices) seen[n.key] = now;
  seen = prune(seen, now);
  try {
    localStorage.setItem(SEEN_KEY, JSON.stringify(seen));
  } catch {
    // Said again after the unlock, at worst.
  }
  return notices;
}

/**
 * Keep reminding from the list while locked: a system notification where allowed (a click brings the tab forward,
 * and the task opens once unlocked), else `inPage`. Returns the stop.
 */
export function remindWhileLocked(list: Reminder[], inPage: (n: Notice) => void): () => void {
  const check = () => {
    for (const n of dueNow(list)) {
      if (typeof Notification === 'undefined' || Notification.permission !== 'granted') {
        inPage(n);
        continue;
      }
      try {
        const note = new Notification(n.title, { body: n.body, tag: n.key });
        note.onclick = () => {
          window.focus();
          clicked = n.uuid;
          note.close();
        };
      } catch {
        inPage(n);
      }
    }
  };
  check();
  const timer = setInterval(check, EVERY_MS);
  return () => clearInterval(timer);
}

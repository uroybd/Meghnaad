import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, dueNotices, isAllDay, prune, reached, remindOffsets, type NotifySettings } from './notify';
import type { Row } from './types';

const DAY = 86400;
// 2026-10-07T12:30:00Z, as everywhere else.
const NOW = 1_791_376_200;
const MIDNIGHT = NOW - (NOW % DAY); // 00:00Z today
const on: NotifySettings = { ...DEFAULT_SETTINGS, enabled: true };

function row(over: Partial<Row> = {}): Row {
  return {
    uuid: 'u1',
    status: 'pending',
    description: 'Pay rent',
    project: null,
    priority: null,
    tags: [],
    annotations: [],
    entry: null,
    modified: null,
    start: null,
    end: null,
    due: null,
    wait: null,
    scheduled: null,
    until: null,
    depends: [],
    blocked: false,
    blocking: false,
    recur: null,
    parent: null,
    mask: null,
    imask: null,
    extra: {},
    urgency: 0,
    id: 1,
    virtual_tags: [],
    orphans: [],
    active_seconds: null,
    sessions: [],
    ...over,
  };
}
type R = Row & { remind_in?: string | null };
const kinds = (rows: R[], now = NOW, seen = new Set<string>(), s = on, tz = 0) =>
  dueNotices(rows, now, s, seen, tz).map((n) => n.kind);
const titles = (rows: R[], now = NOW, tz = 0) => dueNotices(rows, now, on, new Set(), tz).map((n) => n.title);

describe('timed due dates', () => {
  it('heads-up inside the lead window, once', () => {
    const due = NOW + 10 * 60;
    expect(kinds([row({ due })])).toEqual(['soon']);
    expect(kinds([row({ due })], NOW - 16 * 60)).toEqual([]); // too early
    const [n] = dueNotices([row({ due })], NOW, on, new Set(), 0);
    expect(n.title).toBe('Due in 10 minutes');
    expect(kinds([row({ due })], NOW, new Set([n.key]))).toEqual([]); // already shown
  });

  it('"due now" at the due time, but not for something long overdue', () => {
    expect(kinds([row({ due: NOW - 60 })])).toEqual(['due']);
    expect(kinds([row({ due: NOW - 2 * 3600 })])).toEqual(['digest']);
  });

  it('a lead of 0 turns the heads-up off', () => {
    expect(kinds([row({ due: NOW + 60 })], NOW, new Set(), { ...on, leadMinutes: 0 })).toEqual([]);
  });

  it('is keyed by the due time, so moving the due date notifies again', () => {
    const a = dueNotices([row({ due: NOW + 300 })], NOW, on, new Set(), 0)[0];
    const b = dueNotices([row({ due: NOW + 600 })], NOW, on, new Set([a.key]), 0)[0];
    expect(b.key).not.toBe(a.key);
  });
});

describe('all-day due dates (no time)', () => {
  it("are detected in the viewer's zone", () => {
    expect(isAllDay(MIDNIGHT, 0)).toBe(true);
    expect(isAllDay(MIDNIGHT, 19800)).toBe(false); // midnight UTC is 05:30 in IST
    expect(isAllDay(MIDNIGHT - 19800, 19800)).toBe(true); // IST midnight
  });

  it('remind at the chosen morning hour, not at midnight', () => {
    const t = (h: number) => MIDNIGHT + h * 3600;
    expect(kinds([row({ due: MIDNIGHT })], t(1))).toEqual([]);
    expect(kinds([row({ due: MIDNIGHT })], t(9))).toEqual(['allday']);
    expect(kinds([row({ due: MIDNIGHT })], t(23))).toEqual(['allday']);
    expect(kinds([row({ due: MIDNIGHT })], t(9), new Set(), { ...on, allDayHour: 14 })).toEqual([]);
  });

  it('become part of the overdue digest the next day', () => {
    expect(kinds([row({ due: MIDNIGHT - DAY })])).toEqual(['digest']);
  });
});

describe('wait', () => {
  it('tells you when a waiting task comes back', () => {
    expect(kinds([row({ wait: NOW - 60 })])).toEqual(['wait']);
    expect(kinds([row({ wait: NOW + 3600 })])).toEqual([]);
    expect(kinds([row({ wait: NOW - 60 })], NOW, new Set(), { ...on, waitOver: false })).toEqual([]);
  });

  it('a task that is still waiting is not announced as due', () => {
    expect(kinds([row({ due: NOW - 60, wait: NOW + 3600 })])).toEqual([]);
  });
});

describe('overdue digest', () => {
  it('is one notification per day, however many tasks', () => {
    const rows = [row({ uuid: 'a', due: NOW - 5 * DAY }), row({ uuid: 'b', due: NOW - 9 * DAY }), row({ uuid: 'c' })];
    const [n] = dueNotices(rows, NOW, on, new Set(), 0);
    expect(n.kind).toBe('digest');
    expect(n.title).toBe('2 overdue tasks');
    expect(n.uuid).toBeNull();
    expect(dueNotices(rows, NOW, on, new Set([n.key]), 0)).toEqual([]);
    // Tomorrow it can fire again.
    expect(dueNotices(rows, NOW + DAY, on, new Set([n.key]), 0).map((x) => x.kind)).toEqual(['digest']);
  });

  it('uses the singular for one task', () => {
    expect(dueNotices([row({ due: NOW - DAY })], NOW, on, new Set(), 0)[0].title).toBe('1 overdue task');
  });
});

describe('only pending tasks', () => {
  it('ignores completed and deleted tasks', () => {
    expect(kinds([row({ status: 'completed', due: NOW - 60 }), row({ status: 'deleted', due: NOW + 60 })])).toEqual([]);
  });
});

describe('prune', () => {
  it('forgets old keys', () => {
    expect(prune({ old: NOW - 4 * DAY, fresh: NOW - 3600 }, NOW)).toEqual({ fresh: NOW - 3600 });
  });
});

describe('remind_in', () => {
  it('reads minutes and Taskwarrior durations through the engine', () => {
    expect(remindOffsets('10,1h')).toEqual([3600, 600]);
    expect(remindOffsets('10min')).toEqual([600]);
    expect(remindOffsets('default')).toBeNull();
    expect(remindOffsets(undefined)).toBeNull();
    expect(remindOffsets('none')).toEqual([]);
  });

  it('the closest moment that has come, none once the date is here', () => {
    expect(reached([3600, 600], 1000 + 3600, 1000)).toBe(3600);
    expect(reached([3600, 600], 1000 + 500, 1000)).toBe(600);
    expect(reached([3600, 600], 1000 + 7200, 1000)).toBeNull();
    expect(reached([600], 1000, 1000)).toBeNull();
  });

  it('each time in the list is said once, the latest that has come', () => {
    const due = NOW + 50 * 60;
    const r = row({ due, remind_in: '1h,10' } as Partial<R>);
    const [first] = dueNotices([r], NOW, on, new Set(), 0);
    expect(first.title).toBe('Due in 50 minutes');
    expect(kinds([r], NOW, new Set([first.key]))).toEqual([]);
    const [second] = dueNotices([r], due - 5 * 60, on, new Set([first.key]), 0);
    expect(second.title).toBe('Due in 5 minutes');
    expect(second.key).not.toBe(first.key);
    // The setting's 15 minutes is not used when the task has its own times.
    expect(kinds([row({ due: NOW + 14 * 60, remind_in: '1h' } as Partial<R>)])).toEqual(['soon']);
    expect(kinds([row({ due: NOW + 14 * 60, remind_in: '5' } as Partial<R>)])).toEqual([]);
  });

  it('none or 0 turns them off, default and nonsense use the setting', () => {
    const due = NOW + 10 * 60;
    expect(kinds([row({ due, remind_in: 'none' } as Partial<R>)])).toEqual([]);
    expect(kinds([row({ due, remind_in: '0' } as Partial<R>)])).toEqual([]);
    expect(kinds([row({ due, remind_in: 'default' } as Partial<R>)])).toEqual(['soon']);
    expect(kinds([row({ due, remind_in: 'soon' } as Partial<R>)])).toEqual(['soon']);
    // "Due now" still comes.
    expect(kinds([row({ due: NOW - 60, remind_in: 'none' } as Partial<R>)])).toEqual(['due']);
  });

  it("scheduled dates get them too, with the setting or the task's own", () => {
    expect(titles([row({ scheduled: NOW + 10 * 60 })])).toEqual(['Scheduled in 10 minutes']);
    expect(kinds([row({ scheduled: NOW + 20 * 60 })])).toEqual([]);
    expect(kinds([row({ scheduled: NOW + 20 * 60, remind_in: '30' } as Partial<R>)])).toEqual(['scheduled']);
    // No notice at the scheduled time itself.
    expect(kinds([row({ scheduled: NOW - 60 })])).toEqual([]);
    // Not while waiting.
    expect(kinds([row({ scheduled: NOW + 10 * 60, wait: NOW + 3600 })])).toEqual([]);
  });

  it("a date with no time counts back from the hour, and only with the task's own times", () => {
    const tomorrow = MIDNIGHT + DAY; // 9:00 tomorrow is the hour
    expect(kinds([row({ due: tomorrow })], tomorrow + 9 * 3600 - 10 * 60)).toEqual([]);
    const r = row({ due: tomorrow, remind_in: '30,1d' } as Partial<R>);
    expect(titles([r], tomorrow + 9 * 3600 - 20 * 60)).toEqual(['Due today']);
    expect(titles([r], tomorrow - 14 * 3600)).toEqual(['Due tomorrow']); // 10:00 today, a day before 9:00 tomorrow
    expect(kinds([r], tomorrow - 16 * 3600)).toEqual([]); // 8:00 today: too early
  });
});

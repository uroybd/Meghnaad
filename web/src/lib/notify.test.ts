import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, dueNotices, isAllDay, prune, type NotifySettings } from './notify';
import type { Row } from './types';

const DAY = 86400;
// 2026-10-07T12:30:00Z, as everywhere else.
const NOW = 1_791_376_200;
const MIDNIGHT = NOW - (NOW % DAY); // 00:00Z today
const on: NotifySettings = { ...DEFAULT_SETTINGS, enabled: true };

function row(over: Partial<Row> = {}): Row {
  return {
    uuid: 'u1', status: 'pending', description: 'Pay rent', project: null, priority: null, tags: [],
    annotations: [], entry: null, modified: null, start: null, end: null, due: null, wait: null,
    scheduled: null, until: null, depends: [], blocked: false, blocking: false, recur: null, parent: null,
    extra: {}, urgency: 0, id: 1, virtual_tags: [], orphans: [], active_seconds: null, sessions: [], ...over,
  };
}
const kinds = (rows: Row[], now = NOW, seen = new Set<string>(), s = on, tz = 0) =>
  dueNotices(rows, now, s, seen, tz).map((n) => n.kind);

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
  it('are detected in the viewer\'s zone', () => {
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

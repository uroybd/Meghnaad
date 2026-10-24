// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import CalendarView from './CalendarView.svelte';
import { store } from './store.svelte';
import type { CalendarDay, CalendarResult } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

const day = (n: number, over: Partial<CalendarDay> = {}): CalendarDay => ({
  day: n,
  today: false,
  weekend: false,
  holiday: false,
  scheduled: false,
  due: null,
  ...over,
});

function show(days: CalendarDay[]) {
  const result: CalendarResult = {
    kind: 'calendar',
    months: [{ year: 2026, month: 12, name: 'December', weeks: [{ number: 49, days }] }],
    weekdays: ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'],
    week_numbers: false,
    legend: false,
    due_colours: false,
    holiday_colours: true,
    holidays: null,
    details: null,
  };
  app = mount(CalendarView, { target: document.body, props: { result } });
  flushSync();
}

describe('holidays on the calendar', () => {
  it('names the holidays of a day in its tooltip', () => {
    show([day(24), day(25, { holiday: true, holidays: ['Christmas Day', 'Boxing Eve'] }), day(26, { holiday: true })]);
    const cells = [...document.querySelectorAll('td.day')];
    expect(cells[1]?.getAttribute('title')).toBe('December 25: Christmas Day, Boxing Eve');
    expect(cells[2]?.getAttribute('title')).toBe('December 26: holiday');
  });

  it('says which holiday it is when the day is tapped, and again to hide it', () => {
    show([day(24), day(25, { holiday: true, holidays: ['Christmas Day'] })]);
    const tap = () => {
      document.querySelector<HTMLButtonElement>('td.holiday button')?.click();
      flushSync();
    };
    expect(document.querySelector('.picked')).toBeNull();
    tap();
    expect(document.querySelector('.picked')?.textContent).toBe('December 25: Christmas Day');
    tap();
    expect(document.querySelector('.picked')).toBeNull();
    // A day with nothing to say stays plain text.
    expect(document.querySelectorAll('td.day button')).toHaveLength(1);
  });

  it('keeps a tap on a due day for its tasks', () => {
    const focus = vi.spyOn(store, 'focusReport').mockImplementation(() => undefined);
    show([day(25, { holiday: true, holidays: ['Christmas Day'], due: 'due' })]);
    document.querySelector<HTMLButtonElement>('td.day button')?.click();
    flushSync();
    expect(focus).toHaveBeenCalledWith('list', ['due:2026-12-25']);
    expect(document.querySelector('.picked')).toBeNull();
  });
});

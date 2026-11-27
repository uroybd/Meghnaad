// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import IterField from './IterField.svelte';
import { anchorOf, buildRule, parseRule } from './iterRule';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

/** The picker over `value`, opened; what it holds is read back from `props.value`. */
function picker(value: string, anchor = '2026-12-25') {
  const props = $state({ value, anchor });
  app = mount(IterField, { target: document.body, props });
  flushSync();
  (document.querySelector('.trigger') as HTMLButtonElement).click();
  flushSync();
  return props;
}
const button = (text: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('.panel button')].find((b) => b.textContent?.trim() === text)!;
const tab = (name: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('[role=tab]')].find((b) => b.textContent === name)!;

describe('the iter rule, read and written by the engine', () => {
  it('round-trips a calendar rule and leaves the rest to the Taskwarrior mode', () => {
    const r = parseRule('FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231');
    expect(r).toMatchObject({ freq: 'weekly', interval: 2, days: ['MO', 'FR'], until: '2027-12-31' });
    expect(buildRule(r!)).toBe('FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231');
    expect(parseRule('monthly')).toBeNull();
  });

  it('counts a new rule from the first date that is a day, else today', () => {
    expect(anchorOf(['eom', '2026-12-25T10:00', ''])).toBe('2026-12-25');
    expect(anchorOf(['', ''], new Date(2026, 10, 27))).toBe('2026-11-27');
  });
});

describe('the Iter picker', () => {
  it('opens a Taskwarrior period in its Taskwarrior mode, and a pill sets one', () => {
    const p = picker('monthly');
    expect(tab('Taskwarrior').getAttribute('aria-selected')).toBe('true');
    button('Every week').click();
    flushSync();
    expect(p.value).toBe('weekly');
  });

  it('builds a rule as a calendar app does, from the task’s day', () => {
    const p = picker('');
    tab('Calendar (RRULE)').click();
    flushSync();
    // Every week on the anchor's weekday (2026-12-25 is a Friday).
    expect(p.value).toBe('FREQ=WEEKLY;BYDAY=FR');
    (document.querySelector('[aria-label=Monday]') as HTMLButtonElement).click();
    flushSync();
    expect(p.value).toBe('FREQ=WEEKLY;BYDAY=MO,FR');
    const unit = document.querySelector('[aria-label=Unit]') as HTMLSelectElement;
    unit.value = 'monthly';
    unit.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(p.value).toBe('FREQ=MONTHLY;BYMONTHDAY=25');
    const which = document.querySelector('[aria-label="Which day of the month"]') as HTMLSelectElement;
    expect([...which.options].map((o) => o.text)).toEqual(['On day 25', 'On the fourth Friday', 'On the last Friday']);
    which.value = 'n-1FR';
    which.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(p.value).toBe('FREQ=MONTHLY;BYDAY=-1FR');
    const after = document.querySelectorAll<HTMLInputElement>('input[name=iterfield-ends]')[2];
    after.click();
    flushSync();
    expect(p.value).toBe('FREQ=MONTHLY;BYDAY=-1FR;COUNT=10');
    expect(document.querySelector('.said')?.textContent).toBe('Every month on the last Friday, 10 times');
  });

  it('opens a calendar rule in its Calendar mode, and going back restores what was typed', () => {
    const p = picker('FREQ=WEEKLY;BYDAY=TU');
    expect(tab('Calendar (RRULE)').getAttribute('aria-selected')).toBe('true');
    expect((document.querySelector('[aria-label=Tuesday]') as HTMLElement).getAttribute('aria-pressed')).toBe('true');
    unmount(app!);
    app = null;
    document.body.innerHTML = '';
    const q = picker('3d');
    tab('Calendar (RRULE)').click();
    flushSync();
    expect(q.value).toBe('FREQ=WEEKLY;BYDAY=FR');
    tab('Taskwarrior').click();
    flushSync();
    expect(q.value).toBe('3d');
    expect(p.value).toBe('FREQ=WEEKLY;BYDAY=TU');
  });
});

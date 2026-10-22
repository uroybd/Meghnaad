// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { reactive } from '../reactive.svelte';
import DateField from './DateField.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

function draw(value = '') {
  const props = reactive({ value, label: 'Due' });
  app = mount(DateField, { target: document.body, props });
  flushSync();
  return props;
}
const trigger = () => document.querySelector<HTMLButtonElement>('button.trigger')!;
const typed = () => document.querySelector<HTMLInputElement>('#datefield-input')!;
const type = (v: string) => {
  typed().value = v;
  typed().dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
};

describe('a date field', () => {
  it('shows the date, or what to do, and opens a panel with a field to type in', () => {
    const p = draw('');
    expect(trigger().textContent).toContain('Set date');
    expect(document.querySelector('[role=dialog]')).toBeNull();
    trigger().click();
    flushSync();
    expect(document.querySelector('[role=dialog]')).not.toBeNull();
    type('eow-2d');
    expect(p.value).toBe('eow-2d');
    expect(trigger().textContent).toContain('eow-2d');
  });

  it('takes a keyword from the buttons, and a day and time from the calendar', () => {
    const p = draw('');
    trigger().click();
    flushSync();
    [...document.querySelectorAll<HTMLButtonElement>('.word')].find((b) => b.textContent === 'sow')!.click();
    flushSync();
    expect(p.value).toBe('sow');
    expect(document.querySelector('.hint')?.textContent).toContain('date expression');

    const day = document.querySelector<HTMLInputElement>('input[type=date]')!;
    day.value = '2026-10-01';
    day.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(p.value).toBe('2026-10-01');
    const time = document.querySelector<HTMLInputElement>('input[type=time]')!;
    time.value = '09:30';
    time.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(p.value).toBe('2026-10-01T09:30');
  });

  it('says in a tooltip what each keyword means, and when that is now', () => {
    draw('');
    trigger().click();
    flushSync();
    const tip = (w: string) =>
      [...document.querySelectorAll<HTMLButtonElement>('.word')].find((b) => b.textContent === w)!.title;
    expect(tip('sow')).toMatch(/^start of week \(.+\)$/);
    expect(tip('eom')).toMatch(/^end of month \(.+\)$/);
    expect(tip('now')).toMatch(/^this moment \(.+\)$/);
    for (const b of document.querySelectorAll<HTMLButtonElement>('.word')) expect(b.title).not.toBe('');
  });

  it('completes the engine’s date words as they are typed, and clears', () => {
    const p = draw('');
    trigger().click();
    flushSync();
    type('eo');
    const words = [...document.querySelectorAll('[role=option] .v')].map((e) => e.textContent);
    expect(words).toEqual(expect.arrayContaining(['eod', 'eow', 'eom', 'eoy']));
    document.querySelector<HTMLElement>('[role=option]')!.click();
    flushSync();
    expect(p.value).toMatch(/^eo[dwmy]$/);
    [...document.querySelectorAll<HTMLButtonElement>('.foot button')]
      .find((b) => b.textContent?.includes('Clear'))!
      .click();
    flushSync();
    expect(p.value).toBe('');
  });

  it('closes with Done and with Escape', () => {
    draw('sow');
    trigger().click();
    flushSync();
    [...document.querySelectorAll<HTMLButtonElement>('.foot button')].find((b) => b.textContent === 'Done')!.click();
    flushSync();
    expect(document.querySelector('[role=dialog]')).toBeNull();
    trigger().click();
    flushSync();
    typed().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    flushSync();
    expect(document.querySelector('[role=dialog]')).toBeNull();
  });
});

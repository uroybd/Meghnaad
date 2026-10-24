// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import AddPreviewCard from './AddPreviewCard.svelte';
import TaskEditor from './TaskEditor.svelte';
import { seedOf, type AddPreview } from './addPreview';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  // jsdom has no modal dialogs.
  HTMLDialogElement.prototype.showModal = function showModal() {
    this.setAttribute('open', '');
  };
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date(2026, 9, 18, 12, 0, 0));
  store.config = {
    config: { settings: { 'holiday.xmas.name': 'Christmas Day', 'holiday.xmas.date': '20261225' }, udas: {} },
  } as never;
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.config = null;
  vi.useRealTimers();
});

const preview = (dates: Record<string, number>): AddPreview => ({
  description: 'Party',
  project: null,
  priority: null,
  tags: [],
  dates,
  recur: null,
  depends: [],
  udas: {},
  errors: [],
});

describe('the task form', () => {
  it('outlines a due and a scheduled date that are on a holiday, and no other', () => {
    const seed = { ...seedOf(preview({}), 'Party'), due: '2026-12-25', scheduled: '2026-12-24', wait: '2026-12-25' };
    app = mount(TaskEditor, { target: document.body, props: { seed, onclose: () => {} } });
    flushSync();
    const field = (id: string) => document.getElementById(id) as HTMLButtonElement;
    expect(field('te-due').classList.contains('warn')).toBe(true);
    expect(field('te-due').title).toContain('Christmas Day');
    expect(field('te-sched').classList.contains('warn')).toBe(false);
    expect(field('te-wait').classList.contains('warn')).toBe(false);
  });

  it('resolves a word such as eod to its day first', () => {
    store.config = {
      config: {
        settings: { 'holiday.today.name': 'Founders day', 'holiday.today.date': '20261018' },
        udas: {},
      },
    } as never;
    const seed = { ...seedOf(preview({}), 'Party'), due: 'eod' };
    app = mount(TaskEditor, { target: document.body, props: { seed, onclose: () => {} } });
    flushSync();
    expect(document.getElementById('te-due')?.classList.contains('warn')).toBe(true);
    expect(document.getElementById('te-due')?.title).toContain('due (eod) is 2026-10-18');
  });
});

describe('the quick add card', () => {
  it('says so beside a due date on a holiday', () => {
    const at = Math.floor(new Date(2026, 11, 25, 9, 0, 0).getTime() / 1000);
    const other = Math.floor(new Date(2026, 11, 26, 9, 0, 0).getTime() / 1000);
    app = mount(AddPreviewCard, { target: document.body, props: { preview: preview({ due: at, scheduled: other }) } });
    flushSync();
    const facts = [...document.querySelectorAll('.fact')];
    const due = facts.find((f) => f.textContent?.includes('due'));
    const sched = facts.find((f) => f.textContent?.includes('scheduled'));
    expect(due?.classList.contains('warn')).toBe(true);
    expect(due?.getAttribute('title')).toContain('Christmas Day');
    expect(sched?.classList.contains('warn')).toBe(false);
  });
});

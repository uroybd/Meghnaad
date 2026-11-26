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
  iter: null,
  iter_type: null,
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

describe('the task form, for an iterative task', () => {
  const shown = () => document.querySelector('footer .preview')?.textContent ?? '';

  it('takes Iter and Iter type from a quick add line and sends them', () => {
    const seed = { ...seedOf(preview({}), 'Rent'), due: '2026-11-20', iter: 'monthly', iterType: 'fixed' };
    app = mount(TaskEditor, { target: document.body, props: { seed, onclose: () => {} } });
    flushSync();
    expect((document.getElementById('te-iter') as HTMLInputElement).value).toBe('monthly');
    expect((document.getElementById('te-iter-type') as HTMLSelectElement).value).toBe('fixed');
    expect(shown()).toContain('iter:monthly iter_type:fixed');
  });

  it('says a date is needed, and offers no Iter on a repeating task', () => {
    const seed = { ...seedOf(preview({}), 'Rent'), iter: 'monthly', iterType: 'fixed' };
    app = mount(TaskEditor, { target: document.body, props: { seed, onclose: () => {} } });
    flushSync();
    expect(document.querySelector('[role=alert]')?.textContent).toContain('needs a due, scheduled or wait date');
    unmount(app);
    app = mount(TaskEditor, {
      target: document.body,
      props: { seed: { ...seedOf(preview({}), 'Bins'), due: '2026-11-20', recur: 'weekly' }, onclose: () => {} },
    });
    flushSync();
    expect(document.getElementById('te-iter')).toBeNull();
  });
});

describe('the task form, its repeat mode', () => {
  const shown = () => document.querySelector('footer .preview')?.textContent ?? '';
  const pick = (id: string, value: string) => {
    const el = document.getElementById(id) as HTMLSelectElement;
    el.value = value;
    el.dispatchEvent(new Event('change'));
    flushSync();
  };
  const task = (fields: Record<string, unknown>) =>
    ({
      uuid: '11111111-2222-3333-4444-555555555555',
      id: 3,
      status: 'iterative',
      description: 'Water plants',
      tags: [],
      annotations: [],
      depends: [],
      due: Math.floor(new Date(2026, 10, 20).getTime() / 1000),
      urgency: 0,
      orphans: [],
      active_seconds: null,
      sessions: [],
      extra: {},
      ...fields,
    }) as never;

  it('shows the fields of the mode it is in, and only those', () => {
    app = mount(TaskEditor, { target: document.body, props: { seed: seedOf(preview({}), 'Bins'), onclose: () => {} } });
    flushSync();
    expect((document.getElementById('te-recur-mode') as HTMLSelectElement).value).toBe('none');
    expect(document.getElementById('te-iter')).toBeNull();
    expect(document.getElementById('te-recur')).toBeNull();
    pick('te-recur-mode', 'iterative');
    expect(document.getElementById('te-iter')).not.toBeNull();
    expect(document.getElementById('te-recur')).toBeNull();
    pick('te-recur-mode', 'recurring');
    expect(document.getElementById('te-iter')).toBeNull();
    expect(document.getElementById('te-recur')).not.toBeNull();
  });

  it('does not make an iterative task recurring: its series is ended first', () => {
    app = mount(TaskEditor, {
      target: document.body,
      props: { row: task({ iter: 'monthly', iter_type: 'fixed' }), onclose: () => {} },
    });
    flushSync();
    const offered = [...(document.getElementById('te-recur-mode') as HTMLSelectElement).options].map((o) => o.value);
    expect(offered).toEqual(['none', 'iterative']);
  });

  it("clears the other mode's fields in the modify it runs, and says so", () => {
    app = mount(TaskEditor, {
      target: document.body,
      props: { row: task({ iter: 'monthly', iter_type: 'fixed' }), onclose: () => {} },
    });
    flushSync();
    pick('te-recur-mode', 'none');
    expect(shown()).toContain('modify iter: iter_type:');
    expect(document.body.textContent).toContain('Saving ends its iterative series');
    expect(document.body.textContent).toContain('then it can be made recurring');
  });

  it('keeps a recurring task recurring', () => {
    app = mount(TaskEditor, {
      target: document.body,
      props: { row: task({ status: 'recurring', recur: 'weekly' }), onclose: () => {} },
    });
    flushSync();
    const select = document.getElementById('te-recur-mode') as HTMLSelectElement;
    expect(select.value).toBe('recurring');
    expect(select.disabled).toBe(true);
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

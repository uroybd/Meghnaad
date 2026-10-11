// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';
import NavMenu from './NavMenu.svelte';
import { notifier } from './notifier.svelte';
import { review } from './review.svelte';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

let app: ReturnType<typeof mount> | null = null;
const navigate = vi.fn();

const item = (label: string) =>
  [...document.querySelectorAll('nav button')].find((b) => b.textContent?.trim().startsWith(label)) as
    HTMLButtonElement | undefined;

beforeEach(() => {
  store.config = {
    reports: ['next', 'list', 'overdue'].map((name) => ({ name, description: `${name} tasks`, columns: [] })),
    config: { udas: {}, settings: {}, reports: {} },
  } as unknown as ConfigResponse;
  store.view = 'tasks';
  store.report = 'next';
  navigate.mockReset();
  app = mount(NavMenu, { target: document.body, props: { onnavigate: navigate } });
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

describe('NavMenu', () => {
  it('lists every page of the top bar, with Tasks opening into the reports', () => {
    for (const label of [
      'Tasks',
      'Projects',
      'Tags',
      'Summary',
      'Calendar',
      'Burndown',
      'Console',
      'Review',
      'Reminders',
      'Settings',
    ])
      expect(item(label), label).toBeDefined();
    const reports = [...document.querySelectorAll('.reports li button')].map((b) => b.textContent?.trim());
    expect(reports).toEqual(['next', 'list', 'overdue']);
    // Closed to begin with, so the pages are not pushed down the drawer; the current report is named on the item.
    expect(item('Tasks')!.getAttribute('aria-expanded')).toBe('false');
    expect(item('Tasks')!.textContent).toContain('next');
  });

  it('opens and closes the reports from the Tasks item', async () => {
    const tasks = item('Tasks')!;
    expect(document.querySelector('.reports')!.classList.contains('collapsed')).toBe(true);
    tasks.click();
    await tick();
    expect(tasks.getAttribute('aria-expanded')).toBe('true');
    expect(document.querySelector('.reports')!.classList.contains('collapsed')).toBe(false);
    tasks.click();
    await tick();
    expect(document.querySelector('.reports')!.classList.contains('collapsed')).toBe(true);
  });

  it('picks a report: shows it on the Tasks page and closes the drawer', () => {
    store.view = 'calendar';
    item('overdue')!.click();
    expect(store.report).toBe('overdue');
    expect(store.view).toBe('tasks');
    expect(navigate).toHaveBeenCalledTimes(1);
  });

  it('goes to a page and closes the drawer', async () => {
    for (const [label, view] of [
      ['Projects', 'projects'],
      ['Tags', 'tags'],
      ['Summary', 'summary'],
      ['Calendar', 'calendar'],
      ['Burndown', 'burndown'],
      ['Console', 'console'],
    ] as const) {
      item(label)!.click();
      expect(store.view).toBe(view);
    }
    expect(navigate).toHaveBeenCalledTimes(6);
    await tick();
    expect(item('Console')!.getAttribute('aria-current')).toBe('page');
    expect(item('Tags')!.getAttribute('aria-current')).toBeNull();
  });

  it('has Settings, Reminders and Review, each closing the drawer', async () => {
    const open = vi.spyOn(store, 'openSettings').mockImplementation(() => {});
    item('Settings')!.click();
    expect(open).toHaveBeenCalled();

    store.notifyOpen = false;
    item('Reminders')!.click();
    expect(store.notifyOpen).toBe(true);

    const start = vi.spyOn(review, 'start').mockResolvedValue(undefined as never);
    item('Review')!.click();
    expect(start).toHaveBeenCalledWith({ surface: 'card' });
    expect(navigate).toHaveBeenCalledTimes(3);
  });

  it('says whether reminders are on', async () => {
    notifier.settings.enabled = true;
    await tick();
    expect(item('Reminders')!.textContent).toContain('on');
    notifier.settings.enabled = false;
    await tick();
    expect(item('Reminders')!.textContent).toContain('off');
  });
});

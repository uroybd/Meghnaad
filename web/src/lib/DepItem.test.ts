// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import DepItem from './DepItem.svelte';
import { store } from './store.svelte';
import type { Row } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.detail = null;
  store.rev++; // what was fetched for a task is forgotten when the tasks change
  store.tasks = [];
  vi.unstubAllGlobals();
});

const UUID = '9f8e7d6c-5b4a-4938-8271-605f4e3d2c1b';
const row = {
  uuid: UUID,
  id: 7,
  status: 'pending',
  description: 'Book the venue',
  project: 'Work.Event',
  priority: null,
  tags: [],
  due: null,
  virtual_tags: [],
} as unknown as Row;

const answer = (r: unknown) =>
  new Response(JSON.stringify({ wrote: false, result: r, command: null, feedback: [] }), { status: 200 });

const settle = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

const draw = (props: { row?: Row | null; onopen?: (u: string) => void } = {}) => {
  app = mount(DepItem, { target: document.body, props: { uuid: UUID, ...props } });
  flushSync();
  return document.querySelector('button.line') as HTMLButtonElement;
};

describe('another task in the detail pane', () => {
  it('shows what the page knows of it, and fetches nothing until it is tapped', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => answer({ kind: 'info', tasks: [row] }));
    vi.stubGlobal('fetch', fetchMock);
    store.tasks = [{ id: 7, uuid: UUID, description: 'Book the venue' }];
    const line = draw();
    expect(line.textContent).toContain('Book the venue');
    expect(line.textContent).toContain('9f8e7d6c');
    expect(fetchMock).not.toHaveBeenCalled();

    line.click();
    await settle();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(JSON.parse(fetchMock.mock.calls[0][1].body).args).toEqual([UUID, 'info']);
    const card = document.querySelector('[data-testid="depitem-card"]')!;
    expect(card.textContent).toContain('Work.Event');

    line.click();
    await settle();
    expect(document.querySelector('[data-testid="depitem-card"]')).toBeNull();
  });

  it('does not fetch a task the caller already holds', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    draw({ row }).click();
    await settle();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(document.querySelector('[data-testid="depitem-card"]')!.textContent).toContain('Book the venue');
  });

  it('opens the task from the card, through the caller when it gives a way', async () => {
    const onopen = vi.fn();
    draw({ row, onopen }).click();
    await settle();
    (document.querySelector('.open') as HTMLButtonElement).click();
    expect(onopen).toHaveBeenCalledWith(UUID);
    expect(store.detail).toBeNull();
  });

  it('opens it in the drawer when the caller gives no way', async () => {
    draw({ row }).click();
    await settle();
    (document.querySelector('.open') as HTMLButtonElement).click();
    flushSync();
    expect(store.detail?.uuid).toBe(UUID);
  });

  it('says so when the task is gone', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => answer({ kind: 'text', lines: ['No matches.'] })),
    );
    draw().click();
    await settle();
    expect(document.querySelector('[data-testid="depitem-card"]')!.textContent).toContain('not here any more');
  });
});

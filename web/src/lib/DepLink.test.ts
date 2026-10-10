// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import DepLink from './DepLink.svelte';
import { store } from './store.svelte';
import type { Row } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.detail = null;
  store.rev++; // what was fetched for a task is forgotten when the tasks change
  vi.unstubAllGlobals();
});

const UUID = '1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d';
const row = {
  uuid: UUID,
  id: 4,
  status: 'pending',
  description: 'Book the venue',
  project: 'Work.Event',
  priority: 'H',
  tags: ['next'],
  due: 1_800_000_000,
  virtual_tags: [],
} as unknown as Row;

const answer = (r: unknown) =>
  new Response(JSON.stringify({ wrote: false, result: r, command: null, feedback: [] }), { status: 200 });

const settle = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

const draw = () => {
  app = mount(DepLink, { target: document.body, props: { uuid: UUID } });
  flushSync(); // the handlers are attached in an effect
  return document.querySelector('button.id') as HTMLButtonElement;
};

describe('a link to another task', () => {
  it('shows the short id, and a card when pointed at, asking once for the task', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => answer({ kind: 'info', tasks: [row] }));
    vi.stubGlobal('fetch', fetchMock);
    const id = draw();
    expect(id.textContent).toBe('1a2b3c4d');
    expect(document.querySelector('[data-testid="deplink-card"]')).toBeNull();

    id.parentElement!.dispatchEvent(new MouseEvent('pointerenter'));
    await settle();
    const card = document.querySelector('[data-testid="deplink-card"]')!;
    expect(card.textContent).toContain('Book the venue');
    expect(card.textContent).toContain('Work.Event');
    expect(card.textContent).toContain('priority H');
    expect(card.textContent).toContain('+next');
    expect(JSON.parse(fetchMock.mock.calls[0][1].body).args).toEqual([UUID, 'info']);

    id.parentElement!.dispatchEvent(new MouseEvent('pointerleave'));
    await settle();
    expect(document.querySelector('[data-testid="deplink-card"]')).toBeNull();
    id.parentElement!.dispatchEvent(new MouseEvent('pointerenter'));
    await settle();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('opens the task in the drawer from the card’s button', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => answer({ kind: 'info', tasks: [row] })),
    );
    const id = draw();
    id.parentElement!.dispatchEvent(new MouseEvent('pointerenter'));
    await settle();
    (document.querySelector('.open') as HTMLButtonElement).click();
    flushSync();
    expect(store.detail?.uuid).toBe(UUID);
    expect(document.querySelector('[data-testid="deplink-card"]')).toBeNull();
  });

  it('on a touch screen a tap shows the card and a second tap puts it away', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => answer({ kind: 'info', tasks: [row] })),
    );
    vi.stubGlobal('matchMedia', () => ({ matches: false }));
    const id = draw();
    id.click();
    await settle();
    expect(document.querySelector('[data-testid="deplink-card"]')).not.toBeNull();
    expect(store.detail).toBeNull();
    id.click();
    await settle();
    expect(document.querySelector('[data-testid="deplink-card"]')).toBeNull();
  });

  it('where a pointer can hover, a click goes straight to the task', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => answer({ kind: 'info', tasks: [row] })),
    );
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    draw().click();
    flushSync();
    expect(store.detail?.uuid).toBe(UUID);
  });

  it('says so when the task is gone', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => answer({ kind: 'text', lines: ['No matches.'] })),
    );
    const id = draw();
    id.parentElement!.dispatchEvent(new MouseEvent('pointerenter'));
    await settle();
    expect(document.querySelector('[data-testid="deplink-card"]')!.textContent).toContain('not here any more');
  });
});

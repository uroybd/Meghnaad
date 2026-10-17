// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { parseSettings } from './appsettings';
import BoardsPage from './BoardsPage.svelte';
import { store } from './store.svelte';

const board = (id: string, name: string) => ({ id, name, sections: [] });

let app: ReturnType<typeof mount> | null = null;
/** The `settings.json` the stand-in server holds. */
let saved = '';
const puts: string[] = [];

function server(file: object) {
  saved = JSON.stringify(file);
  puts.length = 0;
  vi.stubGlobal(
    'fetch',
    vi.fn().mockImplementation(async (url: string, init?: { method?: string; body?: string }) => {
      if (url === '/api/settings' && init?.method === 'PUT') {
        saved = init.body ?? '';
        puts.push(saved);
        return new Response(JSON.stringify({ ok: true }));
      }
      if (url === '/api/settings') return new Response(saved);
      return new Response(JSON.stringify({ config: {} }));
    }),
  );
  store.appSettings = parseSettings(saved);
}

const settle = async () => {
  for (let i = 0; i < 8; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};
const click = async (label: RegExp) => {
  [...document.querySelectorAll('button')]
    .find((b) => label.test(b.textContent ?? '') || label.test(b.getAttribute('aria-label') ?? ''))!
    .click();
  await settle();
};
const slugInput = () => document.querySelector<HTMLInputElement>('.slug-input input')!;
const type = async (input: HTMLInputElement, value: string) => {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
};

beforeEach(() => {
  store.boardId = null;
  // jsdom has no modal dialogs.
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
  store.appSettings = null;
  store.boardId = null;
});

describe('the boards page', () => {
  it('lists boards as links to their addresses', async () => {
    server({ boardsVersion: 1, boards: [board('week', 'My week'), board('q4', 'Q4')] });
    app = mount(BoardsPage, { target: document.body });
    await settle();
    expect([...document.querySelectorAll<HTMLAnchorElement>('a.card')].map((a) => a.getAttribute('href'))).toEqual([
      '/board/week',
      '/board/q4',
    ]);
    document.querySelector<HTMLAnchorElement>('a.card')!.click();
    await settle();
    expect(store.boardId).toBe('week');
    expect(document.querySelector('.address')?.textContent).toBe('/board/week');
  });

  it('changes a board’s address, and the home page goes with it', async () => {
    server({ boardsVersion: 1, boards: [board('week', 'My week'), board('q4', 'Q4')], homepage: { board: 'week' } });
    store.boardId = 'week';
    app = mount(BoardsPage, { target: document.body });
    await settle();
    await click(/Edit/);
    expect(slugInput().value).toBe('week');
    await type(slugInput(), 'Weekly Review');
    // Said as it is typed, and Save waits.
    expect(slugInput().value).toBe('weekly review');
    expect(document.body.textContent).toContain('lower case letters');
    expect(document.querySelector<HTMLButtonElement>('button.primary')!.disabled).toBe(true);

    await type(slugInput(), 'weekly');
    expect(document.querySelector<HTMLButtonElement>('button.primary')!.disabled).toBe(false);
    await click(/^Save$/);

    expect(puts).toHaveLength(1);
    const out = JSON.parse(puts[0]);
    expect(out.boards.map((b: { id: string }) => b.id)).toEqual(['weekly', 'q4']);
    expect(out.homepage).toEqual({ board: 'weekly' });
    // Still on the board, at its new address, and not thrown back to the list.
    expect(store.boardId).toBe('weekly');
    expect(document.querySelector('.address')?.textContent).toBe('/board/weekly');
  });

  it('leaves the home page alone when another board’s address changes, and refuses an address in use', async () => {
    server({ boardsVersion: 1, boards: [board('week', 'My week'), board('q4', 'Q4')], homepage: { board: 'week' } });
    store.boardId = 'q4';
    app = mount(BoardsPage, { target: document.body });
    await settle();
    await click(/Edit/);
    await type(slugInput(), 'week');
    expect(document.body.textContent).toContain('already a board at /board/week');
    expect(document.querySelector<HTMLButtonElement>('button.primary')!.disabled).toBe(true);
    await type(slugInput(), 'quarter');
    await click(/^Save$/);
    const out = JSON.parse(puts[0]);
    expect(out.homepage).toEqual({ board: 'week' });
    expect(store.boardId).toBe('quarter');
  });

  it('gives a new board an address made from its name', async () => {
    server({ boardsVersion: 1, boards: [board('week', 'My week')] });
    app = mount(BoardsPage, { target: document.body });
    await settle();
    await type(document.querySelector<HTMLInputElement>('.new input')!, 'My Week!');
    await click(/New board/);
    const out = JSON.parse(puts[0]);
    // 'my-week' is free; the one with the same name as the first would be 'my-week-2'.
    expect(out.boards.map((b: { id: string }) => b.id)).toEqual(['week', 'my-week']);
    expect(store.boardId).toBe('my-week');
  });

  it('opens a section’s settings in a panel at the side, not among the board', async () => {
    server({
      boardsVersion: 1,
      boards: [
        {
          id: 'week',
          name: 'My week',
          sections: [{ id: 'open', title: 'Open', x: 0, y: 0, w: 3, h: 2, type: 'number', measure: { fn: 'count' } }],
        },
      ],
    });
    store.boardId = 'week';
    app = mount(BoardsPage, { target: document.body });
    await settle();
    await click(/Edit/);
    expect(document.querySelector('dialog')).toBeNull();
    await click(/Settings of Open/);
    const panel = document.querySelector('dialog')!;
    expect(panel.getAttribute('aria-label')).toBe('Open');
    expect(panel.querySelector('input')).not.toBeNull();
    await click(/^Close$/);
    expect(document.querySelector('dialog')).toBeNull();
  });

  it('has a switch, off, to leave the board’s filter out of a section, and saves it', async () => {
    server({
      boardsVersion: 1,
      boards: [
        {
          id: 'week',
          name: 'My week',
          filter: 'project:Home',
          sections: [{ id: 'open', title: 'Open', x: 0, y: 0, w: 3, h: 2, type: 'number', measure: { fn: 'count' } }],
        },
      ],
    });
    store.boardId = 'week';
    app = mount(BoardsPage, { target: document.body });
    await settle();
    await click(/Edit/);
    await click(/Settings of Open/);
    const ignore = [...document.querySelectorAll<HTMLInputElement>('dialog input[type=checkbox]')].find((i) =>
      i.closest('label')?.textContent?.includes('Ignore the board'),
    )!;
    expect(ignore.checked).toBe(false);
    ignore.click();
    await settle();
    await click(/^Close$/);
    await click(/^Save/);
    const saved = JSON.parse(puts[puts.length - 1]);
    expect(saved.boards[0].sections[0].ignoreBoardFilter).toBe(true);
  });

  it('opens the panel for a section just added', async () => {
    server({ boardsVersion: 1, boards: [board('week', 'My week')] });
    store.boardId = 'week';
    app = mount(BoardsPage, { target: document.body });
    await settle();
    await click(/Edit/);
    await click(/Number/);
    expect(document.querySelector('dialog')?.getAttribute('aria-label')).toBe('Number');
  });
});

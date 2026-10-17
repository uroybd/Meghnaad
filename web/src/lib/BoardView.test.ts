// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { Board, Section } from './boards';
import BoardView from './BoardView.svelte';

const sec = (id: string, x: number, y: number, w: number, h: number): Section => ({
  id,
  title: id.toUpperCase(),
  x,
  y,
  w,
  h,
  type: 'number',
  measure: { fn: 'count' },
});
const BOARD: Board = {
  id: 'b',
  name: 'B',
  sections: [sec('b', 6, 0, 6, 2), sec('a', 0, 0, 6, 2), sec('c', 0, 2, 12, 2)],
};

let app: ReturnType<typeof mount> | null = null;
const sets = <T extends object>(props: T) => props;

function stubs(narrow: boolean) {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockImplementation(async (url: string) => {
      if (url !== '/api/cli') return new Response(JSON.stringify({ config: {} }));
      const result = { kind: 'file', name: 't.json', mime: 'application/json', text: '[{"id":1},{"id":2}]', count: 2 };
      return new Response(JSON.stringify({ wrote: false, result, command: null }));
    }),
  );
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockImplementation(() => ({
      matches: narrow,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    })),
  );
}

beforeEach(() => {
  vi.useFakeTimers();
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
  vi.useRealTimers();
  vi.unstubAllGlobals();
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const titles = () => [...document.querySelectorAll('.cell .title')].map((e) => e.textContent);
const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label^="${label}"]`)!;

function draw(props: object, narrow = false) {
  stubs(narrow);
  app = mount(BoardView, { target: document.body, props: sets({ board: BOARD, ...props }) });
  flushSync();
}

describe('a board', () => {
  it('draws its sections in reading order, whatever order the file lists them in', () => {
    draw({});
    expect(titles()).toEqual(['A', 'B', 'C']);
  });

  it('puts each section where the file says, as a place on the grid', () => {
    draw({});
    const cell = document.querySelectorAll<HTMLElement>('.cell')[1];
    expect(cell.style.getPropertyValue('--x')).toBe('6');
    expect(cell.style.getPropertyValue('--w')).toBe('6');
    expect(cell.style.getPropertyValue('--h')).toBe('2');
  });

  it('has no handles until the layout is being edited', () => {
    draw({});
    expect(document.querySelector('.tools')).toBeNull();
    expect(document.querySelector('.resize')).toBeNull();
  });

  it('fills a number with the count of the tasks its filter picks', async () => {
    draw({});
    await vi.advanceTimersByTimeAsync(400);
    flushSync();
    expect(document.querySelector('.big')?.textContent).toBe('2');
  });
});

describe('editing the layout', () => {
  it('moves a section a column with the arrow keys, and says where it is now', () => {
    const onchange = vi.fn();
    draw({ editing: true, onchange });
    button('Move A').dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }),
    );
    flushSync();
    const next: Section[] = onchange.mock.calls[0][0];
    // A moved right onto B, which is pushed down; nothing overlaps.
    expect(next.find((s) => s.id === 'a')).toMatchObject({ x: 1, y: 0 });
    expect(next.find((s) => s.id === 'b')!.y).toBeGreaterThan(0);
    expect(document.querySelector('[aria-live]')?.textContent).toContain('A: column 2, row 1');
  });

  it('resizes a section with the arrow keys on its corner', () => {
    const onchange = vi.fn();
    draw({ editing: true, onchange });
    button('Resize A').dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    );
    expect(onchange.mock.calls[0][0].find((s: Section) => s.id === 'a')).toMatchObject({ w: 6, h: 3 });
  });

  it('ignores other keys', () => {
    const onchange = vi.fn();
    draw({ editing: true, onchange });
    button('Move A').dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
    expect(onchange).not.toHaveBeenCalled();
  });

  it('asks to select a section and to remove one', () => {
    const onselect = vi.fn();
    const onremove = vi.fn();
    draw({ editing: true, onselect, onremove });
    button('Settings of B').click();
    button('Remove C').click();
    expect(onselect).toHaveBeenCalledWith('b');
    expect(onremove).toHaveBeenCalledWith('c');
  });

  it('asks to select a section and to remove one', () => {
    const onselect = vi.fn();
    const onedit = vi.fn();
    const onremove = vi.fn();
    draw({ editing: true, onselect, onedit, onremove });
    button('Settings of B').click();
    button('Remove C').click();
    expect(onselect).toHaveBeenCalledWith('b');
    expect(onedit).toHaveBeenCalledWith('b');
    expect(onremove).toHaveBeenCalledWith('c');
  });
});

describe('a board on a phone', () => {
  it('puts the sections on four columns, as wide as their share of the row', () => {
    draw({}, true);
    const cells = [...document.querySelectorAll<HTMLElement>('.cell')];
    expect(cells.map((c) => c.style.getPropertyValue('--w'))).toEqual(['2', '2', '4']);
    expect(cells.map((c) => c.style.getPropertyValue('--x'))).toEqual(['0', '2', '0']);
    expect(document.querySelector<HTMLElement>('.grid')!.style.getPropertyValue('--cols')).toBe('4');
  });

  it('shows up and down buttons, and a corner to resize by', () => {
    const onchange = vi.fn();
    draw({ editing: true, onchange }, true);
    expect(document.querySelector('.resize')).not.toBeNull();
    expect(button('Move A up').disabled).toBe(true);
    button('Move A down').click();
    const next: Section[] = onchange.mock.calls[0][0];
    expect([...next].sort((p, q) => p.y - q.y || p.x - q.x).map((s) => s.id)).toEqual(['b', 'a', 'c']);
    // The widths are kept.
    expect(next.map((s) => [s.id, s.w])).toEqual([
      ['b', 6],
      ['a', 6],
      ['c', 12],
    ]);
  });

  it('resizes by the phone’s columns', () => {
    const onchange = vi.fn();
    draw({ editing: true, onchange }, true);
    button('Resize A').dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true, cancelable: true }),
    );
    // Two of four columns, one fewer: one column, which is three of the twelve.
    expect(onchange.mock.calls[0][0].find((s: Section) => s.id === 'a')!.w).toBe(3);
  });
});

describe('the info of a card', () => {
  const loaded = async () => {
    draw({});
    await vi.advanceTimersByTimeAsync(400);
    flushSync();
  };
  const info = (id: string) => document.getElementById(`board-info-${id}`) as HTMLButtonElement;

  it('is on every card, and waits for its data', async () => {
    draw({});
    expect(document.querySelectorAll('button[id^=board-info-]')).toHaveLength(3);
    expect(info('a').disabled).toBe(true);
    await vi.advanceTimersByTimeAsync(400);
    flushSync();
    expect(info('a').disabled).toBe(false);
    expect(info('a').getAttribute('aria-label')).toBe('Information about A');
  });

  it('opens a panel with the card’s filter and the tasks it got, and closes it', async () => {
    await loaded();
    info('a').click();
    flushSync();
    const dialog = document.querySelector('dialog')!;
    expect(dialog.hasAttribute('open')).toBe(true);
    expect(dialog.textContent).toContain('A · Number');
    expect(dialog.textContent).toContain('(every task)');
    // The stand-in server answers every export with two tasks.
    expect(dialog.querySelectorAll('.tasks tbody tr')).toHaveLength(2);
    dialog.querySelector<HTMLButtonElement>('button[aria-label=Close]')!.click();
    flushSync();
    expect(document.querySelector('dialog')).toBeNull();
    // Focus goes back to the button that opened it.
    await vi.advanceTimersByTimeAsync(5);
    expect(document.activeElement).toBe(info('a'));
  });

  it('is not hidden under the tools of the card while the layout is edited', async () => {
    draw({ editing: true });
    expect(document.querySelector('header.editing')).not.toBeNull();
    expect(info('a')).not.toBeNull();
  });
});

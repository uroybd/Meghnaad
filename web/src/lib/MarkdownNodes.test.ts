// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import MarkdownNodes from './MarkdownNodes.svelte';
import { toggled, trees } from './markdown';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

const draw = (text: string, ontoggle?: (line: number) => void) => {
  const tree = trees([text])[0]!;
  app = mount(MarkdownNodes, { target: document.body, props: { nodes: tree, ontoggle } });
  flushSync();
  return document.body;
};

describe('Markdown in a note', () => {
  it('draws the subset: emphasis, code, links, lists, quotes', () => {
    const body = draw('# Plan\n**bold** and *it* and `code` and [docs](https://x.example)\n\n- one\n- two\n\n> quoted');
    expect(body.querySelector('[role=heading]')?.textContent?.trim()).toBe('Plan');
    expect(body.querySelector('strong')?.textContent).toBe('bold');
    expect(body.querySelector('em')?.textContent).toBe('it');
    expect(body.querySelector('p code')?.textContent).toBe('code');
    expect([...body.querySelectorAll('li')].map((l) => l.textContent?.trim())).toEqual(['one', 'two']);
    expect(body.querySelector('blockquote')?.textContent?.trim()).toBe('quoted');
    const a = body.querySelector('a')!;
    expect(a.getAttribute('href')).toBe('https://x.example');
    expect(a.getAttribute('target')).toBe('_blank');
    expect(a.getAttribute('rel')).toContain('noopener');
  });

  it('never turns text into markup or a script link', () => {
    const body = draw('<img src=x onerror=alert(1)> [x](javascript:alert(1)) **<b>hi</b>**');
    expect(body.querySelector('img')).toBeNull();
    expect(body.querySelector('b')).toBeNull();
    expect(body.querySelector('a')).toBeNull();
    expect(body.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(body.textContent).toContain('[x](javascript:alert(1))');
  });

  it('shows checkboxes, ticked as the text says, and tells which line was clicked', () => {
    const calls: number[] = [];
    const body = draw('intro\n- [ ] milk\n- [x] eggs\n- plain', (l) => calls.push(l));
    const boxes = [...body.querySelectorAll<HTMLInputElement>('input[type=checkbox]')];
    expect(boxes.map((b) => b.checked)).toEqual([false, true]);
    boxes[0].click();
    boxes[1].click();
    expect(calls).toEqual([1, 2]);
    // The click does not change the box itself: the text changes, and the box is drawn from it.
    expect(boxes.map((b) => b.checked)).toEqual([false, true]);
  });

  it('only shows the boxes where nothing can be changed', () => {
    const body = draw('- [ ] milk');
    expect(body.querySelector<HTMLInputElement>('input')!.disabled).toBe(true);
  });

  it('flips the box in the text, on the right line', () => {
    expect(toggled('a\n- [ ] b\n- [x] c', 1)).toBe('a\n- [x] b\n- [x] c');
    expect(toggled('a\n- [ ] b', 0)).toBeNull();
  });
});

describe('ticking a box', () => {
  it('modifies the field that holds the text, and not twice at once', async () => {
    const calls: string[][] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async (_url: string, init?: { body?: string }) => {
        const body = JSON.parse(init?.body ?? '{}');
        if (body.args) calls.push(body.args);
        await new Promise((r) => setTimeout(r, 5));
        return new Response(
          JSON.stringify({
            wrote: false,
            result: { kind: 'changed', message: 'ok', tasks: [] },
            command: null,
            feedback: [],
          }),
          { status: 200 },
        );
      }),
    );
    const first = store.tickBox(null, 'u1', 'plan', '- [ ] a\n- [ ] b', 1);
    const second = store.tickBox(null, 'u1', 'plan', '- [ ] a\n- [ ] b', 0);
    await Promise.all([first, second]);
    expect(calls).toEqual([['u1', 'modify', 'plan:- [ ] a\n- [x] b']]);
    // A line that is not a checkbox is not sent.
    await store.tickBox(null, 'u1', 'plan', 'plain', 0);
    expect(calls).toHaveLength(1);
  });
});

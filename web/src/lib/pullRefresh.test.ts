// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PULL_THRESHOLD, pullToRefresh, type PullOptions } from './pullRefresh';

let node: HTMLElement;
let stop: { destroy(): void } | undefined;
const onpull = vi.fn();
const onrefresh = vi.fn(async () => {});

function touch(type: string, y: number, extra: Partial<{ touches: unknown[]; cancelable: boolean }> = {}) {
  const e = new Event(type, { cancelable: extra.cancelable ?? true }) as Event & { touches: unknown[] };
  e.touches = extra.touches ?? (type === 'touchend' ? [] : [{ clientY: y }]);
  node.dispatchEvent(e);
  return e;
}

function mount(options: Partial<PullOptions> = {}) {
  stop = pullToRefresh(node, { onpull, onrefresh, ...options });
}

beforeEach(() => {
  node = document.createElement('div');
  document.body.append(node);
  onpull.mockClear();
  onrefresh.mockClear();
});
afterEach(() => {
  stop?.destroy();
  node.remove();
});

describe('pull to refresh', () => {
  it('follows the finger with some resistance and refreshes when let go far enough', async () => {
    mount();
    touch('touchstart', 100);
    const moved = touch('touchmove', 100 + PULL_THRESHOLD * 2 + 20);
    // The page is held still while it is pulled.
    expect(moved.defaultPrevented).toBe(true);
    const [shown, ready] = onpull.mock.lastCall!;
    expect(shown).toBeGreaterThanOrEqual(PULL_THRESHOLD);
    expect(shown).toBeLessThan(PULL_THRESHOLD * 2 + 20);
    expect(ready).toBe(true);

    touch('touchend', 0);
    await vi.waitFor(() => expect(onrefresh).toHaveBeenCalledTimes(1));
    // It stays at the threshold while refreshing, then goes away.
    await vi.waitFor(() => expect(onpull.mock.lastCall![0]).toBe(0));
  });

  it('does nothing for a pull that is too short', () => {
    mount();
    touch('touchstart', 100);
    touch('touchmove', 100 + PULL_THRESHOLD);
    expect(onpull.mock.lastCall![1]).toBe(false);
    touch('touchend', 0);
    expect(onrefresh).not.toHaveBeenCalled();
    expect(onpull.mock.lastCall![0]).toBe(0);
  });

  it('leaves a scroll alone: the list is not at its top, or the finger goes up', () => {
    mount();
    node.scrollTop = 40;
    touch('touchstart', 100);
    expect(touch('touchmove', 300).defaultPrevented).toBe(false);
    touch('touchend', 0);

    node.scrollTop = 0;
    touch('touchstart', 300);
    expect(touch('touchmove', 200).defaultPrevented).toBe(false);
    touch('touchend', 0);
    expect(onrefresh).not.toHaveBeenCalled();
  });

  it('is off when told so, and for a gesture with two fingers', () => {
    let on = false;
    mount({ enabled: () => on });
    touch('touchstart', 100);
    expect(touch('touchmove', 300).defaultPrevented).toBe(false);
    on = true;
    touch('touchstart', 100, { touches: [{ clientY: 100 }, { clientY: 120 }] });
    expect(touch('touchmove', 300).defaultPrevented).toBe(false);
    touch('touchend', 0);
    expect(onrefresh).not.toHaveBeenCalled();
  });

  it('does not start another refresh while one is under way', async () => {
    let finish: () => void = () => {};
    mount({ onrefresh: () => new Promise<void>((r) => (finish = r)) });
    touch('touchstart', 100);
    touch('touchmove', 300);
    touch('touchend', 0);
    touch('touchstart', 100);
    expect(touch('touchmove', 300).defaultPrevented).toBe(false);
    finish();
  });
});

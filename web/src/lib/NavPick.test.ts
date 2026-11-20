import { afterEach, describe, expect, it, vi } from 'vitest';
import { createRawSnippet, mount, tick, unmount } from 'svelte';
import NavPick from './NavPick.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const label = createRawSnippet(() => ({ render: () => '<span class="lbl">Boards</span>' }));

describe('a page of several in the top bar', () => {
  it('lists each to open directly, then the page’s own list, and marks the one shown', async () => {
    const onpick = vi.fn();
    app = mount(NavPick, {
      target: document.body,
      props: {
        label: 'Boards',
        on: true,
        items: [
          { id: 'home', name: 'Home' },
          { id: 'work', name: 'Work' },
        ],
        current: 'work',
        all: 'All boards',
        onpick,
        children: label,
      },
    });
    // The button is bound once the component's effects have run.
    await tick();
    const button = document.querySelector('button[aria-label=Boards]') as HTMLButtonElement;
    expect(document.querySelector('[role=menu]')).toBeNull();
    button.click();
    await tick();
    const items = [...document.querySelectorAll('[role=menuitem]')] as HTMLButtonElement[];
    expect(items.map((b) => b.textContent?.trim())).toEqual(['Home', 'Work', 'All boards']);
    expect(items[1].getAttribute('aria-current')).toBe('true');

    items[0].click();
    await tick();
    expect(onpick).toHaveBeenCalledWith('home');
    expect(document.querySelector('[role=menu]')).toBeNull();

    button.click();
    await tick();
    (document.querySelectorAll('[role=menuitem]')[2] as HTMLButtonElement).click();
    expect(onpick).toHaveBeenLastCalledWith(null);
  });
});

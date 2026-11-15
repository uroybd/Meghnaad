// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import TagsInput from './TagsInput.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const settle = async () => {
  await Promise.resolve();
  flushSync();
};

/** The field, with `value()` reading what it binds. */
function field(value = '', names = ['errand', 'waiting', 'work']) {
  const props = $state({ id: 'f', names, value });
  app = mount(TagsInput, { target: document.body, props });
  flushSync();
  return { el: document.querySelector<HTMLInputElement>('#f')!, value: () => props.value, props };
}
const pills = () => [...document.querySelectorAll('.tagpill')].map((p) => p.textContent?.trim());
const options = () => [...document.querySelectorAll('[role=option]')].map((o) => o.textContent?.trim());
async function type(el: HTMLInputElement, text: string) {
  el.focus();
  el.value = text;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
}
async function press(el: HTMLInputElement, key: string) {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  el.dispatchEvent(e);
  await settle();
  return e;
}

describe('a field of tags drawn as pills', () => {
  it('shows the tags it is given as pills', () => {
    const f = field('bugfix jira +work');
    expect(pills()).toEqual(['bugfix', 'jira', 'work']);
    expect(f.el.value).toBe('');
  });

  it('makes a pill of each word a space or comma finishes, and binds them all', async () => {
    const f = field();
    await type(f.el, 'home,');
    await type(f.el, 'chore ne');
    expect(pills()).toEqual(['home', 'chore']);
    expect(f.el.value).toBe('ne');
    // The word being typed is part of the value, so saving now keeps it.
    expect(f.value()).toBe('home chore ne');
  });

  it('takes a pill off with its ×, and Backspace in an empty word takes the last one back to edit', async () => {
    const f = field('a b c');
    document.querySelector<HTMLButtonElement>('[aria-label="Remove tag b"]')!.click();
    await settle();
    expect(pills()).toEqual(['a', 'c']);
    expect(f.value()).toBe('a c');
    f.el.focus();
    f.el.setSelectionRange(0, 0);
    expect((await press(f.el, 'Backspace')).defaultPrevented).toBe(true);
    expect(pills()).toEqual(['a']);
    expect(f.el.value).toBe('c');
  });

  it('offers names not already there, and a chosen one becomes a pill', async () => {
    const f = field('errand');
    await type(f.el, 'w');
    expect(options()).toEqual(['waiting', 'work']);
    await press(f.el, 'ArrowDown');
    expect((await press(f.el, 'Enter')).defaultPrevented).toBe(true);
    expect(pills()).toEqual(['errand', 'waiting']);
    expect(f.value()).toBe('errand waiting');
    await type(f.el, 'n');
    expect(document.body.textContent).toContain('“n” will be a new tag.');
  });

  it('lets Enter with nothing chosen submit, the word made a pill', async () => {
    const f = field();
    await type(f.el, 'taxes');
    expect((await press(f.el, 'Enter')).defaultPrevented).toBe(false);
    expect(pills()).toEqual(['taxes']);
    expect(f.value()).toBe('taxes');
  });

  it('reads a value set from outside again', async () => {
    const f = field('a');
    f.props.value = 'x y';
    await settle();
    expect(pills()).toEqual(['x', 'y']);
  });
});

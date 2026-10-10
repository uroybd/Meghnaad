// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import SuggestInput from './SuggestInput.svelte';

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

function field(props: { names: string[]; list?: boolean; noun?: string; value?: string }) {
  app = mount(SuggestInput, { target: document.body, props: { id: 'f', noun: 'project', value: '', ...props } });
  return document.querySelector<HTMLInputElement>('#f')!;
}
const options = () => [...document.querySelectorAll('[role=option]')].map((o) => o.textContent?.trim());
async function type(el: HTMLInputElement, text: string) {
  el.focus();
  el.value = text;
  el.setSelectionRange(text.length, text.length);
  el.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
}
async function press(el: HTMLInputElement, key: string) {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  el.dispatchEvent(e);
  await settle();
  return e;
}

const projects = ['Home', 'Home.Kitchen', 'Work', 'Work.Reports'];

describe('a field that suggests names and takes new ones', () => {
  it('offers names as you type', async () => {
    const el = field({ names: projects });
    await type(el, 'ho');
    expect(options()).toEqual(['Home', 'Home.Kitchen']);
    expect(el.getAttribute('aria-expanded')).toBe('true');
  });

  it('takes the offer you moved to with Enter, and nothing otherwise', async () => {
    const el = field({ names: projects });
    await type(el, 'ho');
    // Enter with nothing chosen is left alone, so the form submits what was typed.
    expect((await press(el, 'Enter')).defaultPrevented).toBe(false);
    expect(el.value).toBe('ho');
    await press(el, 'ArrowDown');
    await press(el, 'ArrowDown');
    expect((await press(el, 'Enter')).defaultPrevented).toBe(true);
    expect(el.value).toBe('Home.Kitchen');
    expect(options()).toEqual([]);
  });

  it('lets a new name through and says it is new', async () => {
    const el = field({ names: projects });
    await type(el, 'Taxes');
    expect(options()).toEqual([]);
    expect(document.body.textContent).toContain('“Taxes” will be a new project.');
    expect((await press(el, 'Tab')).defaultPrevented).toBe(false);
    expect(el.value).toBe('Taxes');
  });

  it('puts the offers away on Escape without closing anything around it', async () => {
    const el = field({ names: projects });
    await type(el, 'wo');
    expect(options().length).toBe(2);
    const e = await press(el, 'Escape');
    expect(e.defaultPrevented).toBe(true);
    expect(options()).toEqual([]);
    expect(el.value).toBe('wo');
    // With no offers open, Escape is not ours to take.
    expect((await press(el, 'Escape')).defaultPrevented).toBe(false);
  });

  it('offers each tag on its own and not one already there', async () => {
    const el = field({ names: ['errand', 'waiting', 'work'], list: true, noun: 'tag' });
    await type(el, 'errand w');
    expect(options()).toEqual(['waiting', 'work']);
    await press(el, 'ArrowDown');
    await press(el, 'Enter');
    expect(el.value).toBe('errand waiting ');
    await type(el, 'errand waiting e');
    expect(options()).toEqual([]);
    await type(el, 'errand waiting n');
    expect(document.body.textContent).toContain('“n” will be a new tag.');
  });

  it('takes an offer that is tapped', async () => {
    const el = field({ names: projects });
    await type(el, 'wo');
    document.querySelectorAll<HTMLElement>('[role=option]')[1].click();
    await settle();
    expect(el.value).toBe('Work.Reports');
  });
});

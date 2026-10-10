// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ConsoleInput from './ConsoleInput.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.promptRequest = null;
  store.promptLine = '';
});

describe('the prompt', () => {
  it('takes the text and the focus when something asks for them, each time', async () => {
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('input') as HTMLInputElement;
    input.value = 'half typed';
    input.dispatchEvent(new Event('input', { bubbles: true }));

    store.requestPrompt('1,3-5 ');
    flushSync();
    await Promise.resolve();
    expect(input.value).toBe('1,3-5 ');
    expect(document.activeElement).toBe(input);
    expect(input.selectionStart).toBe(6);

    input.value = '1,3-5 modify';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    store.requestPrompt('2 ');
    flushSync();
    expect(input.value).toBe('2 ');
  });

  it('ignores a request made before it appeared', () => {
    store.requestPrompt('old ');
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    expect((document.querySelector('input') as HTMLInputElement).value).toBe('');
  });
});

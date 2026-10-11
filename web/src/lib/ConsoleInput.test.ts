// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { parseSettings } from './appsettings';
import ConsoleInput from './ConsoleInput.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.promptRequest = null;
  store.promptLine = '';
  store.appSettings = null;
  vi.restoreAllMocks();
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

describe('task commands at the prompt', () => {
  function submit(text: string) {
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('input') as HTMLInputElement;
    input.value = text;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    flushSync();
    return run;
  }

  it('sends what the command stands for, and keeps what was typed for the history', () => {
    store.appSettings = parseSettings('{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" } ] }');
    const run = submit('task project:Home fail');
    expect(run).toHaveBeenCalledWith({ line: 'project:Home done outcome:fail' }, 'task project:Home fail');
  });

  it('sends any other line as it was typed', () => {
    store.appSettings = parseSettings('{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" } ] }');
    const run = submit('add fail the test');
    expect(run).toHaveBeenCalledWith({ line: 'add fail the test' }, 'add fail the test');
  });
});

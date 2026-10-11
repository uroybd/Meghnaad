// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { parseSettings } from './appsettings';
import { review } from './review.svelte';
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

describe('tasksh review at the prompt', () => {
  function type(text: string, key = 'Enter') {
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('input') as HTMLInputElement;
    input.value = text;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
    flushSync();
    return input;
  }

  afterEach(() => review.close());

  it('starts a review in the console instead of sending the word to Taskwarrior', () => {
    const start = vi.spyOn(review, 'start').mockResolvedValue();
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    store.view = 'tasks';
    type('review 5');
    expect(start).toHaveBeenCalledWith({ limit: 5, surface: 'console' });
    expect(run).not.toHaveBeenCalled();
    expect(store.view).toBe('console');
  });

  it('leaves a line that is not the command alone', () => {
    const start = vi.spyOn(review, 'start').mockResolvedValue();
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    type('project:Work review');
    expect(start).not.toHaveBeenCalled();
    expect(run).toHaveBeenCalled();
  });

  it('takes the typed answers while a review asks, an empty Enter among them, and none go to the history', () => {
    review.surface = 'console';
    review.phase = 'running';
    const respond = vi.spyOn(review, 'respond').mockResolvedValue();
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    const input = type('s');
    expect(respond).toHaveBeenLastCalledWith('s');
    expect(input.value).toBe('');
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    flushSync();
    expect(respond).toHaveBeenLastCalledWith('');
    expect(run).not.toHaveBeenCalled();
    expect(store.history).not.toContain('s');
  });

  it('says it is a review: the prompt, and the menu', () => {
    review.surface = 'console';
    review.phase = 'running';
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    expect(document.querySelector('.ps')?.textContent).toBe('review>');
    expect(document.querySelector('[data-testid=review-hint]')?.textContent).toContain('(Enter) Mark as reviewed');
  });

  it('quits on Escape', () => {
    review.surface = 'console';
    review.phase = 'running';
    const quit = vi.spyOn(review, 'quit').mockReturnValue();
    type('', 'Escape');
    expect(quit).toHaveBeenCalled();
  });
});

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
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
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
    expect((document.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
  });
});

describe('task commands at the prompt', () => {
  function submit(text: string) {
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
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

describe('a line of several statements at the prompt', () => {
  function type(text: string) {
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
    input.value = text;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    flushSync();
    return input;
  }

  it('runs the statements as a chain, with a task command written out in each, and keeps what was typed', () => {
    store.appSettings = parseSettings('{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" } ] }');
    const chain = vi.spyOn(store, 'runChain').mockResolvedValue();
    type('count && 3 fail || add x ; list');
    expect(chain).toHaveBeenCalledWith(
      [
        { join: null, line: 'count' },
        { join: '&&', line: '3 done outcome:fail' },
        { join: '||', line: 'add x' },
        { join: ';', line: 'list' },
      ],
      'count && 3 fail || add x ; list',
    );
  });

  it('prints what is wrong with a line that is not one, and runs nothing', () => {
    const chain = vi.spyOn(store, 'runChain').mockResolvedValue();
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    const before = store.entries.length;
    type('count &&');
    expect(chain).not.toHaveBeenCalled();
    expect(run).not.toHaveBeenCalled();
    expect(store.entries.at(-1)?.result).toEqual({ kind: 'error', message: 'Nothing after ‘&&’.' });
    store.entries.length = before;
  });

  it('will not run review inside one', () => {
    const chain = vi.spyOn(store, 'runChain').mockResolvedValue();
    const before = store.entries.length;
    type('count ; review');
    expect(chain).not.toHaveBeenCalled();
    expect(store.entries.at(-1)?.result).toMatchObject({ kind: 'error' });
    store.entries.length = before;
  });

  it('is off while the line runs, and back after the last statement', () => {
    const input = type('');
    store.chainBusy = true;
    flushSync();
    expect(input.disabled).toBe(true);
    expect(input.placeholder).toBe('Running…');
    store.chainBusy = false;
    flushSync();
    expect(input.disabled).toBe(false);
  });
});

describe('after Enter', () => {
  it('empties the prompt, and the line the next prompt starts from, before the page can change', () => {
    // A command typed on the Tasks page opens the Console, which builds a new prompt that starts from the shared line.
    // That line is kept by an effect that has not run yet at that moment, so it has to be emptied with the field.
    vi.spyOn(store, 'run').mockImplementation(async () => {
      store.view = 'console';
      return {} as never;
    });
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
    input.value = 'count';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(store.promptLine).toBe('count');

    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    // No flush: the shared line is already empty, as a prompt built right now would read it.
    expect(store.promptLine).toBe('');
    flushSync();
    expect(input.value).toBe('');
    store.view = 'tasks';
  });
});

describe('tasksh review at the prompt', () => {
  function type(text: string, key = 'Enter') {
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
    input.value = text;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
    flushSync();
    return input;
  }
  const press = (key: string, init: KeyboardEventInit = {}, on: EventTarget = window) => {
    const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
    on.dispatchEvent(e);
    flushSync();
    return e;
  };
  const reviewing = (phase: 'running' | 'asking' | 'modifying' = 'running') => {
    review.surface = 'console';
    review.phase = phase;
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
  };

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

  it('takes a key as the answer at once, with no Enter after it, and Enter alone is the usual one', () => {
    const pressed = vi.spyOn(review, 'press').mockResolvedValue();
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    reviewing();
    const s = press('s');
    expect(pressed).toHaveBeenLastCalledWith('s');
    expect(s.defaultPrevented).toBe(true);
    press('Enter');
    expect(pressed).toHaveBeenLastCalledWith('Enter');
    expect(run).not.toHaveBeenCalled();
    expect(store.history).not.toContain('s');
    expect((document.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
  });

  it('leaves alone the keys that are not an answer: arrows, Tab, and anything with Ctrl or Cmd', () => {
    const pressed = vi.spyOn(review, 'press').mockResolvedValue();
    reviewing();
    press('ArrowUp');
    press('Tab');
    press('r', { ctrlKey: true });
    press('s', { metaKey: true });
    expect(pressed).not.toHaveBeenCalled();
  });

  it('does not take the key of a button, which has its own Enter and Space', () => {
    const pressed = vi.spyOn(review, 'press').mockResolvedValue();
    reviewing();
    const b = document.querySelector('button') as HTMLButtonElement;
    press('Enter', {}, b);
    expect(pressed).not.toHaveBeenCalled();
  });

  it('has the prompt off while a review asks, and a bar of buttons over it', () => {
    reviewing();
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
    expect(document.querySelector('.ps')?.textContent).toBe('review>');
    expect(input.disabled).toBe(true);
    const bar = [...document.querySelectorAll('[role=toolbar] button')].map((b) =>
      b.textContent?.replace(/\s+/g, ' ').trim(),
    );
    expect(bar).toEqual([
      'Reviewed Enter',
      'Skip s',
      'Edit e',
      'Modify m',
      'Complete c',
      'Delete d',
      'Open as card',
      'Quit q',
    ]);
  });

  it('shows Yes and No for a question, answered with y or n', () => {
    review.question = 'Do you want to modify all pending recurrences of this same task?';
    const pressed = vi.spyOn(review, 'press').mockResolvedValue();
    reviewing('asking');
    expect(document.querySelector('[data-testid=review-question]')?.textContent).toContain('recurrences');
    expect(
      [...document.querySelectorAll('[role=group] button')].map((b) => b.textContent?.replace(/\s+/g, ' ').trim()),
    ).toEqual(['Yes y', 'No n']);
    press('y');
    expect(pressed).toHaveBeenLastCalledWith('y');
  });

  it('turns the prompt on to type what to change: Enter sends it, Escape cancels', async () => {
    const respond = vi.spyOn(review, 'respond').mockResolvedValue();
    const modify = vi.spyOn(review, 'modify').mockResolvedValue();
    reviewing('modifying');
    const input = document.querySelector('textarea') as HTMLTextAreaElement;
    expect(input.disabled).toBe(false);
    input.value = '+tag project:X';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    flushSync();
    expect(respond).toHaveBeenCalledWith('+tag project:X');
    expect(input.value).toBe('');
    // A letter is typed here, not taken as a choice.
    const pressed = vi.spyOn(review, 'press').mockResolvedValue();
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 's', bubbles: true }));
    expect(pressed).not.toHaveBeenCalled();
    press('Escape');
    expect(modify).toHaveBeenCalledWith('');
  });

  it('moves the review to the card with the Open as card button', () => {
    const toCard = vi.spyOn(review, 'toCard').mockReturnValue();
    reviewing();
    [...document.querySelectorAll<HTMLButtonElement>('[role=toolbar] button')]
      .find((b) => b.textContent?.includes('Open as card'))!
      .click();
    expect(toCard).toHaveBeenCalled();
  });

  it('quits on Escape', () => {
    const quit = vi.spyOn(review, 'quit').mockReturnValue();
    reviewing();
    press('Escape');
    expect(quit).toHaveBeenCalled();
  });
});

describe('the active context at the prompt', () => {
  const withActive = (name: string | null) =>
    (store.config = { reports: [], config: { settings: {}, contexts: {}, active_context: name } } as never);
  afterEach(() => (store.config = null));

  it('is shown, and cleared by the button, which runs `context none`', () => {
    withActive('work');
    const run = vi.spyOn(store, 'run').mockResolvedValue({} as never);
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    expect(document.querySelector('[data-testid="active-context"]')?.textContent).toBe('work');
    document.querySelector<HTMLButtonElement>('button[aria-label="Clear the context"]')!.click();
    expect(run).toHaveBeenCalledWith({ line: 'context none' }, 'context none');
  });

  it('is not there when no context is active', () => {
    withActive(null);
    app = mount(ConsoleInput, { target: document.body });
    flushSync();
    expect(document.querySelector('[data-testid="active-context"]')).toBeNull();
  });
});

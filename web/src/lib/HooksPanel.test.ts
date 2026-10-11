// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';

const api = vi.hoisted(() => ({
  getHooks: vi.fn(),
  checkHooks: vi.fn(),
  putHooks: vi.fn(),
}));
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), ...api }));

/** A stand-in for CodeMirror, which does not need to be exercised here: it holds the text and the marks. */
const editors = vi.hoisted(() => [] as { text: string; marks: unknown[]; change: (t: string) => void }[]);
vi.mock('./hookEditor', () => ({
  createHookEditor: (_parent: HTMLElement, initial: string, onChange: (t: string) => void) => {
    const state = { text: initial, marks: [] as unknown[], change: onChange };
    editors.push(state);
    return {
      text: () => state.text,
      setText: (t: string) => {
        state.text = t;
        onChange(t);
      },
      mark: (p: unknown[]) => {
        state.marks = p;
      },
      focus: () => {},
      destroy: () => {},
    };
  },
}));

import HooksPanel from './HooksPanel.svelte';
import { HOOKS_EXAMPLE } from './hooksExample';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;

async function open(saved: string) {
  api.getHooks.mockResolvedValue(saved);
  const before = editors.length;
  app = mount(HooksPanel, { target: document.body });
  await vi.waitFor(() => expect(editors.length).toBeGreaterThan(before));
  await tick();
  return editors[editors.length - 1];
}

/** Type into the editor, and let the check that follows a moment after typing run. */
async function typeIn(editor: { text: string; change: (t: string) => void }, text: string) {
  editor.text = text;
  editor.change(text);
  await vi.advanceTimersByTimeAsync(450);
  await tick();
}

const button = (label: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label) as
    HTMLButtonElement | undefined;
const status = () => document.querySelector('p.status')?.textContent?.replace(/\s+/g, ' ').trim() ?? '';

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  editors.length = 0;
  for (const f of Object.values(api)) f.mockReset();
  vi.spyOn(store, 'notify').mockImplementation(() => {});
});

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the Hooks tab', () => {
  it('shows the saved script, checks what is typed, and names the hooks it defines', async () => {
    api.checkHooks.mockResolvedValue({ ok: true, defined: ['on_add', 'on_modify'] });
    const editor = await open('function on_add(task) end');
    expect(editor.text).toBe('function on_add(task) end');
    expect(button('Save')?.disabled).toBe(true);

    await typeIn(editor, 'function on_add(task) end\nfunction on_modify(old, new) end');
    expect(api.checkHooks).toHaveBeenLastCalledWith('function on_add(task) end\nfunction on_modify(old, new) end');
    expect(status()).toBe('Defines on_add, on_modify.');
    expect(editor.marks).toEqual([]);
    expect(button('Save')?.disabled).toBe(false);
  });

  it('saves what was typed, tells the page, and has nothing left to save', async () => {
    api.checkHooks.mockResolvedValue({ ok: true, defined: ['on_add'] });
    api.putHooks.mockResolvedValue(['on_add']);
    const editor = await open('');
    await typeIn(editor, 'function on_add(task) end');
    button('Save')!.click();
    await tick();
    await vi.advanceTimersByTimeAsync(0);
    expect(api.putHooks).toHaveBeenCalledWith('function on_add(task) end');
    expect(store.notify).toHaveBeenCalledWith('Hooks saved: on_add.', 'ok');
    await tick();
    expect(button('Save')?.disabled).toBe(true);
    expect(document.body.textContent).toContain('Saved.');
  });

  it('says what is wrong and where, marks the line, and does not let a broken script be saved', async () => {
    api.checkHooks.mockResolvedValue({ ok: false, line: 3, message: 'unexpected symbol near <eof>' });
    const editor = await open('');
    await typeIn(editor, 'function on_add(task)\n  task.tags:add(\n');
    expect(status()).toBe('Line 3: unexpected symbol near <eof>');
    expect(editor.marks).toEqual([{ line: 3, message: 'unexpected symbol near <eof>' }]);
    expect(button('Save')?.disabled).toBe(true);
  });

  it('offers an example when there is nothing, and removes the hooks when emptied', async () => {
    api.checkHooks.mockResolvedValue({ ok: true, defined: ['on_add', 'on_modify'] });
    api.putHooks.mockResolvedValue([]);
    const editor = await open('');
    expect(status()).toContain('No hooks');
    button('Insert an example')!.click();
    await tick();
    expect(editor.text).toBe(HOOKS_EXAMPLE);
    expect(button('Insert an example')).toBeUndefined();

    // Saved scripts that are emptied are removed, with no check needed.
    unmount(app!);
    document.body.innerHTML = '';
    const again = await open('function on_add(task) end');
    await typeIn(again, '   ');
    expect(button('Remove the hooks')?.disabled).toBe(false);
    button('Remove the hooks')!.click();
    await tick();
    await vi.advanceTimersByTimeAsync(0);
    expect(api.putHooks).toHaveBeenLastCalledWith('   ');
    expect(store.notify).toHaveBeenLastCalledWith('Hooks removed.', 'ok');
  });

  it('shows why a save failed, and keeps the edits', async () => {
    api.checkHooks.mockResolvedValue({ ok: true, defined: ['on_add'] });
    const { ApiError } = await import('./api');
    api.putHooks.mockRejectedValue(new ApiError('The script was not saved. line 2: boom', 400));
    const editor = await open('');
    await typeIn(editor, 'function on_add(task) end');
    button('Save')!.click();
    await tick();
    await vi.advanceTimersByTimeAsync(0);
    await tick();
    expect(document.querySelector('[role=alert]')?.textContent).toContain('line 2: boom');
    expect(editor.text).toBe('function on_add(task) end');
    expect(button('Save')?.disabled).toBe(false);
  });

  it('says so when the script cannot be read, instead of showing an empty editor to overwrite it', async () => {
    api.getHooks.mockRejectedValue(new Error('Could not reach the server.'));
    app = mount(HooksPanel, { target: document.body });
    await vi.waitFor(() => expect(document.body.textContent).toContain('The hooks could not be read'));
    expect(button('Save')).toBeUndefined();
  });
});

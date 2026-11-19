// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { pageNamed, runCall } from './keyCalls';
import { commandLine, pendingLabel, resolveKeymap, stepKey, type Pending } from './keymap';
import { selection } from './selection.svelte';
import { store } from './store.svelte';
import defaults from '../../public/keymaps/default.json';

afterEach(() => {
  document.body.innerHTML = '';
  selection.clear();
  store.view = 'tasks';
  store.live = null;
  vi.restoreAllMocks();
});

/** A Tasks table of `n` rows, as ReportTable draws them, with the rows the store holds. */
function table(n: number) {
  const rows = Array.from({ length: n }, (_, i) => ({
    uuid: `u${i}`,
    id: i + 1,
    description: `task ${i}`,
    start: i === 1 ? 100 : null,
    end: null,
    annotations: [],
  }));
  document.body.innerHTML = `<main><table><tbody>${rows
    .map((r) => `<tr data-uuid="${r.uuid}" tabindex="0"><td>${r.description}</td></tr>`)
    .join('')}</tbody></table></main>`;
  store.live = { result: { kind: 'report', rows } } as unknown as typeof store.live;
}
const focused = () => (document.activeElement as HTMLElement | null)?.dataset.uuid;

describe('keys and : lines in the web app', () => {
  it('reads pages by number or name', () => {
    expect(pageNamed('3')).toBe('tags');
    expect(pageNamed('9')).toBe('settings');
    expect(pageNamed('bu')).toBe('burndown');
    expect(pageNamed('0')).toBeNull();
    expect(pageNamed('nope')).toBeNull();
  });

  it('runs a : line the engine reads, and leaves the rest to task', () => {
    expect(commandLine(':tabn Tags')).toEqual({ action: 'page.go', args: ['Tags'] });
    expect(commandLine(': tabn')).toBeNull();
    expect(runCall(commandLine(':tabn Tags')!)).toBe(true);
    expect(store.view).toBe('tags');
    runCall(commandLine(':tabp')!);
    expect(store.view).toBe('projects');
    // The `:` key puts `:` on the prompt.
    runCall({ action: 'app.commandLine' });
    expect(store.promptRequest?.text).toBe(':');
  });

  it('moves the chosen row with counts, gg and G', () => {
    table(10);
    const keymap = resolveKeymap({}, defaults as Record<string, unknown>);
    let pending: Pending = {};
    const press = (k: string) => {
      const s = stepKey(keymap, 'tasks', pending, k);
      pending = s.pending;
      if (s.call) runCall(s.call);
    };
    press('j');
    expect(focused()).toBe('u0');
    press('3');
    expect(pendingLabel(pending)).toBe('3');
    press('j');
    expect(focused()).toBe('u3');
    press('2');
    press('k');
    expect(focused()).toBe('u1');
    for (const [line, at] of [
      [':+3', 'u4'],
      [':-2', 'u2'],
    ] as const) {
      const call = commandLine(line);
      if (call) runCall(call);
      expect(focused()).toBe(at);
    }
    press('G');
    expect(focused()).toBe('u9');
    press('g');
    expect(pendingLabel(pending)).toBe('g');
    press('g');
    expect(focused()).toBe('u0');
    press('5');
    press('G');
    expect(focused()).toBe('u4');
    press('space');
    expect(selection.uuids).toEqual(['u4']);
    expect(focused()).toBe('u5');
  });

  it('toggles the chosen task, and starts ticked ones that are not started', async () => {
    table(3);
    const act = vi.spyOn(store, 'act').mockResolvedValue(null);
    runCall({ action: 'move.row', args: ['2'] });
    runCall({ action: 'tasks.toggle' });
    expect(act.mock.calls[0][1]).toEqual(['u1', 'stop']);
    runCall({ action: 'move.row', args: ['1'] });
    runCall({ action: 'tasks.toggle' });
    expect(act.mock.calls[1][1]).toEqual(['u0', 'start']);
    selection.toggleAll(['u0', 'u1', 'u2']);
    runCall({ action: 'tasks.start' });
    expect(act.mock.calls[2][1]).toEqual(['u0', 'u2', 'start']);
  });
});

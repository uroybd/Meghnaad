import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { actionOf, chordOf, keyLabel, resolveKeymap as resolve } from './keymap';

// The asset the page serves.
const defaults = JSON.parse(readFileSync('public/keymaps/default.json', 'utf8')) as Record<string, unknown>;
const resolveKeymap = (settings: Record<string, unknown>) => resolve(settings, defaults);

const press = (key: string, mods: Partial<KeyboardEventInit> = {}) => new KeyboardEvent('keydown', { key, ...mods });

describe('the keymap', () => {
  it('has / for the prompt by default', () => {
    const km = resolveKeymap({});
    expect(km.keys['app.prompt']).toEqual(['/']);
    expect(actionOf(km, press('/'))).toBe('app.prompt');
    expect(km.problems).toEqual([]);
  });

  it('takes keys for every front end, then the web app’s own', () => {
    const km = resolveKeymap({ keys: { 'app.prompt': ['ctrl+k', '/'] }, web: { keys: { 'app.prompt': 'Ctrl-P' } } });
    expect(km.keys['app.prompt']).toEqual(['ctrl+shift+p']);
    expect(actionOf(km, press('/'))).toBeNull();
    expect(actionOf(km, press('P', { ctrlKey: true, shiftKey: true }))).toBe('app.prompt');
  });

  it('says what is wrong and ignores the terminal app’s own keys', () => {
    const km = resolveKeymap({ keys: { 'app.nope': 'x' }, tui: { keys: { 'tasks.done': 'z' } } });
    expect(km.problems).toEqual(['keys: “app.nope” is no action']);
  });

  it('has no keys until the defaults are there', () => {
    expect(resolve({ keys: { 'app.prompt': '/' } }, null).keys).toEqual({});
  });

  it('writes a key the browser gives as settings.json writes it', () => {
    expect(chordOf(press('K', { ctrlKey: true, shiftKey: true }))).toBe('ctrl+shift+k');
    expect(chordOf(press('?', { shiftKey: true }))).toBe('?');
    expect(chordOf(press('ArrowDown'))).toBe('down');
    expect(chordOf(press('Shift', { shiftKey: true }))).toBeNull();
    expect(keyLabel('ctrl+k')).toBe('Ctrl+K');
  });
});

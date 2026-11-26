import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  actionOf,
  chordOf,
  have,
  keyCommands,
  keyLabel,
  migrateSettings,
  resolveKeymap as resolve,
  type CommandsDoc,
} from './keymap';

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
    const km = resolveKeymap({
      version: 6,
      keys: { app: { 'ctrl+k': 'app.prompt' } },
      web: { keys: { app: { 'Ctrl-P': 'app.prompt', 'ctrl+k': null, '/': null } } },
    });
    expect(km.keys['app.prompt']).toEqual(['ctrl+shift+p']);
    expect(actionOf(km, press('/'))).toBeNull();
    expect(actionOf(km, press('P', { ctrlKey: true, shiftKey: true }))).toBe('app.prompt');
  });

  it('says what is wrong and ignores the terminal app’s own keys', () => {
    const km = resolveKeymap({
      version: 6,
      keys: { app: { x: 'app.nope' } },
      tui: { keys: { tasks: { z: 'tasks.done' } } },
    });
    expect(km.problems).toEqual(['keys.app: x runs “app.nope”, which is skipped: “app.nope” is no action']);
  });

  it('has no keys until the defaults are there', () => {
    expect(resolve({ version: 6, keys: { app: { '/': 'app.prompt' } } }, null).keys).toEqual({});
  });

  it('lists the commands of this build, and leaves out those of a later one', () => {
    const doc = JSON.parse(readFileSync('public/keymaps/commands.json', 'utf8')) as CommandsDoc;
    expect(have('unreleased', '0.1.0')).toBe(true);
    expect(have('0.8.11', '0.8.10')).toBe(false);
    expect(have('0.8.10', '0.8.10')).toBe(true);
    expect(have('0.8.9', '0.10.0')).toBe(true);
    const later = { ...doc, commands: doc.commands.map((c, i) => (i ? c : { ...c, since: '999.0.0' })) };
    const names = keyCommands(later, '998.0.0').map((c) => c.written);
    expect(names).not.toContain(':tabn[ext]');
    expect(names).toContain(':tabp[revious]');
    expect(keyCommands(doc).find((c) => c.written === ':tabp[revious]')?.link).toBe(`${doc.docs}#tabprevious`);
  });

  it('brings settings of version 1 up to date, once', () => {
    const next = migrateSettings({ web: { keys: { 'app.help': 'h' }, theme: 'dark' } }, defaults);
    expect(next).toEqual({ version: 6, web: { keys: { app: { h: 'app.help', '?': null } }, theme: 'dark' } });
    expect(migrateSettings(next!, defaults)).toBeNull();
  });

  it('writes a key the browser gives as settings.json writes it', () => {
    expect(chordOf(press('K', { ctrlKey: true, shiftKey: true }))).toBe('ctrl+shift+k');
    expect(chordOf(press('?', { shiftKey: true }))).toBe('?');
    expect(chordOf(press('ArrowDown'))).toBe('down');
    expect(chordOf(press('Shift', { shiftKey: true }))).toBeNull();
    expect(keyLabel('ctrl+k')).toBe('Ctrl+K');
  });
});

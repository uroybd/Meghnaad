// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { getAppSettings, getConfig, getTaskrcText, putAppSettings, runCli } from './api';
import { parseSettings } from './appsettings';
import SettingsPage from './SettingsPage.svelte';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  runCli: vi.fn(),
  getConfig: vi.fn(),
  getTaskrcText: vi.fn(),
  getAppSettings: vi.fn(),
  putAppSettings: vi.fn(),
}));

/** A taskrc with neither tasksh part. */
function bare(): ConfigResponse {
  return {
    config: { udas: {}, reports: {}, contexts: {}, active_context: null, urgency: {}, settings: {} },
    reports: [{ name: 'next', description: null, columns: [], filter: null, sort: null }],
    urgency_defaults: {},
    urgency_inherit: false,
  } as unknown as ConfigResponse;
}

const ok = { result: { kind: 'text', lines: ['Config modified.'] }, wrote: false, feedback: [] };
const tick = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  flushSync();
};

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  vi.mocked(getTaskrcText).mockResolvedValue('');
  vi.mocked(getConfig).mockResolvedValue(bare());
  vi.mocked(runCli).mockResolvedValue(ok as never);
  vi.mocked(getAppSettings).mockResolvedValue({});
  vi.mocked(putAppSettings).mockResolvedValue({ ok: true });
  store.appSettings = parseSettings('');
  store.config = bare();
  store.settingsTab = 'taskrc';
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.clearAllMocks();
});

const tab = (name: string) => document.getElementById(`settings-tab-${name}`) as HTMLButtonElement;
const panel = (name: string) => document.getElementById(`settings-panel-${name}`) as HTMLElement;
const button = (within: HTMLElement, text: string) =>
  [...within.querySelectorAll('button')].find((b) => b.textContent?.trim() === text) as HTMLButtonElement;

describe('the Settings page', () => {
  it('has a tab each for general, taskrc, tasksh, commands, hooks and urgency, and shows one at a time', async () => {
    app = mount(SettingsPage, { target: document.body });
    await tick();
    expect([...document.querySelectorAll('[role=tab]')].map((t) => t.textContent?.trim())).toEqual([
      'General',
      'taskrc',
      'tasksh',
      'Commands',
      'Hooks',
      'Urgency',
    ]);
    expect(panel('taskrc').hidden).toBe(false);
    expect(panel('tasksh').hidden).toBe(true);

    tab('tasksh').click();
    await tick();
    expect(tab('tasksh').getAttribute('aria-selected')).toBe('true');
    expect(panel('taskrc').hidden).toBe(true);
    expect(panel('tasksh').hidden).toBe(false);
  });

  it('opens on the tab the store asks for', async () => {
    store.openSettings('urgency');
    app = mount(SettingsPage, { target: document.body });
    await tick();
    expect(store.view).toBe('settings');
    expect(panel('urgency').hidden).toBe(false);
  });

  it('gives each tab its own Apply, and a tab with edits a dot that stays while another is shown', async () => {
    app = mount(SettingsPage, { target: document.body });
    await tick();
    const apply = (name: string) => button(panel(name), 'Apply');
    expect(apply('taskrc')).toBeTruthy();
    expect(apply('tasksh')).toBeTruthy();
    expect(apply('commands')).toBeTruthy();
    expect(apply('urgency')).toBeTruthy();
    expect(apply('taskrc').disabled).toBe(true);
    expect(tab('taskrc').querySelector('.dot')).toBeNull();

    const text = panel('taskrc').querySelector('textarea') as HTMLTextAreaElement;
    text.value = 'bulk=9';
    text.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    expect(apply('taskrc').disabled).toBe(false);
    expect(tab('taskrc').querySelector('.dot')).not.toBeNull();
    expect(tab('tasksh').querySelector('.dot')).toBeNull();

    // Another tab, and back: the edit is still there.
    tab('urgency').click();
    await tick();
    expect(tab('taskrc').querySelector('.dot')).not.toBeNull();
    tab('taskrc').click();
    await tick();
    expect((panel('taskrc').querySelector('textarea') as HTMLTextAreaElement).value).toBe('bulk=9');
  });
});

describe('the tasksh tab', () => {
  async function open() {
    store.settingsTab = 'tasksh';
    app = mount(SettingsPage, { target: document.body });
    await tick();
  }

  it('says what is missing and what Apply will write', async () => {
    await open();
    const p = panel('tasksh');
    const status = [...p.querySelectorAll('ul[aria-label=Status] li')].map((li) =>
      li.textContent?.replace(/\s+/g, ' '),
    );
    expect(status[0]).toContain('not defined');
    expect(status[1]).toContain('not defined');
    expect(p.textContent).toContain('Apply will write 6 settings');
    expect(p.textContent).toContain('uda.reviewed.type=date');
    expect(button(p, 'Apply').disabled).toBe(false);
  });

  it('writes the lines one at a time, columns before the rest of the report, then reads the settings again', async () => {
    await open();
    button(panel('tasksh'), 'Apply').click();
    await tick();
    await tick();
    const sent = vi.mocked(runCli).mock.calls.map(([input]) => (input.args ?? []).slice(2));
    expect(sent).toEqual([
      ['config', 'uda.reviewed.type', 'date'],
      ['config', 'uda.reviewed.label', 'Reviewed'],
      ['config', 'report._reviewed.columns', 'uuid'],
      ['config', 'report._reviewed.description', 'Tasksh review report.  Adjust the filter to your needs.'],
      ['config', 'report._reviewed.sort', 'reviewed+,modified+'],
      [
        'config',
        'report._reviewed.filter',
        '( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )',
      ],
    ]);
    // No questions are asked, and only the tasksh names are touched.
    for (const [input] of vi.mocked(runCli).mock.calls)
      expect(input.args?.slice(0, 2)).toEqual(['rc.confirmation:no', 'rc.verbose:nothing']);
    expect(getConfig).toHaveBeenCalled();
  });

  it('stops at the first line the server refuses, and says why', async () => {
    vi.mocked(runCli).mockResolvedValueOnce(ok as never);
    vi.mocked(runCli).mockResolvedValueOnce({ result: { kind: 'error', message: 'Nope.' }, wrote: false } as never);
    await open();
    button(panel('tasksh'), 'Apply').click();
    await tick();
    await tick();
    // Two `config` lines went out (the second was refused); the others are the names refreshed with the settings.
    const sent = vi.mocked(runCli).mock.calls.map(([input]) => input.args ?? []);
    expect(sent.filter((a) => a.includes('config'))).toHaveLength(2);
    expect(sent.filter((a) => !a.includes('config'))).toEqual([['_names']]);
    expect(panel('tasksh').querySelector('[role=alert]')?.textContent).toBe('Nope.');
  });
});

describe('the commands tab', () => {
  async function open(json = '') {
    store.appSettings = parseSettings(json);
    store.settingsTab = 'commands';
    app = mount(SettingsPage, { target: document.body });
    await tick();
  }
  const type = (el: HTMLInputElement, value: string) => {
    el.value = value;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  };
  const inputs = () => [...panel('commands').querySelectorAll<HTMLInputElement>('ul[aria-label=Commands] input.mono')];
  const apply = () => button(panel('commands'), 'Apply');

  it('shows the saved commands', async () => {
    await open('{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" } ] }');
    expect(inputs().map((i) => i.value)).toEqual(['fail', 'done outcome:fail']);
    expect(apply().disabled).toBe(true);
  });

  it('adds one, keeps Apply off until it is valid, and saves settings.json as text', async () => {
    await open('{ "future": 1 }');
    button(panel('commands'), 'Add a command').click();
    await tick();
    expect(apply().disabled).toBe(true);
    const [name, command] = inputs();
    type(name, 'two words');
    type(command, 'done outcome:fail');
    await tick();
    expect(panel('commands').textContent).toContain('a name is one word');
    expect(apply().disabled).toBe(true);

    type(name, 'fail');
    await tick();
    expect(apply().disabled).toBe(false);
    expect(tab('commands').querySelector('.dot')).not.toBeNull();

    apply().click();
    await tick();
    await tick();
    expect(putAppSettings).toHaveBeenCalledTimes(1);
    // Other settings are kept, and the commands are written out.
    expect(JSON.parse(vi.mocked(putAppSettings).mock.calls[0][0])).toEqual({
      future: 1,
      taskCommands: [{ name: 'fail', command: 'done outcome:fail' }],
    });
    expect(getAppSettings).toHaveBeenCalled();
  });

  it('refuses a name used twice', async () => {
    await open('{ "taskCommands": [ { "name": "a", "command": "done" } ] }');
    button(panel('commands'), 'Add a command').click();
    await tick();
    const f = inputs();
    type(f[2], 'a');
    type(f[3], 'stop');
    await tick();
    expect(panel('commands').textContent).toContain('already a command named a');
    expect(apply().disabled).toBe(true);
  });

  it('says what in the saved file cannot be used', async () => {
    await open('{ "taskCommands": [ { "name": "ok", "command": "done" }, { "name": "no good", "command": "x" } ] }');
    expect(panel('commands').querySelector('[role=alert]')?.textContent).toContain('One entry in settings.json can');
  });
});

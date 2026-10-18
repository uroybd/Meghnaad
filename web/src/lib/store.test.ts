import { beforeEach, describe, expect, it, vi } from 'vitest';
import { runCli } from './api';
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli: vi.fn() }));

import { reportArgs } from './cmdline';
import { HISTORY_MAX, shortenUuids, store, type Entry } from './store.svelte';

beforeEach(() => {
  store.history = [];
  store.lastCommand = '';
  localStorage.clear();
});

describe('remember', () => {
  it('shows the last command with a task prefix, and keeps history without it', () => {
    store.remember('project:Home list');
    expect(store.lastCommand).toBe('task project:Home list');
    expect(store.history).toEqual(['project:Home list']);
    store.remember('task done');
    expect(store.history).toEqual(['project:Home list', 'done']);
  });

  it('keeps only the last 20, newest last', () => {
    for (let i = 0; i < 30; i++) store.remember(`cmd ${i}`);
    expect(store.history).toHaveLength(HISTORY_MAX);
    expect(store.history[0]).toBe('cmd 10');
    expect(store.history.at(-1)).toBe('cmd 29');
  });

  it('moves a repeated command to the end instead of duplicating it', () => {
    store.remember('a');
    store.remember('b');
    store.remember('a');
    expect(store.history).toEqual(['b', 'a']);
  });

  it('live-table updates replace each other so typing a filter does not flood history', () => {
    store.remember('add milk'); // a real command
    for (const f of ['p', 'pr', 'proj', 'project:Home work']) store.remember(f, true);
    expect(store.history).toEqual(['add milk', 'project:Home work']);
    store.remember('1 done'); // a real command is kept, and is not replaced by what follows
    store.remember('list', true);
    expect(store.history).toEqual(['add milk', 'project:Home work', '1 done', 'list']);
    store.remember('next', true);
    expect(store.history).toEqual(['add milk', 'project:Home work', '1 done', 'next']);
  });

  it('persists to localStorage', () => {
    store.remember('persist me');
    expect(JSON.parse(localStorage.getItem('tw-web-history')!)).toEqual(['persist me']);
  });

  it('ignores blanks', () => {
    store.remember('   ');
    expect(store.history).toEqual([]);
  });
});

describe('shortenUuids', () => {
  it('shortens full uuids for display only', () => {
    expect(shortenUuids('task 074a5bbe-86f4-4fae-b6cd-cb118064ef58 done')).toBe('task 074a5bbe done');
    expect(shortenUuids('task 12345678 done')).toBe('task 12345678 done');
  });
});

describe('focusReport', () => {
  const key = (filter: string, report: string) => JSON.stringify(reportArgs(filter, report));

  it('leaves the table to be run when no result is handed over', () => {
    // The Tasks view skips a run whose arguments match `liveKey`, so a filter that was never run
    // must not be recorded there (clicking a calendar day showed the unfiltered table).
    store.liveKey = key('', 'next');
    store.focusReport('next', ['due:2026-10-12']);
    expect(store.view).toBe('tasks');
    expect(store.filter).toBe('due:2026-10-12');
    expect(store.liveKey).not.toBe(key('due:2026-10-12', 'next'));
  });

  it('marks the handed-over result as the one on screen', () => {
    const from = {
      id: 1,
      input: { line: 'x' },
      title: 'x',
      result: null,
      loading: false,
      failure: null,
      at: 0,
    } as Entry;
    store.focusReport('next', ['project:Home'], from);
    expect(store.liveKey).toBe(key('project:Home', 'next'));
    expect(store.live?.input).toEqual({ args: reportArgs('project:Home', 'next') });
  });
});

describe('running a report', () => {
  const report = {
    result: { kind: 'report', report: 'list', rows: [], columns: [], matched: 0, sort: null, breaks: [] },
    command: { name: 'list', report: true, filter: ['project:Home'] },
    wrote: false,
  };

  beforeEach(() => {
    store.entries = [];
    store.live = null;
    vi.mocked(runCli).mockResolvedValue(report as never);
  });

  it('prints the table in the Console when typed there', async () => {
    store.view = 'console';
    const e = await store.run({ line: 'project:Home list' });
    expect(store.view).toBe('console');
    expect(store.entries.map((x) => x.id)).toEqual([e.id]);
    expect(e.result?.kind).toBe('report');
    expect(store.live).toBeNull();
  });

  it('opens it in the Tasks view when typed anywhere else', async () => {
    store.view = 'calendar';
    const e = await store.run({ line: 'project:Home list' });
    expect(store.view).toBe('tasks');
    expect(store.entries.find((x) => x.id === e.id)).toBeUndefined();
    expect(store.report).toBe('list');
    expect(store.filter).toBe('project:Home');
    expect(store.live?.result?.kind).toBe('report');
  });
});

describe('opening a task from an address', () => {
  const UUID = '1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d';
  const answer = (lines: string[]) => ({ wrote: false, result: { kind: 'text', lines }, command: null, feedback: [] });
  beforeEach(() => {
    store.detail = null;
    store.toast = null;
    vi.mocked(runCli).mockReset();
  });

  it('opens the drawer for the one task that starts so, asking with an attribute filter', async () => {
    vi.mocked(runCli).mockResolvedValue(answer([UUID]) as never);
    expect(await store.openFromAddress('/task/12345678')).toBe('/task/1a2b3c4d');
    expect(vi.mocked(runCli).mock.calls[0][0]).toEqual({ args: ['uuid.startswith:12345678', 'uuids'] });
    expect(store.detail?.uuid).toBe(UUID);
  });

  it('does not ask again for the task that is already open', async () => {
    store.detail = { uuid: UUID, from: null };
    expect(await store.openFromAddress('/task/1a2b3c4d')).toBe('/task/1a2b3c4d');
    expect(runCli).not.toHaveBeenCalled();
  });

  it('closes the drawer for any other address', async () => {
    store.detail = { uuid: UUID, from: null };
    expect(await store.openFromAddress('/')).toBeUndefined();
    expect(store.detail).toBeNull();
    expect(runCli).not.toHaveBeenCalled();
  });

  it('says so, and goes to the front page, when no task or several start so', async () => {
    vi.mocked(runCli).mockResolvedValue(answer([]) as never);
    expect(await store.openFromAddress('/task/deadbeef')).toBe('/');
    expect(store.toast?.text).toContain('No task starts with deadbeef');
    vi.mocked(runCli).mockResolvedValue(answer([UUID, '1a2b3c4d-0000-0000-0000-000000000000']) as never);
    expect(await store.openFromAddress('/task/1a2b3c4d')).toBe('/');
    expect(store.toast?.text).toContain('More than one task');
    expect(store.detail).toBeNull();
  });
});

describe('showing a tag or a project on its page', () => {
  it('goes to the Projects page, closes the drawer, and asks again each time', () => {
    store.view = 'tasks';
    store.detail = { uuid: 'x', from: null };
    store.showProject('Home.Kitchen');
    expect(store.view).toBe('projects');
    expect(store.detail).toBeNull();
    const first = store.projectFocus!;
    expect(first.project).toBe('Home.Kitchen');
    store.showProject('Home.Kitchen');
    expect(store.projectFocus!.n).toBe(first.n + 1);
    store.projectFocus = null;
  });
});

describe('hidden reports', () => {
  const report = (name: string) => ({ name, description: null, columns: [], filter: null, sort: null });
  const names = () => store.offeredReports.map((r) => r.name);

  beforeEach(() => {
    store.config = { reports: [report('next'), report('_reviewed'), report('list')] } as never;
    store.report = 'next';
    store.setShowHiddenReports(false);
  });

  it('are left out of what is offered until they are shown, and the choice is remembered', () => {
    expect(store.hasHiddenReports).toBe(true);
    expect(names()).toEqual(['next', 'list']);
    store.setShowHiddenReports(true);
    expect(names()).toEqual(['next', '_reviewed', 'list']);
    expect(localStorage.getItem('tw-web-show-hidden-reports')).toBe('1');
    store.setShowHiddenReports(false);
    expect(localStorage.getItem('tw-web-show-hidden-reports')).toBe('0');
  });

  it('stay offered while one of them is in use', () => {
    store.report = '_reviewed';
    expect(names()).toEqual(['next', '_reviewed', 'list']);
  });

  it('give the switch nothing to do when there are none', () => {
    store.config = { reports: [report('next'), report('list')] } as never;
    expect(store.hasHiddenReports).toBe(false);
  });
});

describe('the names offered for completion', () => {
  it('come from one request for both lists, with the parents of a project and without virtual tags', async () => {
    vi.mocked(runCli).mockResolvedValue({
      result: { kind: 'json', value: { tags: ['next', 'work', 'OVERDUE'], projects: ['Home.Kitchen', 'Work'] } },
      wrote: false,
    } as never);
    store.tags = ['mine'];
    store.projects = [];
    await store.refreshNames();
    expect(vi.mocked(runCli)).toHaveBeenLastCalledWith({ args: ['_names'] });
    expect(store.tags).toEqual(['mine', 'next', 'work']);
    expect(store.projects).toEqual(['Home', 'Home.Kitchen', 'Work']);
  });

  it('leave what is known alone when the answer is not a list, or the request fails', async () => {
    store.tags = ['keep'];
    store.projects = ['Keep'];
    vi.mocked(runCli).mockResolvedValue({ result: { kind: 'text', lines: ['x'] }, wrote: false } as never);
    await store.refreshNames();
    vi.mocked(runCli).mockRejectedValue(new Error('offline'));
    await store.refreshNames();
    expect(store.tags).toEqual(['keep']);
    expect(store.projects).toEqual(['Keep']);
  });
});

describe('a page running in a context of its own', () => {
  const sent = () => vi.mocked(runCli).mock.calls.at(-1)![0];

  it('sends its choice with what it runs, and the console sends none', async () => {
    store.config = null;
    vi.mocked(runCli).mockResolvedValue({ result: { kind: 'text', lines: [] } } as never);
    store.contextChoices = {};
    store.setContextChoice('summary', 'work');
    await store.runPanel('summary', { args: ['summary'] });
    expect(sent()).toMatchObject({ args: ['rc.context=work', 'summary'] });
    store.setContextChoice('summary', undefined);
    await store.refresh(store.panels.summary!);
    expect(sent()).toMatchObject({ args: ['summary'] });
    await store.run({ line: 'count' });
    expect(sent()).toMatchObject({ line: 'count' });
    store.setContextChoice('tasks', 'none');
    await store.run({ line: 'count' });
    expect(sent()).toMatchObject({ line: 'count' });
  });

  it('fetches the table again when its context changes, even for the same filter', async () => {
    vi.mocked(runCli).mockResolvedValue({ result: { kind: 'text', lines: [] } } as never);
    store.setContextChoice('tasks', undefined);
    await store.runLive({ args: ['next'] });
    expect(store.liveContext).toBe('');
    store.setContextChoice('tasks', 'none');
    await store.runLive({ args: ['next'] });
    expect(store.liveContext).toBe('none');
    expect(sent()).toMatchObject({ args: ['rc.context=none', 'next'] });
  });
});

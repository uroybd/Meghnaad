// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import TaskInfo from './TaskInfo.svelte';
import { store } from './store.svelte';
import type { ConfigResponse, Row } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  store.config = null;
  vi.unstubAllGlobals();
});

const task = (uuid: string): Row =>
  ({
    uuid,
    status: 'pending',
    description: 'Write the report',
    project: null,
    priority: null,
    tags: [],
    annotations: [],
    entry: 1,
    modified: 1,
    start: null,
    end: null,
    due: null,
    wait: null,
    scheduled: null,
    until: null,
    depends: [],
    blocked: false,
    blocking: false,
    recur: null,
    parent: null,
    mask: null,
    imask: null,
    extra: {},
    urgency: 1,
    id: 1,
    orphans: [],
    active_seconds: null,
    sessions: [],
  }) as Row;

const answer = (value: unknown) =>
  new Response(JSON.stringify({ wrote: false, result: { kind: 'json', value }, command: null, feedback: [] }), {
    status: 200,
  });

const settle = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

describe('a task’s history', () => {
  it('is not asked for until the section is opened, and then once', async () => {
    store.config = { colors: {}, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
    const fetchMock = vi
      .fn()
      .mockImplementation(async () =>
        answer([
          { at: 1_700_000_000, changes: [{ kind: 'set', prop: 'project', old: null, value: 'Home', date: false }] },
        ]),
      );
    vi.stubGlobal('fetch', fetchMock);
    app = mount(TaskInfo, { target: document.body, props: { task: task('11111111-1111-1111-1111-111111111111') } });
    await settle();
    expect(fetchMock).not.toHaveBeenCalled();

    const details = document.querySelector('details.hist') as HTMLDetailsElement;
    details.open = true;
    details.dispatchEvent(new Event('toggle'));
    await settle();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const body = JSON.parse(fetchMock.mock.calls[0][1].body);
    expect(body.args).toEqual(['_history', '11111111-1111-1111-1111-111111111111']);
    expect(document.querySelector('[data-testid="history"]')).not.toBeNull();
    expect(details.textContent).toContain('1 moment');
  });

  it('says so when nothing is recorded', async () => {
    store.config = { colors: {}, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => answer([])),
    );
    app = mount(TaskInfo, { target: document.body, props: { task: task('22222222-2222-2222-2222-222222222222') } });
    const details = document.querySelector('details.hist') as HTMLDetailsElement;
    details.open = true;
    details.dispatchEvent(new Event('toggle'));
    await settle();
    expect(details.textContent).toContain('Nothing is recorded');
  });
});

describe('the tasks around this one', () => {
  const OTHER = '33333333-3333-3333-3333-333333333333';
  const report = (rows: Row[]) =>
    new Response(JSON.stringify({ wrote: false, result: { kind: 'report', rows }, command: null, feedback: [] }), {
      status: 200,
    });

  it('lists what it depends on as lines to tap, asking for nothing until then', async () => {
    store.config = { colors: {}, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    app = mount(TaskInfo, {
      target: document.body,
      props: { task: { ...task('44444444-4444-4444-4444-444444444444'), depends: [OTHER], blocked: true } },
    });
    await settle();
    expect(document.querySelectorAll('button.line')).toHaveLength(1);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('finds the tasks waiting on it only when asked, and shows them without another request', async () => {
    store.config = { colors: {}, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
    const waiting = { ...task(OTHER), description: 'Send the invitations' };
    const fetchMock = vi.fn().mockImplementation(async () => report([waiting]));
    vi.stubGlobal('fetch', fetchMock);
    const me = '55555555-5555-5555-5555-555555555555';
    app = mount(TaskInfo, { target: document.body, props: { task: { ...task(me), blocking: true } } });
    await settle();
    expect(fetchMock).not.toHaveBeenCalled();

    (document.querySelector('button.ask') as HTMLButtonElement).click();
    await settle();
    expect(JSON.parse(fetchMock.mock.calls[0][1].body).args).toEqual(['status:pending', `depends.has:${me}`, 'all']);
    const line = document.querySelector('button.line') as HTMLButtonElement;
    expect(line.textContent).toContain('Send the invitations');
    line.click();
    await settle();
    expect(document.querySelector('[data-testid="depitem-card"]')).not.toBeNull();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('sends a click on the project, or one of its parts, to the Projects page', async () => {
    store.config = { colors: {}, config: { udas: {}, settings: {}, reports: {} } } as unknown as ConfigResponse;
    app = mount(TaskInfo, {
      target: document.body,
      props: { task: { ...task('66666666-6666-6666-6666-666666666666'), project: 'Home.Kitchen' } },
    });
    await settle();
    const parts = [...document.querySelectorAll<HTMLButtonElement>('.path button')];
    expect(parts.map((b) => b.textContent)).toEqual(['Home', 'Kitchen']);
    parts[1].click();
    flushSync();
    expect(store.view).toBe('projects');
    expect(store.projectFocus?.project).toBe('Home.Kitchen');
    store.projectFocus = null;
    store.view = 'tasks';
  });
});

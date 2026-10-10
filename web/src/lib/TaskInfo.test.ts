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

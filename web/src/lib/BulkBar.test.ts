// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import BulkBar from './BulkBar.svelte';
import { selection } from './selection.svelte';
import { store, type Entry } from './store.svelte';
import type { ConfigResponse, Row } from './types';

const U1 = '11111111-1111-1111-1111-111111111111';
const U2 = '22222222-2222-2222-2222-222222222222';
const U3 = '33333333-3333-3333-3333-333333333333';

const row = (uuid: string, id: number | null, description: string) =>
  ({ uuid, id, description, status: 'pending', tags: [], annotations: [], extra: {}, depends: [] }) as unknown as Row;

let app: ReturnType<typeof mount> | null = null;
let sent: { args: string[]; approved?: string[]; confirmed?: boolean }[] = [];

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  store.config = {
    colors: {},
    config: {
      udas: { size: { name: 'size', type: 'string', label: null, values: ['S', 'M'], default: null, indicator: null } },
      settings: {},
      reports: {},
    },
  } as unknown as ConfigResponse;
  store.live = {
    id: 1,
    input: { args: ['next'] },
    title: 'next',
    result: {
      kind: 'report',
      report: 'next',
      description: null,
      columns: [],
      rows: [row(U1, 1, 'one'), row(U2, 2, 'two'), row(U3, null, 'three')],
      matched: 3,
      breaks: [],
      sort: '',
    },
    loading: false,
    failure: null,
    at: 0,
  } as unknown as Entry;
  sent = [];
  vi.stubGlobal(
    'fetch',
    vi.fn().mockImplementation(async (_url: string, init: { body: string }) => {
      const body = JSON.parse(init.body);
      sent.push(body);
      const wrote = ['done', 'delete', 'modify'].some((w) => body.args?.includes(w));
      const result = wrote
        ? { kind: 'changed', message: 'Done.', tasks: [] }
        : {
            kind: 'report',
            report: 'next',
            description: null,
            columns: [],
            rows: [],
            matched: 0,
            breaks: [],
            sort: '',
          };
      return new Response(JSON.stringify({ wrote, result, command: null, feedback: [] }), { status: 200 });
    }),
  );
  app = mount(BulkBar, { target: document.body });
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  selection.clear();
  store.live = null;
  store.config = null;
  store.promptRequest = null;
  vi.unstubAllGlobals();
});

const settle = async () => {
  for (let i = 0; i < 6; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  flushSync();
};
const bar = () => document.querySelector('[data-testid="bulkbar"]');
const button = (text: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(text)) as HTMLButtonElement;
const pick = (...uuids: string[]) => {
  selection.toggleAll(uuids);
  flushSync();
};

describe('the bulk bar', () => {
  it('appears only when more than one task is selected', () => {
    expect(bar()).toBeNull();
    pick(U1);
    expect(bar()).toBeNull();
    pick(U2);
    expect(bar()?.textContent).toContain('2 selected');
    pick(U1, U2); // untick both
    expect(bar()).toBeNull();
  });

  it('completes every selected task in one command, answering its questions yes', async () => {
    pick(U1, U2);
    button('Complete').click();
    await settle();
    expect(sent[0]).toMatchObject({ args: [U1, U2, 'done'], approved: [U1, U2], confirmed: true });
    expect(selection.count).toBe(0);
    expect(bar()).toBeNull();
  });

  it('deletes after a second click, as the single delete does', async () => {
    pick(U1, U2, U3);
    button('Delete').click();
    await settle();
    expect(sent).toHaveLength(0);
    expect(button('Delete 3?').textContent).toContain('Click again');
    button('Delete 3?').click();
    await settle();
    expect(sent[0]).toMatchObject({ args: [U1, U2, U3, 'delete'], approved: [U1, U2, U3] });
    expect(selection.count).toBe(0);
  });

  it('keeps the selection when the command did not go through', async () => {
    pick(U1, U2);
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockImplementation(
          async () =>
            new Response(
              JSON.stringify({ wrote: false, result: { kind: 'error', message: 'No.' }, command: null, feedback: [] }),
            ),
        ),
    );
    button('Complete').click();
    await settle();
    expect(selection.count).toBe(2);
  });

  it('puts the tasks at the prompt: numbers as a list, a whole uuid for one without a number', () => {
    pick(U1, U2, U3);
    button('Command').click();
    expect(store.promptRequest?.text).toBe(`1,2 ${U3} `);
    const first = store.promptRequest!.n;
    button('Command').click();
    expect(store.promptRequest!.n).toBe(first + 1);
    expect(selection.count).toBe(3);
  });

  it('modifies with the fields and values entered, and says what it will do first', async () => {
    pick(U1, U2);
    button('Modify…').click();
    flushSync();
    expect(document.querySelector('dialog')?.textContent).toContain('Modify 2 tasks');
    // First line: project (the default field).
    const input = document.querySelector('#bm-1') as HTMLInputElement;
    input.value = 'Home';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    // A second line: tags to add.
    button('Add a change').click();
    flushSync();
    const keys = document.querySelectorAll<HTMLSelectElement>('[data-testid="bulk-entry"] select');
    keys[1].value = '+';
    keys[1].dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    const tags = document.querySelector('#bm-2') as HTMLInputElement;
    tags.value = 'urgent work';
    tags.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(document.querySelector('[data-testid="bulk-summary"]')?.textContent).toContain(
      'On 2 tasks: set project to Home, add +urgent +work.',
    );
    button('Modify 2 tasks').click();
    await settle();
    expect(sent[0]).toMatchObject({ args: [U1, U2, 'modify', 'project:Home', '+urgent', '+work'], confirmed: true });
    expect(selection.count).toBe(0);
    expect(document.querySelector('dialog')).toBeNull();
  });

  it('will not apply tags with none named, or a field set twice', () => {
    pick(U1, U2);
    button('Modify…').click();
    flushSync();
    button('Add a change').click();
    flushSync();
    const keys = document.querySelectorAll<HTMLSelectElement>('[data-testid="bulk-entry"] select');
    keys[1].value = '+';
    keys[1].dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(document.querySelector('dialog [role="alert"]')?.textContent).toContain('Add tags');
    expect(button('Modify 2 tasks').disabled).toBe(true);
    keys[1].value = 'project';
    keys[1].dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(document.querySelector('dialog [role="alert"]')?.textContent).toContain('set twice');
  });
});

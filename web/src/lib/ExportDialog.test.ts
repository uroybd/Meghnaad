// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import ExportDialog from './ExportDialog.svelte';
import { runCli } from './api';
import { saveAs } from './download';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli: vi.fn() }));
vi.mock('./download', () => ({ saveAs: vi.fn() }));

const TEXT = '[\n{"id":1,"description":"one","due":"20261119T180000Z","uuid":"u1"}\n]\n';
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
  // jsdom has no modal dialogs.
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };
  localStorage.clear();
  store.config = {
    colors: {},
    config: { udas: {}, settings: {}, reports: {}, contexts: {}, active_context: null },
  } as unknown as ConfigResponse;
  vi.mocked(runCli).mockResolvedValue({
    result: { kind: 'file', name: 'tasks-next.json', text: TEXT, count: 1 },
  } as never);
});
afterEach(() => {
  vi.clearAllMocks();
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

async function open() {
  app = mount(ExportDialog, {
    target: document.body,
    props: { args: ['export', 'next'], columns: [], onclose: vi.fn() },
  });
  flushSync();
  await tick();
  await Promise.resolve();
  flushSync();
}
const radio = (value: string) => document.querySelector(`input[type=radio][value="${value}"]`) as HTMLInputElement;
const save = () =>
  [...document.querySelectorAll('button')].find((b) => /download|save/i.test(b.textContent ?? '')) as HTMLButtonElement;

describe('the Export dialog', () => {
  it('offers JSON, CSV and TSV, in that order', async () => {
    await open();
    const formats = [...document.querySelectorAll('fieldset')][0].querySelectorAll('input[type=radio]');
    expect([...formats].map((r) => (r as HTMLInputElement).value)).toEqual(['json', 'csv', 'tsv']);
  });

  it('saves a TSV file and remembers the choice', async () => {
    await open();
    radio('tsv').click();
    flushSync();
    save().click();
    flushSync();
    expect(saveAs).toHaveBeenCalledWith(
      'tasks-next.tsv',
      'id\tdescription\tdue\tuuid\ttime_tracking\n1\tone\t2026-11-19 18:00:00\tu1\t0\n',
      'text/tab-separated-values',
    );
    expect(JSON.parse(localStorage.getItem('tw-web-export')!).format).toBe('tsv');
  });
});

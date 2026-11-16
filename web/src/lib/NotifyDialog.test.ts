// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount } from 'svelte';
import NotifyDialog from './NotifyDialog.svelte';
import { runCli } from './api';
import { store } from './store.svelte';
import type { ConfigResponse } from './types';

vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli: vi.fn() }));

let app: ReturnType<typeof mount> | null = null;
const withUdas = (udas: Record<string, unknown>) =>
  ({
    colors: {},
    config: { udas, settings: {}, reports: {}, contexts: {}, active_context: null },
  }) as unknown as ConfigResponse;

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  store.config = withUdas({});
  vi.spyOn(store, 'settingsChanged').mockResolvedValue(undefined as never);
  vi.mocked(runCli).mockResolvedValue({ result: { kind: 'text', lines: [] } } as never);
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

describe('the Reminders dialog', () => {
  it('adds the remind_in UDA with config, as review setup does', async () => {
    app = mount(NotifyDialog, { target: document.body });
    flushSync();
    expect(document.querySelector('[data-testid=remind-uda]')?.textContent).toContain('no remind_in attribute');
    const button = [...document.querySelectorAll('button')].find((b) => b.textContent?.includes('remind_in'));
    button?.click();
    await tick();
    await Promise.resolve();
    await Promise.resolve();
    expect(vi.mocked(runCli).mock.calls.map(([r]) => r.args)).toEqual([
      ['rc.confirmation:no', 'rc.verbose:nothing', 'config', 'uda.remind_in.type', 'string'],
      ['rc.confirmation:no', 'rc.verbose:nothing', 'config', 'uda.remind_in.label', 'Remind in'],
    ]);
    expect(store.settingsChanged).toHaveBeenCalled();
  });

  it('says so when it is there', () => {
    store.config = withUdas({ remind_in: { name: 'remind_in', type: 'string', label: 'Remind in' } });
    app = mount(NotifyDialog, { target: document.body });
    flushSync();
    expect(document.querySelector('[data-testid=remind-uda]')?.textContent).toContain('is in your taskrc');
    expect([...document.querySelectorAll('button')].some((b) => b.textContent?.includes('remind_in'))).toBe(false);
  });
});

// `runCli` and `importTasks` when the engine runs in the page: the answers are the server's, the server is not asked,
// settings that a command changed are saved on the server, and a browser that cannot start the engine says why.

import { beforeEach, describe, expect, it, vi } from 'vitest';

const engine = {
  start: vi.fn(),
  stop: vi.fn(),
  run: vi.fn(),
  import: vi.fn(),
  setConfig: vi.fn(),
  pushSoon: vi.fn(),
};
let available = true;
vi.mock('../engine/client.svelte', () => ({ engine, engineAvailable: () => available }));

const config = { config: { settings: { weekstart: 'monday' } }, reports: [] };
const fetchMock = vi.fn();

function json(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), { status }));
}

async function api() {
  vi.resetModules();
  return import('./api');
}

beforeEach(() => {
  available = true;
  for (const f of Object.values(engine)) f.mockReset();
  engine.start.mockResolvedValue(undefined);
  engine.setConfig.mockResolvedValue(null);
  fetchMock.mockReset();
  fetchMock.mockImplementation((url: string) => (url === '/api/config' ? json(config) : json({ ok: true })));
  vi.stubGlobal('fetch', fetchMock);
});

const urls = () => fetchMock.mock.calls.map((c) => c[0]);

describe('the engine in the page', () => {
  it('starts with the saved settings, runs a command locally, and never asks the server to', async () => {
    const { runCli } = await api();
    engine.run.mockResolvedValue(
      JSON.stringify({ wrote: false, result: { kind: 'text', lines: ['3'] }, command: null }),
    );
    const res = await runCli({ line: 'count' });
    expect(res.result).toEqual({ kind: 'text', lines: ['3'] });
    expect(engine.start).toHaveBeenCalledWith(JSON.stringify(config.config));
    const sent = engine.run.mock.calls[0][0];
    expect(sent).toMatchObject({ line: 'count', tz: expect.any(Number), now: expect.any(Number) });
    expect(urls()).toEqual(['/api/config']);
    expect(engine.pushSoon).not.toHaveBeenCalled();
  });

  it('pushes after a write, without waiting for it', async () => {
    const { runCli } = await api();
    engine.run.mockResolvedValue(
      JSON.stringify({ wrote: true, result: { kind: 'text', lines: ['Created task 1.'] }, command: null }),
    );
    await runCli({ args: ['add', 'x'] });
    expect(engine.pushSoon).toHaveBeenCalledTimes(1);
  });

  it('turns a refused request into the same error the server gave', async () => {
    const { runCli, ApiError } = await api();
    engine.run.mockResolvedValue(JSON.stringify({ error: 'send exactly one of `line` or `args`' }));
    const err = await runCli({}).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 400, message: 'send exactly one of `line` or `args`' });
  });

  it('saves settings that a command changed on the server, and gives them to the engine', async () => {
    const { runCli } = await api();
    const changed = { settings: { weekstart: 'sunday' } };
    engine.run.mockResolvedValue(
      JSON.stringify({
        wrote: false,
        result: { kind: 'text', lines: ['Config modified.'] },
        command: null,
        config: changed,
      }),
    );
    const res = await runCli({ line: 'config weekstart sunday' });
    expect('config' in res).toBe(false);
    const put = fetchMock.mock.calls.find((c) => c[0] === '/api/config/engine');
    expect(put?.[1]).toMatchObject({ method: 'PUT', body: JSON.stringify(changed) });
    expect(engine.setConfig).toHaveBeenCalledWith(JSON.stringify(changed));
  });

  it('keeps the engine as it was, and says so, when the settings could not be saved', async () => {
    const { runCli } = await api();
    fetchMock.mockImplementation((url: string) => (url === '/api/config' ? json(config) : json({ error: 'no' }, 500)));
    engine.run.mockResolvedValue(
      JSON.stringify({
        wrote: false,
        result: { kind: 'text', lines: ['Config modified.'] },
        command: null,
        config: { settings: {} },
      }),
    );
    const res = await runCli({ line: 'config x y' });
    expect(res.result).toEqual({ kind: 'error', message: 'The settings could not be saved. Nothing was changed.' });
    expect(engine.setConfig).not.toHaveBeenCalled();
  });

  it('imports through the engine, and pushes what was imported', async () => {
    const { importTasks } = await api();
    engine.import.mockResolvedValue(
      JSON.stringify({
        applied: true,
        added: 1,
        modified: 0,
        skipped: 0,
        lines: [],
        more: 0,
        warnings: [],
        feedback: [],
      }),
    );
    const report = await importTasks('[]', true);
    expect(report.added).toBe(1);
    expect(engine.import).toHaveBeenCalledWith('[]', true, expect.any(Number));
    expect(engine.pushSoon).toHaveBeenCalledTimes(1);
    expect(urls()).not.toContain('/api/import?apply=1');
    engine.pushSoon.mockClear();
    engine.import.mockResolvedValue(
      JSON.stringify({
        applied: false,
        added: 1,
        modified: 0,
        skipped: 0,
        lines: [],
        more: 0,
        warnings: [],
        feedback: [],
      }),
    );
    await importTasks('[]', false);
    expect(engine.pushSoon).not.toHaveBeenCalled();
  });

  it('takes the settings again when they are read again', async () => {
    const { getConfig } = await api();
    await getConfig();
    await getConfig();
    expect(engine.start).toHaveBeenCalledTimes(1);
    expect(engine.setConfig).toHaveBeenCalledTimes(1);
  });
});

describe('where the engine cannot run', () => {
  it('says so, and why, instead of asking the server to run commands', async () => {
    available = false;
    const { runCli, importTasks } = await api();
    const err = await runCli({ line: 'count' }).catch((e: unknown) => e);
    expect(err).toMatchObject({ message: expect.stringContaining('Web Workers and WebAssembly') });
    await expect(importTasks('[]', true)).rejects.toThrow('Web Workers and WebAssembly');
    expect(engine.start).not.toHaveBeenCalled();
    expect(urls()).toEqual(['/api/config']);
  });

  it('gives the reason it could not start to every command, and stops what it started', async () => {
    const { runCli, getConfig } = await api();
    engine.start.mockRejectedValue(new Error('the module could not be compiled'));
    await expect(runCli({ line: 'count' })).rejects.toThrow(
      'The engine could not start in this browser: the module could not be compiled',
    );
    await expect(runCli({ line: 'next' })).rejects.toThrow('could not be compiled');
    expect(engine.stop).toHaveBeenCalledTimes(1);
    expect(engine.run).not.toHaveBeenCalled();
    // The settings are still read: the page works around the engine's failure to start, it does not crash on it.
    await expect(getConfig()).resolves.toBeTruthy();
  });
});

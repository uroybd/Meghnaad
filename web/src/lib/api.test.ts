import { afterEach, describe, expect, it, vi } from 'vitest';
import { runCli, getConfig } from './api';

const limit = () => new Response('error code: 1102', { status: 500 });
const ok = (body: unknown) => new Response(JSON.stringify(body), { status: 200 });

afterEach(() => vi.unstubAllGlobals());

describe('a request cut off by Cloudflare\'s resource limit (1102)', () => {
  it('is tried once more when it is only a read', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(limit()).mockResolvedValueOnce(ok({ result: { kind: 'text', lines: [] } }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(runCli({ args: ['project:Home', 'next'] })).resolves.toBeTruthy();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('is tried once more when it is a GET', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(limit()).mockResolvedValueOnce(ok({ config: {} }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(getConfig()).resolves.toBeTruthy();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('is never repeated when it might have changed something', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => limit());
    vi.stubGlobal('fetch', fetchMock);
    await expect(runCli({ args: ['add', 'Buy milk'] })).rejects.toThrow(/1102/);
    await expect(runCli({ line: '3 done' })).rejects.toThrow(/1102/);
    expect(fetchMock).toHaveBeenCalledTimes(2); // once each
  });

  it('gives up after one more try, saying what happened', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => limit());
    vi.stubGlobal('fetch', fetchMock);
    await expect(runCli({ args: ['next'] })).rejects.toThrow(/out of CPU time/);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('does not touch our own errors', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => new Response(JSON.stringify({ error: 'bad filter' }), { status: 400 }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(runCli({ args: ['next'] })).rejects.toThrow('bad filter');
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});

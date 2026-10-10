import { afterEach, describe, expect, it, vi } from 'vitest';
import { runCli, getConfig, importTasks, importTaskrcUrl, MAX_IMPORT_BYTES } from './api';
// The Worker's source, as text, to read its size limit from.
import workerSource from '../../../crates/worker/src/lib.rs?raw';

const limit = () => new Response('error code: 1102', { status: 500 });
const ok = (body: unknown) => new Response(JSON.stringify(body), { status: 200 });

afterEach(() => vi.unstubAllGlobals());

describe("a request cut off by Cloudflare's resource limit (1102)", () => {
  it('is tried once more when it is only a read', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(limit())
      .mockResolvedValueOnce(ok({ result: { kind: 'text', lines: [] } }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(runCli({ args: ['project:Home', 'next'] })).resolves.toBeTruthy();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('is tried once more when it is a GET', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(limit())
      .mockResolvedValueOnce(ok({ config: {} }));
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
    const fetchMock = vi
      .fn()
      .mockImplementation(async () => new Response(JSON.stringify({ error: 'bad filter' }), { status: 400 }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(runCli({ args: ['next'] })).rejects.toThrow('bad filter');
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});

describe('importing a file', () => {
  it('knows the same size limit as the Worker', () => {
    // Two copies of one number, in two languages: this is what ties them together.
    const mb = /const MAX_IMPORT_BYTES: usize = (\d+) \* 1024 \* 1024;/.exec(workerSource)?.[1];
    expect(Number(mb) * 1024 * 1024).toBe(MAX_IMPORT_BYTES);
  });

  it('sends the file as it is, with whether to apply it and the time zone', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => ok({ applied: false, added: 1, modified: 0, skipped: 0 }));
    vi.stubGlobal('fetch', fetchMock);
    const file = '[{"description":"a"}]';
    await importTasks(file, false);
    await importTasks(file, true);
    const [first, second] = fetchMock.mock.calls;
    expect(first[0]).toMatch(/^\/api\/import\?apply=0&tz=-?\d+$/);
    expect(second[0]).toMatch(/^\/api\/import\?apply=1&tz=-?\d+$/);
    expect(second[1]).toMatchObject({ method: 'POST', body: file });
  });

  it('is never repeated on its own, even though its body parses as JSON', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => limit());
    vi.stubGlobal('fetch', fetchMock);
    await expect(importTasks('[{"description":"a"}]', true)).rejects.toThrow(/1102/);
    await expect(importTasks('{"description":"a"}', true)).rejects.toThrow(/1102/);
    expect(fetchMock).toHaveBeenCalledTimes(2); // once each
  });

  it("shows the server's reason when a file is refused", async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(JSON.stringify({ error: 'Task 2: due: bad' }), { status: 400 })),
    );
    await expect(importTasks('x', false)).rejects.toThrow('Task 2: due: bad');
  });
});

describe('importing a taskrc from a link', () => {
  it("sends only the link, as text, and shows the server's reason when it is refused", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(ok({ udas: 1, reports: 0, contexts: 0, blocked: [], ignored: [], warnings: [], text: '' }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ error: 'the link must start with https://' }), { status: 400 }),
      );
    vi.stubGlobal('fetch', fetchMock);
    await expect(importTaskrcUrl('https://example.com/rc')).resolves.toMatchObject({ udas: 1 });
    const [path, init] = fetchMock.mock.calls[0];
    expect(path).toBe('/api/config/taskrc/url');
    expect(init.method).toBe('POST');
    expect(init.body).toBe('https://example.com/rc');
    await expect(importTaskrcUrl('http://x')).rejects.toThrow('the link must start with https://');
  });

  it('is never repeated on its own', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => limit());
    vi.stubGlobal('fetch', fetchMock);
    await expect(importTaskrcUrl('https://example.com/rc')).rejects.toThrow(/1102/);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});

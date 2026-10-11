import { afterEach, describe, expect, it, vi } from 'vitest';
import { getConfig, importTaskrcUrl } from './api';

const limit = () => new Response('error code: 1102', { status: 500 });
const ok = (body: unknown) => new Response(JSON.stringify(body), { status: 200 });

afterEach(() => vi.unstubAllGlobals());

describe("a request cut off by Cloudflare's resource limit (1102)", () => {
  it('is tried once more when it is a read', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(limit())
      .mockResolvedValueOnce(ok({ config: {} }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(getConfig()).resolves.toBeTruthy();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('gives up after one more try, saying what happened', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => limit());
    vi.stubGlobal('fetch', fetchMock);
    await expect(getConfig()).rejects.toThrow(/out of CPU time/);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('does not touch our own errors', async () => {
    const fetchMock = vi
      .fn()
      .mockImplementation(async () => new Response(JSON.stringify({ error: 'bad request' }), { status: 400 }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(getConfig()).rejects.toThrow('bad request');
    expect(fetchMock).toHaveBeenCalledTimes(1);
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

import { describe, expect, it, vi } from 'vitest';
import { installNativeApi, NativePort, watchLaunchTasks, type TcEnginePlugin } from './native';
import type { Call, EngineReply } from './protocol';

function fakePlugin(): TcEnginePlugin & { calls: unknown[][] } {
  const calls: unknown[][] = [];
  const log =
    <T>(name: string, result: T) =>
    (...args: unknown[]) => {
      calls.push([name, ...args]);
      return Promise.resolve(result);
    };
  return {
    calls,
    start: log('start', { restored: true }),
    send: vi.fn(async (o: { op: string; arg?: string }) => {
      calls.push(['send', o]);
      return {
        value: o.op === 'sync' ? '{"changed":true,"settings":false}' : `{"echo":${JSON.stringify(o.arg ?? null)}}`,
      };
    }) as TcEnginePlugin['send'],
    api: vi.fn(async (o: { method: string; path: string; body: string }) => {
      calls.push(['api', o]);
      return { status: 200, type: 'application/json; charset=utf-8', body: '{"ok":true}' };
    }) as TcEnginePlugin['api'],
    forget: log('forget', undefined),
    getConnection: vi.fn(async () => ({ configured: false, localOnly: false })),
    useLocally: log('useLocally', undefined),
    getLaunchTask: vi.fn(async () => ({ uuid: null as string | null })),
    addListener: vi.fn(async () => ({ remove: async () => {} })),
    getSyncOptions: vi.fn(async () => ({ settings: false, hooks: false })),
    setSyncOptions: log('setSyncOptions', undefined),
    setConnection: log('setConnection', undefined),
    clearConnection: log('clearConnection', undefined),
  };
}

/** Send messages to a port and collect what it answers, in order. */
async function ask(port: NativePort, messages: Call[]): Promise<EngineReply[]> {
  const replies: EngineReply[] = [];
  port.onmessage = (e) => replies.push(e.data);
  for (const m of messages) port.postMessage(m);
  await vi.waitFor(() => expect(replies.length).toBe(messages.length));
  return replies;
}

describe('NativePort', () => {
  it('answers the engine messages from the app, in order', async () => {
    const api = fakePlugin();
    const replies = await ask(new NativePort(api), [
      { id: 1, op: 'start', config: '{}', persist: true },
      { id: 2, op: 'config', config: '{"a":1}' },
      { id: 3, op: 'hooks', defined: '["on_add"]' },
      { id: 4, op: 'run', request: '{"line":"list"}' },
      { id: 5, op: 'import', text: '[]', apply: false, tz: 3600, now: 5 },
      { id: 6, op: 'sync' },
      { id: 7, op: 'forget' },
    ]);
    expect(replies.map((r) => r.id)).toEqual([1, 2, 3, 4, 5, 6, 7]);
    expect(replies[0]).toMatchObject({ ok: true, value: true });
    expect(replies[3]).toMatchObject({ ok: true, value: '{"echo":"{\\"line\\":\\"list\\"}"}' });
    expect(replies[5]).toMatchObject({ ok: true, value: true });
    expect(api.calls.map((c) => c[0])).toEqual(['start', 'send', 'send', 'send', 'send', 'forget']);
    // The hooks message is not sent anywhere: the phone reads its own script.
    expect(api.calls[2]).toEqual(['send', { op: 'run', arg: '{"line":"list"}' }]);
    const imported = (api.calls[3][1] as { arg: string }).arg;
    expect(JSON.parse(imported)).toEqual({ text: '[]', apply: false, tz: 3600, now: 5 });
  });

  it('reports why a call failed, and goes on with the next', async () => {
    const api = fakePlugin();
    api.send = vi
      .fn()
      .mockRejectedValueOnce(new Error('could not reach the bucket'))
      .mockResolvedValueOnce({ value: '{"changed":false,"settings":false}' });
    const replies = await ask(new NativePort(api), [
      { id: 1, op: 'sync' },
      { id: 2, op: 'sync' },
    ]);
    expect(replies[0]).toEqual({ id: 1, ok: false, error: 'could not reach the bucket' });
    expect(replies[1]).toMatchObject({ id: 2, ok: true, value: false });
  });
});

describe('NativePort syncing', () => {
  it('tells the page when the bucket replaced the settings or hooks, and answers whether tasks changed', async () => {
    const api = fakePlugin();
    api.send = vi
      .fn()
      .mockResolvedValueOnce({ value: '{"changed":false,"settings":true}' })
      .mockResolvedValueOnce({ value: '{"changed":true,"settings":false}' });
    const port = new NativePort(api);
    const told = vi.fn();
    port.onsettings = told;
    const replies = await ask(port, [
      { id: 1, op: 'sync' },
      { id: 2, op: 'sync' },
    ]);
    expect(replies[0]).toMatchObject({ ok: true, value: false });
    expect(replies[1]).toMatchObject({ ok: true, value: true });
    expect(told).toHaveBeenCalledTimes(1);
  });
});

describe('watchLaunchTasks', () => {
  it('opens the task the app was started for, and each one tapped later, and stops when told to', async () => {
    const api = fakePlugin();
    api.getLaunchTask = vi
      .fn()
      .mockResolvedValueOnce({ uuid: 'aaaa' })
      .mockResolvedValueOnce({ uuid: 'bbbb' })
      .mockResolvedValue({ uuid: null });
    const remove = vi.fn(async () => {});
    let tapped: () => void = () => {};
    api.addListener = vi.fn(async (_event: 'openTask', fn: () => void) => {
      tapped = fn;
      return { remove };
    });
    const open = vi.fn();
    const stop = await watchLaunchTasks(open, api);
    expect(open).toHaveBeenCalledWith('aaaa');
    tapped();
    await vi.waitFor(() => expect(open).toHaveBeenCalledWith('bbbb'));
    tapped();
    await new Promise((r) => setTimeout(r, 0));
    expect(open).toHaveBeenCalledTimes(2);
    stop();
    expect(remove).toHaveBeenCalled();
  });
});

describe('installNativeApi', () => {
  function install(api: TcEnginePlugin) {
    const real = vi.fn(async () => new Response('from the network'));
    const win = {
      fetch: real as unknown as typeof fetch,
      location: new URL('https://localhost/') as unknown as Location,
    };
    installNativeApi(api, win);
    return { fetch: win.fetch, real };
  }

  it('answers /api/setup by whether the phone is connected, or was told to go without a bucket', async () => {
    const api = fakePlugin();
    const { fetch } = install(api);
    expect(await (await fetch('/api/setup')).json()).toEqual({ configured: false, missing: [], host: '' });
    api.getConnection = vi.fn(async () => ({ configured: true, localOnly: false }));
    expect((await (await fetch('/api/setup')).json()).configured).toBe(true);
    api.getConnection = vi.fn(async () => ({ configured: false, localOnly: true }));
    expect((await (await fetch('/api/setup')).json()).configured).toBe(true);
  });

  it('sends the other /api routes to the app, with the method, the query and the body', async () => {
    const api = fakePlugin();
    const { fetch, real } = install(api);
    const res = await fetch('/api/config/taskrc?x=1', { method: 'put', body: 'uda.a.type=string\n' });
    expect(res.status).toBe(200);
    expect(res.headers.get('content-type')).toContain('application/json');
    expect(await res.json()).toEqual({ ok: true });
    expect(api.calls.at(-1)).toEqual([
      'api',
      { method: 'PUT', path: '/api/config/taskrc?x=1', body: 'uda.a.type=string\n' },
    ]);
    expect(real).not.toHaveBeenCalled();
  });

  it('leaves every other address to the network', async () => {
    const { fetch, real } = install(fakePlugin());
    expect(await (await fetch('https://cdn.jsdelivr.net/npm/lucide/icons/a.svg')).text()).toBe('from the network');
    await fetch('/assets/app.js');
    await fetch('https://elsewhere.example/api/config');
    expect(real).toHaveBeenCalledTimes(3);
  });
});

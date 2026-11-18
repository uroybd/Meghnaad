// @vitest-environment node
// The static flavour's session (specs 0013, 0017, 0018, 0019): how a tab opens, locks, is trusted and is unlocked with a
// passkey. The engine and the passkey's browser calls are stubbed; the vault is the real one, over IndexedDB in memory.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { memoryDb } from './memoryDb.testing';

const engine = vi.hoisted(() => ({ unlockWith: vi.fn(), stop: vi.fn(), api: vi.fn(), start: vi.fn() }));
vi.mock('./client.svelte', () => ({ engine }));
const passkey = vi.hoisted(() => ({
  createPasskey: vi.fn(),
  usePasskey: vi.fn(),
  signedHere: vi.fn(),
  prfReported: vi.fn(),
}));
vi.mock('./passkey', async (original) => ({ ...(await original<typeof import('./passkey')>()), ...passkey }));
vi.mock('tc-engine', () => ({ Engine: {} }));

type Session = typeof import('./session');
type Vault = typeof import('./vault');

const access = { api: '', repo: 'alice/tasks', branch: '', token: 'ghp_x', salt: 'c2FsdHNhbHRzYWx0c2FsdA==' };
const SECRET = 'correct horse';

let mem: ReturnType<typeof memoryDb>;
let storage: Map<string, string>;
let reload: ReturnType<typeof vi.fn>;

/** A fresh page: the modules as a reload makes them, over the same browser storage. */
async function page(): Promise<{ session: Session; vault: Vault }> {
  vi.resetModules();
  reload = vi.fn();
  vi.stubGlobal('location', { reload, origin: 'https://alice.github.io', hostname: 'alice.github.io' });
  return { session: await import('./session'), vault: await import('./vault') };
}

beforeEach(async () => {
  mem = memoryDb();
  storage = new Map();
  vi.stubGlobal('indexedDB', mem.indexedDB);
  vi.stubGlobal('BroadcastChannel', undefined);
  vi.stubGlobal('addEventListener', vi.fn());
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => storage.get(k) ?? null,
    setItem: (k: string, v: string) => void storage.set(k, v),
    removeItem: (k: string) => void storage.delete(k),
  });
  for (const f of [...Object.values(engine), ...Object.values(passkey)]) f.mockReset();
  const { vault } = await page();
  await vault.keepAccess(access, SECRET);
});

/** Let the checks the clock started finish: they ask IndexedDB, which answers on real timers. */
const settle = () => new Promise((r) => setTimeout(r, 30));

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('opening a tab', () => {
  it('connects a device that never was', async () => {
    await mem.indexedDB.deleteDatabase();
    await new Promise((r) => setTimeout(r, 0));
    const { session } = await page();
    expect(await session.openDevice()).toBe('connect');
  });

  it('asks for the secret on a device that is not trusted', async () => {
    const { session } = await page();
    expect(await session.openDevice()).toBe('unlock');
    expect(engine.unlockWith).not.toHaveBeenCalled();
  });

  it('opens a trusted device by itself, with its key', async () => {
    let { session } = await page();
    await session.unlockWithSecret(SECRET, true);
    const key = engine.unlockWith.mock.calls[0][0].key as Uint8Array;
    engine.unlockWith.mockReset();
    ({ session } = await page());
    expect(await session.openDevice()).toBe('unlocked');
    expect(engine.unlockWith).toHaveBeenCalledWith({ ...access, key });
  });
});

describe('locking', () => {
  it('runs the hooks and reloads; a trusted device then stays locked, a reload too, until it is unlocked', async () => {
    let { session } = await page();
    await session.unlockWithSecret(SECRET, true);
    const hook = vi.fn();
    session.onLock(hook);
    session.lock();
    expect(hook).toHaveBeenCalledWith(false);
    expect(engine.stop).toHaveBeenCalled();
    expect(reload).toHaveBeenCalledTimes(1);
    for (let i = 0; i < 2; i++) {
      ({ session } = await page());
      expect(await session.openDevice()).toBe('unlock');
    }
    await session.unlockWithSecret(SECRET);
    ({ session } = await page());
    expect(await session.openDevice()).toBe('unlocked');
  });

  it('tells the hooks when the device is being forgotten, and forgets the trust with it', async () => {
    const { session, vault } = await page();
    await session.unlockWithSecret(SECRET, true);
    const hook = vi.fn();
    session.onLock(hook);
    await session.forget();
    expect(hook).toHaveBeenCalledWith(true);
    expect(await vault.deviceTrusted()).toBe(false);
    expect(await (await page()).session.openDevice()).toBe('connect');
  });

  it('locks after the idle minutes, but not while the device is trusted', async () => {
    // Only the idle check's clock: the IndexedDB in memory answers on real timers.
    vi.useFakeTimers({ toFake: ['setInterval', 'Date'] });
    let { session } = await page();
    await session.unlockWithSecret(SECRET);
    await vi.advanceTimersByTimeAsync(31 * 60_000);
    await settle();
    expect(reload).toHaveBeenCalledTimes(1);

    ({ session } = await page());
    await session.unlockWithSecret(SECRET, true);
    await vi.advanceTimersByTimeAsync(31 * 60_000);
    await settle();
    expect(reload).not.toHaveBeenCalled();
  });
});

describe('trusting', () => {
  it('turned off, removes the copy and a passkey checked by signature, but not a PRF one', async () => {
    const { session, vault } = await page();
    await session.unlockWithSecret(SECRET, true);
    await vault.keepSignaturePasskey(new Uint8Array([1]), new Uint8Array([2]), -7);
    await session.setTrusted(false);
    expect(await session.isTrusted()).toBe(false);
    expect(await session.passkeyKind()).toBeUndefined();

    await session.setTrusted(true);
    await vault.keepPrfPasskey(
      { access, key: new Uint8Array(32) },
      new Uint8Array([1]),
      new Uint8Array(32),
      new Uint8Array(32),
    );
    await session.setTrusted(false);
    expect(await session.passkeyKind()).toBe('prf');
  });
});

describe('a passkey', () => {
  const secret = new Uint8Array(32).fill(7);

  it('set up on a device that is not trusted seals with its PRF secret, and unlocks with it', async () => {
    let { session } = await page();
    await session.unlockWithSecret(SECRET);
    const key = engine.unlockWith.mock.calls[0][0].key as Uint8Array;
    passkey.createPasskey.mockResolvedValue({ id: new Uint8Array([3]), prf: { salt: new Uint8Array(32), secret } });
    await session.setUpPasskey();
    expect(passkey.createPasskey).toHaveBeenCalledWith('prf');

    ({ session } = await page());
    engine.unlockWith.mockReset();
    passkey.usePasskey.mockResolvedValue({ prf: secret });
    await session.unlockWithPasskey();
    expect(passkey.usePasskey).toHaveBeenCalledWith(new Uint8Array([3]), new Uint8Array(32));
    expect(engine.unlockWith).toHaveBeenCalledWith({ ...access, key });
  });

  it('that gives another secret unlocks nothing, and is removed', async () => {
    let { session } = await page();
    await session.unlockWithSecret(SECRET);
    passkey.createPasskey.mockResolvedValue({ id: new Uint8Array([3]), prf: { salt: new Uint8Array(32), secret } });
    await session.setUpPasskey();
    ({ session } = await page());
    engine.unlockWith.mockReset();
    passkey.usePasskey.mockResolvedValue({ prf: new Uint8Array(32) });
    await expect(session.unlockWithPasskey()).rejects.toThrow('no longer opens');
    expect(engine.unlockWith).not.toHaveBeenCalled();
    expect(await session.passkeyKind()).toBeUndefined();
  });

  it('on a trusted device is checked by its signature, then opens the trusted copy', async () => {
    let { session } = await page();
    await session.unlockWithSecret(SECRET, true);
    passkey.createPasskey.mockResolvedValue({ id: new Uint8Array([4]), publicKey: new Uint8Array([5]), alg: -7 });
    await session.setUpPasskey();
    expect(passkey.createPasskey).toHaveBeenCalledWith('signature');
    session.lock();

    ({ session } = await page());
    engine.unlockWith.mockReset();
    expect(await session.openDevice()).toBe('unlock');
    passkey.usePasskey.mockResolvedValue({});
    passkey.signedHere.mockResolvedValue(false);
    await expect(session.unlockWithPasskey()).rejects.toThrow("didn't check out");
    expect(engine.unlockWith).not.toHaveBeenCalled();

    passkey.signedHere.mockResolvedValue(true);
    await session.unlockWithPasskey();
    expect(passkey.signedHere).toHaveBeenLastCalledWith(
      {},
      { kind: 'signature', id: new Uint8Array([4]), publicKey: new Uint8Array([5]), alg: -7 },
      { origin: 'https://alice.github.io', rpId: 'alice.github.io' },
    );
    expect(engine.unlockWith).toHaveBeenCalledTimes(1);
  });

  it('is offered on a device that is not trusted only where the browser does not say it has no PRF', async () => {
    const { session } = await page();
    expect(await session.passkeyPossible()).toBe(false);
    vi.stubGlobal('PublicKeyCredential', class {});
    vi.stubGlobal('navigator', { credentials: {} });
    passkey.prfReported.mockResolvedValue(false);
    expect(await session.passkeyPossible()).toBe(false);
    passkey.prfReported.mockResolvedValue(undefined);
    expect(await session.passkeyPossible()).toBe(true);
    await session.unlockWithSecret(SECRET, true);
    passkey.prfReported.mockResolvedValue(false);
    expect(await session.passkeyPossible()).toBe(true);
  });
});

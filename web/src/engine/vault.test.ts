// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { memoryDb } from './memoryDb.testing';
import {
  deviceTrusted,
  distrust,
  forgetDevice,
  keepPrfPasskey,
  keepSignaturePasskey,
  keptPasskey,
  openWithPrf,
  trustDevice,
  trustedCopy,
  type Unlocked,
} from './vault';

const text = (b: Uint8Array) => new TextDecoder().decode(b);
const TOKEN = 'ghp_thetoken';
const unlocked: Unlocked = {
  access: { api: '', repo: 'alice/tasks', branch: 'taskchampion', token: TOKEN, salt: 'c2FsdA==' },
  key: crypto.getRandomValues(new Uint8Array(32)),
};

let mem: ReturnType<typeof memoryDb>;
beforeEach(() => {
  mem = memoryDb();
  vi.stubGlobal('indexedDB', mem.indexedDB);
});
afterEach(() => vi.unstubAllGlobals());

describe('a trusted device (spec 0018)', () => {
  it('keeps the access and the key sealed with a key that cannot be read out, and opens them again', async () => {
    expect(await deviceTrusted()).toBe(false);
    await trustDevice(unlocked);
    expect(await deviceTrusted()).toBe(true);
    const kept = mem.data.get('trusted') as { key: CryptoKey; sealed: Uint8Array };
    expect(kept.key.extractable).toBe(false);
    expect(text(kept.sealed)).not.toContain(TOKEN);
    expect(text(kept.sealed)).not.toContain('alice/tasks');
    expect(await trustedCopy()).toEqual(unlocked);
  });

  it('is no longer trusted once that is turned off, or the device forgotten', async () => {
    await trustDevice(unlocked);
    await distrust();
    expect(await trustedCopy()).toBeUndefined();
    await trustDevice(unlocked);
    await forgetDevice();
    expect(await deviceTrusted()).toBe(false);
  });

  it('removes a copy that no longer opens', async () => {
    await trustDevice(unlocked);
    const kept = mem.data.get('trusted') as { sealed: Uint8Array };
    kept.sealed[kept.sealed.length - 1] ^= 1;
    expect(await trustedCopy()).toBeUndefined();
    expect(mem.data.has('trusted')).toBe(false);
  });
});

describe('a passkey that unlocks this device (spec 0019)', () => {
  const id = new Uint8Array([1, 2, 3]);
  const salt = crypto.getRandomValues(new Uint8Array(32));
  const secret = crypto.getRandomValues(new Uint8Array(32));

  it('seals with its PRF secret, keeping neither the secret nor the token', async () => {
    await keepPrfPasskey(unlocked, id, salt, secret);
    const kept = await keptPasskey();
    expect(kept?.kind).toBe('prf');
    const stored = mem.data.get('passkey') as Record<string, unknown>;
    expect(Object.keys(stored).sort()).toEqual(['id', 'kind', 'salt', 'sealed']);
    const sealed = stored.sealed as Uint8Array;
    expect(text(sealed)).not.toContain(TOKEN);
    expect(Buffer.from(sealed).includes(Buffer.from(secret))).toBe(false);
    if (kept?.kind !== 'prf') throw new Error('not a PRF passkey');
    expect(await openWithPrf(kept, secret)).toEqual(unlocked);
    await expect(openWithPrf(kept, crypto.getRandomValues(new Uint8Array(32)))).rejects.toThrow();
  });

  it('on a trusted device keeps only the public key to check', async () => {
    const publicKey = new Uint8Array([9, 9]);
    await keepSignaturePasskey(id, publicKey, -7);
    expect(await keptPasskey()).toEqual({ kind: 'signature', id, publicKey, alg: -7 });
  });

  it('reads nothing it does not recognise as a passkey', async () => {
    mem.data.set('passkey', { kind: 'prf', id });
    expect(await keptPasskey()).toBeUndefined();
  });
});

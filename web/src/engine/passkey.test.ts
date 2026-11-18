// @vitest-environment node
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createPasskey, derToRaw, NoPrf, PasskeyCancelled, signedHere, usePasskey, type Used } from './passkey';

const ORIGIN = 'https://alice.github.io';
const RP = 'alice.github.io';
const enc = (s: string) => new TextEncoder().encode(s);
const b64url = (b: Uint8Array) => Buffer.from(b).toString('base64url');

/** r and s (32 bytes each) as the DER WebAuthn gives an ECDSA signature in. */
function rawToDer(raw: Uint8Array): Uint8Array {
  const int = (b: Uint8Array) => {
    let i = 0;
    while (i < b.length - 1 && b[i] === 0) i++;
    const v = b.subarray(i);
    return v[0] & 0x80 ? [0x02, v.length + 1, 0, ...v] : [0x02, v.length, ...v];
  };
  const body = [...int(raw.subarray(0, 32)), ...int(raw.subarray(32))];
  return new Uint8Array([0x30, body.length, ...body]);
}

async function authData(rpId: string, flags: number): Promise<Uint8Array> {
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', enc(rpId)));
  return new Uint8Array([...hash, flags, 0, 0, 0, 1]);
}

/** What a passkey would give, signed with `key`, with any part changed by `change`. */
async function assertion(
  sign: (data: Uint8Array) => Promise<Uint8Array>,
  change: { origin?: string; rpId?: string; flags?: number; type?: string; otherChallenge?: boolean } = {},
): Promise<Used> {
  const challenge = crypto.getRandomValues(new Uint8Array(32));
  const told = change.otherChallenge ? crypto.getRandomValues(new Uint8Array(32)) : challenge;
  const clientDataJSON = enc(
    JSON.stringify({ type: change.type ?? 'webauthn.get', challenge: b64url(told), origin: change.origin ?? ORIGIN }),
  );
  const authenticatorData = await authData(change.rpId ?? RP, change.flags ?? 0x05);
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', clientDataJSON));
  const signature = await sign(new Uint8Array([...authenticatorData, ...hash]));
  return { challenge, authenticatorData, clientDataJSON, signature };
}

async function es256() {
  const pair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
  const publicKey = new Uint8Array(await crypto.subtle.exportKey('spki', pair.publicKey));
  const sign = async (data: Uint8Array) =>
    rawToDer(
      new Uint8Array(
        await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, pair.privateKey, data as BufferSource),
      ),
    );
  return { kept: { publicKey, alg: -7 }, sign };
}

describe("a trusted device's passkey, checked by its signature", () => {
  const here = { origin: ORIGIN, rpId: RP };

  it('passes when the kept key signed this challenge, here, with the person verified', async () => {
    const { kept, sign } = await es256();
    expect(await signedHere(await assertion(sign), kept, here)).toBe(true);
  });

  it('fails for another origin, site, challenge, kind of request, or without the person verified', async () => {
    const { kept, sign } = await es256();
    for (const change of [
      { origin: 'https://evil.example' },
      { rpId: 'evil.example' },
      { otherChallenge: true },
      { type: 'webauthn.create' },
      { flags: 0x01 },
    ])
      expect(await signedHere(await assertion(sign, change), kept, here), JSON.stringify(change)).toBe(false);
  });

  it("fails for another passkey's signature, or a changed one", async () => {
    const { kept } = await es256();
    const other = await es256();
    expect(await signedHere(await assertion(other.sign), kept, here)).toBe(false);
    const used = await assertion((await es256()).sign);
    used.signature[used.signature.length - 1] ^= 1;
    expect(await signedHere(used, kept, here)).toBe(false);
  });

  it('checks an RSA passkey too', async () => {
    const pair = await crypto.subtle.generateKey(
      { name: 'RSASSA-PKCS1-v1_5', modulusLength: 2048, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' },
      true,
      ['sign', 'verify'],
    );
    const publicKey = new Uint8Array(await crypto.subtle.exportKey('spki', pair.publicKey));
    const sign = async (data: Uint8Array) =>
      new Uint8Array(await crypto.subtle.sign('RSASSA-PKCS1-v1_5', pair.privateKey, data as BufferSource));
    expect(await signedHere(await assertion(sign), { publicKey, alg: -257 }, here)).toBe(true);
  });

  it('reads a DER signature with and without leading zeros', () => {
    const raw = new Uint8Array(64).map((_, i) => (i === 0 || i === 32 ? 0x80 : i));
    expect(derToRaw(rawToDer(raw))).toEqual(raw);
    const small = new Uint8Array(64).map((_, i) => (i < 3 ? 0 : i));
    expect(derToRaw(rawToDer(small))).toEqual(small);
  });
});

describe('making and using a passkey', () => {
  afterEach(() => vi.unstubAllGlobals());

  function browser(create: unknown, get?: unknown) {
    const credentials = { create: vi.fn(create as () => unknown), get: vi.fn(get as () => unknown) };
    vi.stubGlobal('navigator', { credentials });
    return credentials;
  }

  const credential = (ext: unknown, response: unknown = {}) => ({
    rawId: new Uint8Array([7, 7]).buffer,
    response,
    getClientExtensionResults: () => ext,
  });

  it('takes the PRF secret it gives when it is made', async () => {
    const secret = new Uint8Array([5, 5, 5]);
    const b = browser(() => Promise.resolve(credential({ prf: { enabled: true, results: { first: secret.buffer } } })));
    const made = await createPasskey('prf');
    expect(made.id).toEqual(new Uint8Array([7, 7]));
    expect(made.prf?.secret).toEqual(secret);
    expect(made.prf?.salt.length).toBe(32);
    expect(b.get).not.toHaveBeenCalled();
  });

  it('uses it once when the secret comes only on use', async () => {
    const secret = new Uint8Array([6]);
    const response = {
      authenticatorData: new ArrayBuffer(0),
      clientDataJSON: new ArrayBuffer(0),
      signature: new ArrayBuffer(0),
    };
    const b = browser(
      () => Promise.resolve(credential({ prf: { enabled: true } })),
      () => Promise.resolve(credential({ prf: { results: { first: secret.buffer } } }, response)),
    );
    const made = await createPasskey('prf');
    expect(made.prf?.secret).toEqual(secret);
    expect(b.get).toHaveBeenCalledTimes(1);
  });

  it('says so when there is no PRF', async () => {
    browser(() => Promise.resolve(credential({})));
    await expect(createPasskey('prf')).rejects.toBeInstanceOf(NoPrf);
  });

  it('keeps the public key of a passkey checked by signature', async () => {
    const response = { getPublicKey: () => new Uint8Array([1, 2]).buffer, getPublicKeyAlgorithm: () => -7 };
    browser(() => Promise.resolve(credential({}, response)));
    const made = await createPasskey('signature');
    expect(made).toEqual({ id: new Uint8Array([7, 7]), publicKey: new Uint8Array([1, 2]), alg: -7 });
  });

  it('tells a cancelled passkey from a failed one', async () => {
    browser(() => Promise.reject(new DOMException('cancelled', 'NotAllowedError')));
    await expect(createPasskey('prf')).rejects.toBeInstanceOf(PasskeyCancelled);
    browser(undefined, () => Promise.reject(new DOMException('cancelled', 'NotAllowedError')));
    await expect(usePasskey(new Uint8Array([1]))).rejects.toBeInstanceOf(PasskeyCancelled);
    browser(() => Promise.reject(new DOMException('bad', 'SecurityError')));
    await expect(createPasskey('prf')).rejects.toThrow('bad');
  });
});

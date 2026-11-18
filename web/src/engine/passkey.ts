// A passkey that unlocks this device (spec 0019), in the browser's own words (WebAuthn). There is no server: a passkey
// either gives a secret (the PRF extension) that seals what this device keeps, or, on a trusted device, signs a
// challenge this page checks with the public key it kept. vault.ts keeps what is kept; session.ts puts them together.

/** The person cancelled, or the browser gave up waiting (WebAuthn says both the same way). */
export class PasskeyCancelled extends Error {
  constructor() {
    super('The passkey was cancelled.');
  }
}

/** The passkey (or the browser, or the password manager) gave no PRF secret. */
export class NoPrf extends Error {
  constructor() {
    super(
      "This passkey or browser can't unlock the app here: trust this device to use it, or keep using the password.",
    );
  }
}

const ES256 = -7;
const RS256 = -257;

const random = (n: number) => crypto.getRandomValues(new Uint8Array(n));

interface PrfResults {
  prf?: { enabled?: boolean; results?: { first?: ArrayBuffer } };
}

/** Does this browser have passkeys at all? */
export function passkeysOffered(): boolean {
  return typeof PublicKeyCredential !== 'undefined' && typeof navigator !== 'undefined' && !!navigator.credentials;
}

/** What the browser says of PRF: true or false where it can say (`getClientCapabilities`), else undefined. */
export async function prfReported(): Promise<boolean | undefined> {
  const ask = (PublicKeyCredential as unknown as { getClientCapabilities?: () => Promise<Record<string, boolean>> })
    .getClientCapabilities;
  if (!ask) return undefined;
  const caps = await ask.call(PublicKeyCredential).catch(() => undefined);
  return caps && 'extension:prf' in caps ? caps['extension:prf'] : undefined;
}

async function ask<T>(call: () => Promise<T>): Promise<T> {
  try {
    return await call();
  } catch (e) {
    if (e instanceof DOMException && (e.name === 'NotAllowedError' || e.name === 'AbortError'))
      throw new PasskeyCancelled();
    throw e;
  }
}

export interface Created {
  id: Uint8Array;
  /** A PRF passkey: the salt asked with, and the secret it gave. */
  prf?: { salt: Uint8Array; secret: Uint8Array };
  /** A signature passkey: its public key (SPKI) and algorithm. */
  publicKey?: Uint8Array;
  alg?: number;
}

/** Make a passkey for this site: one that gives a PRF secret (`prf`), or one whose signature is checked. */
export async function createPasskey(kind: 'prf' | 'signature'): Promise<Created> {
  const salt = random(32);
  const cred = (await ask(() =>
    navigator.credentials.create({
      publicKey: {
        rp: { name: 'Meghnaad' },
        user: { id: random(16), name: 'Meghnaad on this device', displayName: 'Meghnaad' },
        challenge: random(32),
        pubKeyCredParams: [
          { type: 'public-key', alg: ES256 },
          { type: 'public-key', alg: RS256 },
        ],
        authenticatorSelection: { residentKey: 'preferred', userVerification: 'required' },
        ...(kind === 'prf'
          ? { extensions: { prf: { eval: { first: salt } } } as AuthenticationExtensionsClientInputs }
          : {}),
      },
    }),
  )) as PublicKeyCredential | null;
  if (!cred) throw new PasskeyCancelled();
  const id = new Uint8Array(cred.rawId);
  if (kind === 'signature') {
    const response = cred.response as AuthenticatorAttestationResponse;
    const publicKey = response.getPublicKey?.();
    const alg = response.getPublicKeyAlgorithm?.();
    if (!publicKey || (alg !== ES256 && alg !== RS256))
      throw new Error("This passkey's kind of key can't be checked here.");
    return { id, publicKey: new Uint8Array(publicKey), alg };
  }
  const ext = cred.getClientExtensionResults() as PrfResults;
  const first = ext.prf?.results?.first;
  if (first) return { id, prf: { salt, secret: new Uint8Array(first) } };
  // Some give the secret only when the passkey is used, not when it is made: use it once now.
  if (ext.prf?.enabled) return { id, prf: { salt, secret: (await usePasskey(id, salt)).prf ?? fail() } };
  throw new NoPrf();
}

function fail(): never {
  throw new NoPrf();
}

export interface Used {
  prf?: Uint8Array;
  challenge: Uint8Array;
  authenticatorData: Uint8Array;
  clientDataJSON: Uint8Array;
  signature: Uint8Array;
}

/** Use the passkey `id`: its PRF secret for `salt` when asked, and its signature over a fresh challenge. */
export async function usePasskey(id: Uint8Array, salt?: Uint8Array): Promise<Used> {
  const challenge = random(32);
  const cred = (await ask(() =>
    navigator.credentials.get({
      publicKey: {
        challenge,
        allowCredentials: [{ type: 'public-key', id: id as Uint8Array<ArrayBuffer> }],
        userVerification: 'required',
        ...(salt ? { extensions: { prf: { eval: { first: salt } } } as AuthenticationExtensionsClientInputs } : {}),
      },
    }),
  )) as PublicKeyCredential | null;
  if (!cred) throw new PasskeyCancelled();
  const response = cred.response as AuthenticatorAssertionResponse;
  const first = (cred.getClientExtensionResults() as PrfResults).prf?.results?.first;
  return {
    prf: first ? new Uint8Array(first) : undefined,
    challenge,
    authenticatorData: new Uint8Array(response.authenticatorData),
    clientDataJSON: new Uint8Array(response.clientDataJSON),
    signature: new Uint8Array(response.signature),
  };
}

function base64url(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

/** An ECDSA signature as WebAuthn gives it (DER) in the form WebCrypto checks (r and s, 32 bytes each). */
export function derToRaw(der: Uint8Array): Uint8Array<ArrayBuffer> | undefined {
  if (der[0] !== 0x30) return undefined;
  let at = 2;
  const out = new Uint8Array(64);
  for (const half of [0, 32]) {
    if (der[at] !== 0x02) return undefined;
    const len = der[at + 1];
    let part = der.subarray(at + 2, at + 2 + len);
    while (part.length > 32 && part[0] === 0) part = part.subarray(1);
    if (part.length > 32) return undefined;
    out.set(part, half + 32 - part.length);
    at += 2 + len;
  }
  return out;
}

const USER_PRESENT = 0x01;
const USER_VERIFIED = 0x04;

/**
 * Did the passkey whose public key was kept sign this, now, here? The client data must be a `webauthn.get` for this
 * challenge on this origin; the authenticator data for this site, with the person present and verified; and the
 * signature over both good.
 */
export async function signedHere(
  used: Used,
  kept: { publicKey: Uint8Array; alg: number },
  where: { origin: string; rpId: string },
): Promise<boolean> {
  const client = JSON.parse(new TextDecoder().decode(used.clientDataJSON)) as {
    type?: string;
    challenge?: string;
    origin?: string;
  };
  if (
    client.type !== 'webauthn.get' ||
    client.challenge !== base64url(used.challenge) ||
    client.origin !== where.origin
  )
    return false;
  const data = used.authenticatorData;
  if (data.length < 37) return false;
  const rpHash = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(where.rpId)));
  if (!rpHash.every((b, i) => data[i] === b)) return false;
  if ((data[32] & (USER_PRESENT | USER_VERIFIED)) !== (USER_PRESENT | USER_VERIFIED)) return false;
  const clientHash = new Uint8Array(
    await crypto.subtle.digest('SHA-256', used.clientDataJSON as Uint8Array<ArrayBuffer>),
  );
  const signed = new Uint8Array(data.length + clientHash.length);
  signed.set(data);
  signed.set(clientHash, data.length);
  const spki = kept.publicKey as Uint8Array<ArrayBuffer>;
  try {
    if (kept.alg === ES256) {
      const key = await crypto.subtle.importKey('spki', spki, { name: 'ECDSA', namedCurve: 'P-256' }, false, [
        'verify',
      ]);
      const raw = derToRaw(used.signature);
      return !!raw && (await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, key, raw, signed));
    }
    if (kept.alg === RS256) {
      const key = await crypto.subtle.importKey('spki', spki, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, false, [
        'verify',
      ]);
      return await crypto.subtle.verify('RSASSA-PKCS1-v1_5', key, used.signature as Uint8Array<ArrayBuffer>, signed);
    }
  } catch {
    return false;
  }
  return false;
}

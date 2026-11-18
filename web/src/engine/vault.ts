// The static flavour's access to the git branch on this device: where the tasks are and the token for them, sealed
// with the encryption secret in OpenPGP's format (pgp.ts) and kept in IndexedDB beside the saved copy of the tasks. The
// encryption secret itself is never kept: it is typed to unlock, and turned into the key of the tasks (PBKDF2, 600,000
// rounds of SHA-256 with the branch's salt, as TaskChampion and `task` do), which lives only in this tab's memory.

import { open, seal } from './pgp';
import { openBytes, sealBytes, tx } from './persist';

const ACCESS = 'access';
const TRUSTED = 'trusted';
const PASSKEY = 'passkey';
const DB = 'meghnaad-engine';
const ROUNDS = 600_000;

/** Where the tasks are, the token, and the salt of the key (not secret: it is in the branch's `meta`). */
export interface Access {
  /** The forge's API (empty for GitHub's). */
  api: string;
  repo: string;
  branch: string;
  token: string;
  /** Base64. */
  salt: string;
}

/** An unlocked device: its access and the key of the tasks. */
export interface Unlocked {
  access: Access;
  key: Uint8Array;
}

export function toBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes));
}

export function fromBase64(text: string): Uint8Array {
  return Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
}

/** The key of the tasks from the encryption secret and the branch's salt: 32 bytes, as `tc_core::crypto` makes it. */
export async function deriveKey(secret: string, salt: Uint8Array): Promise<Uint8Array> {
  const base = await crypto.subtle.importKey('raw', new TextEncoder().encode(secret), 'PBKDF2', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits(
    { name: 'PBKDF2', hash: 'SHA-256', salt: salt as Uint8Array<ArrayBuffer>, iterations: ROUNDS },
    base,
    256,
  );
  return new Uint8Array(bits);
}

/** The sealed access kept on this device, if there is one. */
export async function sealedAccess(): Promise<Uint8Array | undefined> {
  const found = await tx<unknown>('readonly', (s) => s.get(ACCESS)).catch(() => undefined);
  return found instanceof Uint8Array ? found : undefined;
}

/** Keep `access` on this device, sealed with `secret`. */
export async function keepAccess(access: Access, secret: string): Promise<void> {
  const sealed = await seal(new TextEncoder().encode(JSON.stringify(access)), secret);
  await tx('readwrite', (s) => s.put(sealed, ACCESS));
}

/** Open the access kept here with `secret`, and make the key: `WrongPassword` if it is not the secret. */
export async function unlock(secret: string): Promise<Unlocked> {
  const sealed = await sealedAccess();
  if (!sealed) throw new Error('This device has not been connected yet.');
  const access = JSON.parse(new TextDecoder().decode(await open(sealed, secret))) as Access;
  return { access, key: await deriveKey(secret, fromBase64(access.salt)) };
}

/** Forget this device: the sealed access and the saved copy of the tasks (what was pushed stays on the branch). */
export function forgetDevice(): Promise<void> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.deleteDatabase(DB);
    req.onsuccess = () => resolve();
    req.onerror = () => reject(req.error ?? new Error('this browser would not forget the data'));
    // Another tab still has it open: it is deleted when that one closes, and it is locked by then.
    req.onblocked = () => resolve();
  });
}

// What an unlocked device is, as bytes to seal: the access and the key of the tasks.
function packUnlocked(u: Unlocked): Uint8Array {
  return new TextEncoder().encode(JSON.stringify({ access: u.access, key: toBase64(u.key) }));
}

function unpackUnlocked(bytes: Uint8Array): Unlocked {
  const kept = JSON.parse(new TextDecoder().decode(bytes)) as { access: Access; key: string };
  return { access: kept.access, key: fromBase64(kept.key) };
}

async function read(name: string): Promise<unknown> {
  return tx<unknown>('readonly', (s) => s.get(name)).catch(() => undefined);
}

/**
 * A trusted device (spec 0018) keeps itself unlocked: the access and the key, sealed with a device key made here that
 * page code can use but never read out. That keeps the raw key out of anything a script could copy away; it is not
 * protection at rest (a browser may keep the device key in its profile as it is), so trusting a device trusts its
 * browser profile.
 */
interface Trusted {
  key: CryptoKey;
  sealed: Uint8Array;
}

function isTrusted(found: unknown): found is Trusted {
  return (
    typeof found === 'object' &&
    found !== null &&
    (found as Trusted).key instanceof CryptoKey &&
    (found as Trusted).sealed instanceof Uint8Array
  );
}

export async function trustDevice(unlocked: Unlocked): Promise<void> {
  const key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
  const sealed = await sealBytes(packUnlocked(unlocked), key);
  await tx('readwrite', (s) => s.put({ key, sealed } satisfies Trusted, TRUSTED));
}

export async function deviceTrusted(): Promise<boolean> {
  return isTrusted(await read(TRUSTED));
}

/** The trusted copy, opened; none if the device is not trusted, or if it no longer opens (then it is removed). */
export async function trustedCopy(): Promise<Unlocked | undefined> {
  const found = await read(TRUSTED);
  if (!isTrusted(found)) return undefined;
  try {
    return unpackUnlocked(await openBytes(found.sealed, found.key));
  } catch {
    await distrust().catch(() => undefined);
    return undefined;
  }
}

export async function distrust(): Promise<void> {
  await tx('readwrite', (s) => s.delete(TRUSTED));
}

/**
 * A passkey that unlocks this device (spec 0019). `prf`: the access and the key sealed with a key only that passkey's
 * PRF secret makes (neither is kept). `signature`: on a trusted device, the passkey's public key, to check that it
 * signed; the trusted copy is what it opens.
 */
export type KeptPasskey =
  | { kind: 'prf'; id: Uint8Array; salt: Uint8Array; sealed: Uint8Array }
  | { kind: 'signature'; id: Uint8Array; publicKey: Uint8Array; alg: number };

export async function keptPasskey(): Promise<KeptPasskey | undefined> {
  const found = (await read(PASSKEY)) as Partial<KeptPasskey> | undefined;
  if (!found || !(found.id instanceof Uint8Array)) return undefined;
  if (found.kind === 'prf' && found.salt instanceof Uint8Array && found.sealed instanceof Uint8Array)
    return found as KeptPasskey;
  if (found.kind === 'signature' && found.publicKey instanceof Uint8Array && typeof found.alg === 'number')
    return found as KeptPasskey;
  return undefined;
}

/** The AES-GCM key a PRF secret makes: the secret itself is used only here. */
async function prfKey(prf: Uint8Array): Promise<CryptoKey> {
  const base = await crypto.subtle.importKey('raw', prf as Uint8Array<ArrayBuffer>, 'HKDF', false, ['deriveKey']);
  return crypto.subtle.deriveKey(
    { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(32), info: new TextEncoder().encode('meghnaad passkey') },
    base,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}

export async function keepPrfPasskey(unlocked: Unlocked, id: Uint8Array, salt: Uint8Array, prf: Uint8Array) {
  const sealed = await sealBytes(packUnlocked(unlocked), await prfKey(prf));
  await tx('readwrite', (s) => s.put({ kind: 'prf', id, salt, sealed } satisfies KeptPasskey, PASSKEY));
}

export async function keepSignaturePasskey(id: Uint8Array, publicKey: Uint8Array, alg: number) {
  await tx('readwrite', (s) => s.put({ kind: 'signature', id, publicKey, alg } satisfies KeptPasskey, PASSKEY));
}

/** Open what a PRF passkey sealed with the secret it gave; throws if it is not that passkey's. */
export async function openWithPrf(kept: KeptPasskey & { kind: 'prf' }, prf: Uint8Array): Promise<Unlocked> {
  return unpackUnlocked(await openBytes(kept.sealed, await prfKey(prf)));
}

export async function forgetPasskey(): Promise<void> {
  await tx('readwrite', (s) => s.delete(PASSKEY));
}

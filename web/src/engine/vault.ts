// The static flavour's access to the git branch on this device: where the tasks are and the token for them, sealed
// with the encryption secret in OpenPGP's format (pgp.ts) and kept in IndexedDB beside the saved copy of the tasks. The
// encryption secret itself is never kept: it is typed to unlock, and turned into the key of the tasks (PBKDF2, 600,000
// rounds of SHA-256 with the branch's salt, as TaskChampion and `task` do), which lives only in this tab's memory.

import { open, seal } from './pgp';
import { tx } from './persist';

const ACCESS = 'access';
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

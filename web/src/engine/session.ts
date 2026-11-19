// The static flavour's session on this device: connecting it to a branch, unlocking it, and locking it again. The key
// of the tasks and the token live only in the memory of unlocked tabs. Unlocking one tab hands them to the other open
// tabs of this site (a same-origin BroadcastChannel), and locking one locks them all: a locked tab is one reloaded, so
// nothing of them is left in it (the hooks given to `onLock` run first: the reminders a locked tab keeps, spec 0017).
// A trusted device (spec 0018) opens itself from a copy kept here; a passkey (spec 0019) unlocks in place of the secret.

import { Engine } from 'tc-engine';
import { engine } from './client.svelte';
import { createPasskey, NoPrf, prfReported, signedHere, usePasskey } from './passkey';
import { checkPin, clearPin, hasPin, markPinLocked, pinLocked, pinMinutes } from './pin';
import {
  deriveKey,
  deviceTrusted,
  distrust,
  forgetDevice,
  forgetPasskey,
  keepAccess,
  keepPrfPasskey,
  keepSignaturePasskey,
  keptPasskey,
  openWithPrf,
  sealedAccess,
  toBase64,
  trustDevice,
  trustedCopy,
  unlock,
  type Access,
  type Unlocked,
} from './vault';

const CHANNEL = 'meghnaad-vault';
const ACTIVE = 'meghnaad-last-active';
const IDLE = 'meghnaad-idle-minutes';
/** Set by a lock, cleared by an unlock: a trusted device locked with Lock now stays locked until it is unlocked. */
const LOCKED = 'meghnaad-locked';
export const DEFAULT_IDLE_MINUTES = 30;
/** How long a tab waits for another to hand it the key before it asks for the secret. */
const ASK_FOR_MS = 400;

type Message =
  | { kind: 'ask' }
  | { kind: 'give'; unlocked: Unlocked }
  | { kind: 'lock' }
  | { kind: 'forget' }
  | { kind: 'pin' }
  | { kind: 'unpin' };

let channel: BroadcastChannel | undefined;
let mine: Unlocked | undefined;

function open(): BroadcastChannel | undefined {
  if (typeof BroadcastChannel === 'undefined') return undefined;
  channel ??= new BroadcastChannel(CHANNEL);
  return channel;
}

/** What is typed on the Connect screen. */
export interface ConnectForm {
  api: string;
  repo: string;
  branch: string;
  token: string;
  secret: string;
}

function why(e: unknown): string {
  return (e instanceof Error ? e.message : String(e)).replace(/^Server Error: /, '');
}

/** Start the engine on the branch with the key: its first sync, with nothing saved here yet, also proves the key. */
async function start(unlocked: Unlocked): Promise<void> {
  const { access, key } = unlocked;
  engine.unlockWith({ api: access.api, repo: access.repo, branch: access.branch, token: access.token, key });
  const answer = await engine.api('GET', '/api/config', '');
  if (answer.status !== 200) {
    const body = JSON.parse(answer.body) as { error?: string };
    throw new Error(body.error ?? `the settings could not be read (${answer.status})`);
  }
  await engine.start(JSON.stringify((JSON.parse(answer.body) as { config: unknown }).config));
}

/** Connect this device: the branch must answer and the secret must open the tasks there. Then it is kept, sealed. */
export async function connect(form: ConnectForm, trust = false): Promise<void> {
  let salt: Uint8Array;
  try {
    salt = await Engine.git_salt(form.api.trim(), form.repo.trim(), form.branch.trim(), form.token.trim());
  } catch (e) {
    throw new Error(`The repository could not be reached: ${why(e)}`, { cause: e });
  }
  const access: Access = {
    api: form.api.trim(),
    repo: form.repo.trim(),
    branch: form.branch.trim(),
    token: form.token.trim(),
    salt: toBase64(salt),
  };
  const unlocked = { access, key: await deriveKey(form.secret, salt) };
  try {
    await start(unlocked);
  } catch (e) {
    engine.stop();
    throw new Error(
      `The tasks on the branch could not be opened: ${why(e)}. Is the encryption secret the one your task CLI uses?`,
      { cause: e },
    );
  }
  await keepAccess(access, form.secret);
  if (trust) await trustDevice(unlocked);
  became(unlocked);
}

/** Unlock with the secret: `WrongPassword` if it isn't the one this device was connected with. */
export async function unlockWithSecret(secret: string, trust = false): Promise<void> {
  const unlocked = await unlock(secret);
  if (trust) await trustDevice(unlocked);
  unlockedWith(unlocked);
}

function unlockedWith(unlocked: Unlocked): void {
  engine.unlockWith({ ...unlocked.access, key: unlocked.key });
  became(unlocked);
}

function mark(locked: boolean): void {
  try {
    if (locked) localStorage.setItem(LOCKED, '1');
    else localStorage.removeItem(LOCKED);
  } catch {
    // Without it a trusted device opens itself again after a reload.
  }
}

function markedLocked(): boolean {
  try {
    return localStorage.getItem(LOCKED) !== null;
  } catch {
    return false;
  }
}

/**
 * How this tab opens: `connect` (never connected), `unlocked` (another open tab handed it the key, or the device is
 * trusted and was not locked with Lock now), else `unlock`, the form.
 */
export async function openDevice(): Promise<'connect' | 'unlock' | 'unlocked'> {
  if (!(await sealedAccess())) return 'connect';
  if (await askOtherTabs()) {
    // A reload of a tab on a device with a PIN opens on the PIN screen; a new tab beside open ones does not.
    if (reloaded()) pinLock();
    return 'unlocked';
  }
  if (!markedLocked()) {
    const copy = await trustedCopy();
    if (copy) {
      unlockedWith(copy);
      // Opened from the trusted copy (a reload, or the first tab): with a PIN, behind the PIN screen.
      pinLock();
      return 'unlocked';
    }
  }
  // A locked tab follows another forgetting the device: what it kept for its reminders goes too.
  open()?.addEventListener('message', (e: MessageEvent<Message>) => {
    if (e.data.kind === 'forget' && !mine) {
      runHooks(true);
      location.reload();
    }
  });
  return 'unlock';
}

/** Is this device trusted (spec 0018)? */
export const isTrusted = deviceTrusted;

/** Trust this unlocked device, or stop: stopping removes the copy, and a passkey checked by signature with it. */
export async function setTrusted(on: boolean): Promise<void> {
  if (on) {
    if (!mine) throw new Error('This device is locked.');
    await trustDevice(mine);
    return;
  }
  await distrust();
  // A device that is not trusted has the full lock and nothing in between.
  clearPin();
  told(false);
  if ((await keptPasskey())?.kind === 'signature') await forgetPasskey();
}

/** Which passkey unlocks this device, if any. */
export async function passkeyKind(): Promise<'prf' | 'signature' | undefined> {
  return (await keptPasskey())?.kind;
}

/** Can a passkey be set up here? Any on a trusted device; on another, not where the browser says it has no PRF. */
export async function passkeyPossible(): Promise<boolean> {
  if (typeof PublicKeyCredential === 'undefined' || typeof navigator === 'undefined' || !navigator.credentials)
    return false;
  return (await deviceTrusted()) || (await prfReported()) !== false;
}

/** Set up a passkey for this unlocked device: checked by signature when it is trusted, else sealing with its PRF secret. */
export async function setUpPasskey(): Promise<void> {
  const unlocked = mine;
  if (!unlocked) throw new Error('This device is locked.');
  if (await deviceTrusted()) {
    const made = await createPasskey('signature');
    if (!made.publicKey || made.alg === undefined) throw new Error("This passkey's kind of key can't be checked here.");
    await keepSignaturePasskey(made.id, made.publicKey, made.alg);
    return;
  }
  const made = await createPasskey('prf');
  if (!made.prf) throw new NoPrf();
  await keepPrfPasskey(unlocked, made.id, made.prf.salt, made.prf.secret);
}

export const removePasskey = forgetPasskey;

/** Unlock with the passkey this device has. Throws `PasskeyCancelled` when it was cancelled, else why it failed. */
export async function unlockWithPasskey(): Promise<void> {
  const kept = await keptPasskey();
  if (!kept) throw new Error('No passkey unlocks this device.');
  if (kept.kind === 'prf') {
    const used = await usePasskey(kept.id, kept.salt);
    if (!used.prf) throw new NoPrf();
    let unlocked: Unlocked;
    try {
      unlocked = await openWithPrf(kept, used.prf);
    } catch {
      await forgetPasskey();
      throw new Error('That passkey no longer opens this device, so it was removed. Type the encryption secret.');
    }
    unlockedWith(unlocked);
    return;
  }
  const used = await usePasskey(kept.id);
  if (!(await signedHere(used, kept, { origin: location.origin, rpId: location.hostname })))
    throw new Error("That passkey's signature didn't check out.");
  const copy = await trustedCopy();
  if (!copy) {
    await forgetPasskey();
    throw new Error('This device is no longer trusted, so its passkey was removed. Type the encryption secret.');
  }
  unlockedWith(copy);
}

/** Another unlocked tab of this site, if one answers soon, hands this one the key; then this one is unlocked too. */
export function askOtherTabs(): Promise<boolean> {
  const ch = open();
  if (!ch) return Promise.resolve(false);
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      ch.removeEventListener('message', listen);
      resolve(false);
    }, ASK_FOR_MS);
    function listen(e: MessageEvent<Message>) {
      if (e.data.kind !== 'give') return;
      clearTimeout(timer);
      ch?.removeEventListener('message', listen);
      const unlocked = e.data.unlocked;
      engine.unlockWith({ ...unlocked.access, key: unlocked.key });
      became(unlocked);
      resolve(true);
    }
    ch.addEventListener('message', listen);
    ch.postMessage({ kind: 'ask' } satisfies Message);
  });
}

/** This tab is unlocked: it answers other tabs, follows a lock, and locks itself when the device has been idle. */
function became(unlocked: Unlocked): void {
  mine = unlocked;
  mark(false);
  const ch = open();
  ch?.addEventListener('message', (e: MessageEvent<Message>) => {
    if (e.data.kind === 'ask' && mine) ch.postMessage({ kind: 'give', unlocked: mine } satisfies Message);
    if (e.data.kind === 'lock') dropAndReload();
    if (e.data.kind === 'forget') dropAndReload(true);
    if (e.data.kind === 'pin') told(true);
    if (e.data.kind === 'unpin') told(false);
  });
  watchIdle();
}

// The PIN screen of a trusted device (spec 0022): up and down in every tab together. What is shown follows `onPin`.
/** Whether this page was opened by a reload. */
function reloaded(): boolean {
  try {
    const nav = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming | undefined;
    return nav?.type === 'reload';
  } catch {
    return false;
  }
}

const pinWatchers = new Set<(locked: boolean) => void>();

function told(locked: boolean): void {
  for (const fn of pinWatchers) fn(locked);
}

/** Follow the PIN screen: `fn` is called with whether it is up, now and on each change, until the returned stop. */
export function onPin(fn: (locked: boolean) => void): () => void {
  pinWatchers.add(fn);
  fn(pinLocked());
  return () => pinWatchers.delete(fn);
}

/** Put the PIN screen up in every tab, if this device has a PIN. */
export function pinLock(): void {
  if (!hasPin()) return;
  markPinLocked(true);
  open()?.postMessage({ kind: 'pin' } satisfies Message);
  told(true);
}

/** Take the PIN screen down in every tab, if `keys` are the PIN. */
export async function pinUnlock(keys: string[]): Promise<boolean> {
  if (!(await checkPin(keys))) return false;
  markPinLocked(false);
  open()?.postMessage({ kind: 'unpin' } satisfies Message);
  told(false);
  return true;
}

/**
 * `:lock`: with no word, the PIN screen on a trusted device that has a PIN, else the full lock. `pin` asks for the PIN
 * screen (an error where there is none) and `key` for the full lock, the encryption secret's.
 */
export async function lockNow(how: 'pin' | 'key' | null = null): Promise<void> {
  const pin = hasPin() && (await deviceTrusted());
  if (how === 'pin' && !pin)
    throw new Error('This device has no PIN: set one in Settings → General, on a trusted device.');
  if (how !== 'key' && pin) pinLock();
  else lock();
}

const lockHooks: ((forgetting: boolean) => void)[] = [];

/**
 * Run `fn` just before this tab locks and reloads, with nothing of the tasks left: `forgetting` when it is because the
 * device is being forgotten (in this tab or another).
 */
export function onLock(fn: (forgetting: boolean) => void): void {
  lockHooks.push(fn);
}

function runHooks(forgetting: boolean): void {
  for (const fn of lockHooks) {
    try {
      fn(forgetting);
    } catch {
      // A hook that fails must not keep the tab unlocked.
    }
  }
}

let dropping = false;

function dropAndReload(forgetting = false): void {
  // Once: the reload is not immediate, and a second message (forget, then lock) must not keep the list again.
  if (dropping) return;
  dropping = true;
  runHooks(forgetting);
  mine = undefined;
  engine.stop();
  location.reload();
}

/** Where this unlocked device syncs (never the token), for Settings. */
export function connectedTo(): { repo: string; branch: string; api: string } | undefined {
  return mine && { repo: mine.access.repo, branch: mine.access.branch, api: mine.access.api };
}

/** Lock every tab of this site: the key and the token are dropped; the saved copy stays, sealed. */
export function lock(): void {
  mark(true);
  // The full lock takes the place of the PIN screen: the secret or a passkey opens the device again.
  markPinLocked(false);
  open()?.postMessage({ kind: 'lock' } satisfies Message);
  dropAndReload();
}

/** Forget this device: lock, and remove the sealed access and the saved copy of the tasks. */
export async function forget(): Promise<void> {
  // 'forget' first: a tab of this version reloads on it; one of an older version knows only 'lock'.
  open()?.postMessage({ kind: 'forget' } satisfies Message);
  open()?.postMessage({ kind: 'lock' } satisfies Message);
  runHooks(true);
  mine = undefined;
  engine.stop();
  mark(false);
  clearPin();
  await forgetDevice();
  location.reload();
}

/** Minutes without use after which the tabs lock (0: never). Kept in this browser. */
export function idleMinutes(): number {
  try {
    const n = Number(localStorage.getItem(IDLE) ?? DEFAULT_IDLE_MINUTES);
    return Number.isFinite(n) && n >= 0 ? n : DEFAULT_IDLE_MINUTES;
  } catch {
    return DEFAULT_IDLE_MINUTES;
  }
}

export function setIdleMinutes(n: number): void {
  try {
    localStorage.setItem(IDLE, String(Math.max(0, Math.round(n))));
  } catch {
    // Not kept: the default holds next time.
  }
}

let watching = false;

/** The last use of any tab of this site is noted (often enough, not on every move), and checked each minute. */
function watchIdle(): void {
  if (watching) return;
  watching = true;
  let noted = 0;
  const note = () => {
    const now = Date.now();
    if (now - noted < 15_000) return;
    noted = now;
    try {
      localStorage.setItem(ACTIVE, String(now));
    } catch {
      // Without it each tab goes by its own use.
    }
  };
  note();
  for (const ev of ['pointerdown', 'keydown', 'wheel', 'touchstart']) {
    addEventListener(ev, note, { passive: true, capture: true });
  }
  setInterval(async () => {
    let last = noted;
    try {
      last = Math.max(last, Number(localStorage.getItem(ACTIVE) ?? 0));
    } catch {
      // This tab's own use.
    }
    const idle = Date.now() - last;
    // A trusted device never locks by itself; with a PIN, the PIN screen comes up instead (spec 0022).
    if (await deviceTrusted()) {
      const minutes = pinMinutes();
      if (minutes && hasPin() && !pinLocked() && idle > minutes * 60_000) pinLock();
      return;
    }
    const minutes = idleMinutes();
    if (minutes && idle > minutes * 60_000) lock();
  }, 60_000);
}

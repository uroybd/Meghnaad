// The static flavour's session on this device: connecting it to a branch, unlocking it, and locking it again. The key
// of the tasks and the token live only in the memory of unlocked tabs. Unlocking one tab hands them to the other open
// tabs of this site (a same-origin BroadcastChannel), and locking one locks them all: a locked tab is one reloaded, so
// nothing of them is left in it.

import { Engine } from 'tc-engine';
import { engine } from './client.svelte';
import { deriveKey, forgetDevice, keepAccess, toBase64, unlock, type Access, type Unlocked } from './vault';

const CHANNEL = 'meghnaad-vault';
const ACTIVE = 'meghnaad-last-active';
const IDLE = 'meghnaad-idle-minutes';
export const DEFAULT_IDLE_MINUTES = 30;
/** How long a tab waits for another to hand it the key before it asks for the secret. */
const ASK_FOR_MS = 400;

type Message = { kind: 'ask' } | { kind: 'give'; unlocked: Unlocked } | { kind: 'lock' };

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
export async function connect(form: ConnectForm): Promise<void> {
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
  became(unlocked);
}

/** Unlock with the secret: `WrongPassword` if it isn't the one this device was connected with. */
export async function unlockWithSecret(secret: string): Promise<void> {
  const unlocked = await unlock(secret);
  engine.unlockWith({ ...unlocked.access, key: unlocked.key });
  became(unlocked);
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
  const ch = open();
  ch?.addEventListener('message', (e: MessageEvent<Message>) => {
    if (e.data.kind === 'ask' && mine) ch.postMessage({ kind: 'give', unlocked: mine } satisfies Message);
    if (e.data.kind === 'lock') dropAndReload();
  });
  watchIdle();
}

function dropAndReload(): void {
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
  open()?.postMessage({ kind: 'lock' } satisfies Message);
  dropAndReload();
}

/** Forget this device: lock, and remove the sealed access and the saved copy of the tasks. */
export async function forget(): Promise<void> {
  open()?.postMessage({ kind: 'lock' } satisfies Message);
  mine = undefined;
  engine.stop();
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
  setInterval(() => {
    const minutes = idleMinutes();
    if (!minutes) return;
    let last = noted;
    try {
      last = Math.max(last, Number(localStorage.getItem(ACTIVE) ?? 0));
    } catch {
      // This tab's own use.
    }
    if (Date.now() - last > minutes * 60_000) lock();
  }, 60_000);
}

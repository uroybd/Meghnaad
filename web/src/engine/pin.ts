// The PIN of a trusted device (spec 0022): a loose, mostly visual block over the app after some idle minutes, gone again
// with four key presses. It seals nothing (the trusted copy opens without it); it keeps a passer-by's eyes off the tasks.
// All of it is this browser's alone, in localStorage: nothing here is synced.

const PIN = 'meghnaad-pin';
const MINUTES = 'meghnaad-pin-minutes';
const LOCKED = 'meghnaad-pin-locked';
export const PIN_LENGTH = 4;
export const DEFAULT_PIN_MINUTES = 5;

const ARROWS: Record<string, string> = { ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→' };

/**
 * The key a press stands for in a PIN: an arrow (`↑`), a lowercase letter or a digit (the number row or the keypad).
 * A letter with Shift or Caps Lock counts as its lowercase one. `null` for anything else, and for a press with Control,
 * Alt or Meta held, or one typed through an input method.
 */
export function pinKey(
  e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'altKey' | 'metaKey' | 'isComposing'>,
): string | null {
  if (e.ctrlKey || e.altKey || e.metaKey || e.isComposing) return null;
  if (ARROWS[e.key]) return ARROWS[e.key];
  if (e.key.length !== 1) return null;
  const k = e.key.toLowerCase();
  return /^[a-z0-9]$/.test(k) ? k : null;
}

/** Kept as a hash, only so the PIN is not lying in storage as typed (it protects nothing). */
async function hashed(keys: string[]): Promise<string> {
  const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`meghnaad-pin:${keys.join(' ')}`));
  return [...new Uint8Array(bytes)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

function get(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function put(key: string, value: string | null): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Not kept: without storage there is no PIN, and the device behaves as it did before one.
  }
}

export function hasPin(): boolean {
  return get(PIN) !== null;
}

export async function setPin(keys: string[]): Promise<void> {
  if (keys.length !== PIN_LENGTH) throw new Error(`A PIN is ${PIN_LENGTH} keys.`);
  put(PIN, await hashed(keys));
}

export function clearPin(): void {
  put(PIN, null);
  put(LOCKED, null);
}

export async function checkPin(keys: string[]): Promise<boolean> {
  const kept = get(PIN);
  return kept !== null && keys.length === PIN_LENGTH && (await hashed(keys)) === kept;
}

/** Minutes without use after which the PIN screen comes up (0: only when asked, with `:lock`). */
export function pinMinutes(): number {
  const n = Number(get(MINUTES) ?? DEFAULT_PIN_MINUTES);
  return Number.isFinite(n) && n >= 0 ? n : DEFAULT_PIN_MINUTES;
}

export function setPinMinutes(n: number): void {
  put(MINUTES, String(Math.max(0, Math.round(n))));
}

/** Whether the PIN screen is up: kept, so a reload of a PIN-locked tab opens PIN-locked. */
export function pinLocked(): boolean {
  return hasPin() && get(LOCKED) !== null;
}

export function markPinLocked(on: boolean): void {
  put(LOCKED, on ? '1' : null);
}

// @vitest-environment node
// The keys a PIN takes, and keeping it (spec 0022).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { checkPin, clearPin, hasPin, markPinLocked, pinKey, pinLocked, pinMinutes, setPin, setPinMinutes } from './pin';

const press = (key: string, held: Partial<Record<'ctrlKey' | 'altKey' | 'metaKey' | 'isComposing', boolean>> = {}) =>
  pinKey({ key, ctrlKey: false, altKey: false, metaKey: false, isComposing: false, ...held });

let storage: Map<string, string>;
beforeEach(() => {
  storage = new Map();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => storage.get(k) ?? null,
    setItem: (k: string, v: string) => void storage.set(k, v),
    removeItem: (k: string) => void storage.delete(k),
  });
});
afterEach(() => vi.unstubAllGlobals());

describe('a key of a PIN', () => {
  it('is an arrow, a lowercase letter or a digit; a capital counts as its lowercase', () => {
    expect(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].map((k) => press(k))).toEqual(['↑', '↓', '←', '→']);
    expect(['a', 'z', '0', '9'].map((k) => press(k))).toEqual(['a', 'z', '0', '9']);
    expect(press('B')).toBe('b');
  });

  it('is nothing else, nor anything pressed with Control, Alt or Meta, or through an input method', () => {
    for (const k of ['Backspace', 'Enter', 'Escape', 'Tab', 'Shift', ' ', '*', '-', 'é', 'F5', 'Home', 'Dead'])
      expect(press(k), k).toBeNull();
    expect(press('a', { ctrlKey: true })).toBeNull();
    expect(press('a', { metaKey: true })).toBeNull();
    expect(press('a', { altKey: true })).toBeNull();
    expect(press('a', { isComposing: true })).toBeNull();
  });
});

describe('the PIN', () => {
  it('is four keys, kept hashed, checked, and cleared with its screen', async () => {
    await expect(setPin(['1', '2', '3'])).rejects.toThrow('4 keys');
    expect(hasPin()).toBe(false);
    await setPin(['1', 'a', '↑', '9']);
    expect([...storage.values()].join(' ')).not.toContain('1 a ↑ 9');
    expect(await checkPin(['1', 'a', '↑', '9'])).toBe(true);
    expect(await checkPin(['1', 'a', '↓', '9'])).toBe(false);
    markPinLocked(true);
    expect(pinLocked()).toBe(true);
    clearPin();
    expect(hasPin()).toBe(false);
    expect(pinLocked()).toBe(false);
  });

  it('comes up after five idle minutes unless set otherwise; 0 is only when asked', () => {
    expect(pinMinutes()).toBe(5);
    setPinMinutes(0);
    expect(pinMinutes()).toBe(0);
  });
});

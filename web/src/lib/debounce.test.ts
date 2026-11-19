import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { debounce } from './debounce';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it('runs once, with the last arguments, after the calls stop', () => {
  const fn = vi.fn();
  const d = debounce(fn, 150);
  d('a');
  vi.advanceTimersByTime(100);
  d('ab');
  vi.advanceTimersByTime(100);
  expect(fn).not.toHaveBeenCalled();
  vi.advanceTimersByTime(50);
  expect(fn).toHaveBeenCalledExactlyOnceWith('ab');
});

it('runs a waiting call at once when flushed, and nothing when there is none', () => {
  const fn = vi.fn();
  const d = debounce(fn, 150);
  d('x');
  d.flush();
  expect(fn).toHaveBeenCalledExactlyOnceWith('x');
  d.flush();
  vi.advanceTimersByTime(500);
  expect(fn).toHaveBeenCalledTimes(1);
});

it('drops a waiting call when cancelled', () => {
  const fn = vi.fn();
  const d = debounce(fn, 150);
  d('x');
  d.cancel();
  vi.advanceTimersByTime(500);
  expect(fn).not.toHaveBeenCalled();
});

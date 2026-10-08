// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { newlineOnEnter, submitOnEnter } from './multiline';

/** A form with one textarea, which records whether it was asked to submit. */
function field() {
  const form = document.createElement('form');
  const area = document.createElement('textarea');
  form.append(area);
  document.body.append(form);
  const submitted = vi.fn();
  form.requestSubmit = submitted;
  return { area, submitted };
}

function press(area: HTMLTextAreaElement, handler: (e: KeyboardEvent) => void, init: KeyboardEventInit) {
  const e = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true, ...init });
  area.addEventListener('keydown', handler, { once: true });
  area.dispatchEvent(e);
  return e;
}

describe('an annotation field (Enter saves, Shift+Enter adds a line)', () => {
  it('saves on Enter and keeps the key from inserting a line', () => {
    const { area, submitted } = field();
    const e = press(area, submitOnEnter, {});
    expect(submitted).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('lets Shift+Enter through to add a line', () => {
    const { area, submitted } = field();
    const e = press(area, submitOnEnter, { shiftKey: true });
    expect(submitted).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
  });

  it('leaves other keys and an IME composition alone', () => {
    const { area, submitted } = field();
    press(area, submitOnEnter, { key: 'a' });
    press(area, submitOnEnter, { isComposing: true });
    expect(submitted).not.toHaveBeenCalled();
  });
});

describe('a string UDA field (Enter adds a line, Ctrl or Cmd+Enter saves)', () => {
  it('does not save on a plain Enter', () => {
    const { area, submitted } = field();
    const e = press(area, newlineOnEnter, {});
    expect(submitted).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
  });

  it('saves on Ctrl+Enter and on Cmd+Enter', () => {
    for (const init of [{ ctrlKey: true }, { metaKey: true }]) {
      const { area, submitted } = field();
      const e = press(area, newlineOnEnter, init);
      expect(submitted, JSON.stringify(init)).toHaveBeenCalledOnce();
      expect(e.defaultPrevented).toBe(true);
    }
  });
});

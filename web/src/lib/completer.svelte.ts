// Tab-completion state shared by the console and the filter bar: the first Tab completes the
// shared prefix, further Tabs cycle through a menu of options (Shift+Tab goes back).

import { apply, complete, insert, type Option, type Vocab } from './completion';

interface Menu {
  /** The line as it was when the menu opened; every selection is rebuilt from it. */
  base: string;
  start: number;
  end: number;
  options: Option[];
  /** -1 = nothing selected yet. */
  sel: number;
}

export interface Edit {
  line: string;
  caret: number;
}

export class Completer {
  menu = $state<Menu | null>(null);

  close() {
    this.menu = null;
  }

  #select(sel: number): Edit {
    const m = this.menu!;
    m.sel = sel;
    return insert(m.base, { start: m.start, end: m.end, options: m.options, common: '' }, m.options[sel].value);
  }

  /** Handle Tab. Returns the edit to apply, or null if there is nothing to complete. */
  tab(line: string, caret: number, vocab: Vocab, reverse = false): Edit | null {
    const m = this.menu;
    if (m) {
      const n = m.options.length;
      return this.#select(m.sel < 0 ? (reverse ? n - 1 : 0) : (m.sel + (reverse ? -1 : 1) + n) % n);
    }
    const c = complete(line, caret, vocab);
    if (c.options.length === 0) return null;
    const r = apply(line, c);
    if (c.options.length === 1) return { line: r.line, caret: r.caret };
    if (r.progressed) {
      // Completed the shared prefix: show what's left to choose from.
      this.menu = { base: r.line, start: c.start, end: c.start + c.common.length, options: c.options, sel: -1 };
      return { line: r.line, caret: r.caret };
    }
    this.menu = { base: line, start: c.start, end: c.end, options: c.options, sel: -1 };
    return this.#select(reverse ? c.options.length - 1 : 0);
  }

  /** A tapped option (no keyboard on a phone): select it and accept it. */
  pick(index: number): Edit | null {
    const m = this.menu;
    if (!m || index < 0 || index >= m.options.length) return null;
    const e = this.#select(index);
    this.menu = null;
    return e;
  }

  /** Arrow keys while the menu is open. */
  move(delta: 1 | -1): Edit | null {
    const m = this.menu;
    if (!m) return null;
    const n = m.options.length;
    return this.#select(m.sel < 0 ? (delta === 1 ? 0 : n - 1) : (m.sel + delta + n) % n);
  }
}

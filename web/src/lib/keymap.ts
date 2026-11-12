// The keys, as `tc_core::keymap` resolves them for the web app: the defaults (`/keymaps/default.json`, an asset served
// with the page as the colour themes are, and carried by the phone and the terminal app), then `keys` and `web.keys` of
// `settings.json`. An action is a dotted name (`app.prompt`); a key is written as Zed and VS Code write it (`ctrl+k`).
// The engine reads the keys and turns a key the browser gives into the same form, so the page, the phone and the
// terminal app read `settings.json` alike.

import { callHelper } from '../engine/helpers';

export interface Keymap {
  /** Each action's keys, as the engine writes them (`ctrl+k`, `/`, `enter`). */
  keys: Record<string, string[]>;
  /** Each context's keys and the action each runs. */
  contexts: Record<string, Record<string, string>>;
  /** What is wrong with `keys` or `web.keys`, one line each. */
  problems: string[];
}

/** Every action the web app has, with what it does, for the list in Settings. */
export const WEB_ACTIONS: { name: string; does: string }[] = [
  { name: 'app.prompt', does: 'Put the cursor in the prompt (from anywhere that isn’t a text field)' },
];

const NONE: Keymap = { keys: {}, contexts: {}, problems: [] };

/** The default keys of every front end, as the page serves them. */
export async function fetchKeymapDefaults(): Promise<Record<string, unknown>> {
  const r = await fetch('/keymaps/default.json');
  if (!r.ok) throw new Error('The default keys could not be loaded.');
  return (await r.json()) as Record<string, unknown>;
}

/** The web app's keys from the defaults and the whole of `settings.json`; none until the defaults are there. */
export function resolveKeymap(settings: Record<string, unknown>, defaults: Record<string, unknown> | null): Keymap {
  if (!defaults) return NONE;
  try {
    return callHelper<Keymap>('keymap', 'web', settings, defaults);
  } catch {
    // The helpers have not loaded: no keys rather than no page.
    return NONE;
  }
}

/** A key the browser gave, as the keymap writes it; null for one it has no name for (a modifier alone). */
export function chordOf(e: KeyboardEvent): string | null {
  const mods = [e.ctrlKey && 'ctrl', e.altKey && 'alt', e.shiftKey && 'shift', e.metaKey && 'meta'].filter(
    (m): m is string => !!m,
  );
  try {
    return callHelper<string | null>('key_chord', e.key, mods);
  } catch {
    return null;
  }
}

/** The action a key runs in `context` (the web app has one, `app`), if any. */
export function actionOf(keymap: Keymap, e: KeyboardEvent, context = 'app'): string | null {
  const chord = chordOf(e);
  return (chord && keymap.contexts[context]?.[chord]) ?? null;
}

/** A key as a hint shows it: `↵`, `Ctrl+K`, `/`. */
export function keyLabel(chord: string): string {
  try {
    return callHelper<string>('key_label', chord);
  } catch {
    return chord;
  }
}

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

/**
 * The commands of the `:` line as people read them (`/keymaps/commands.json`, an asset every front end carries, from
 * which `docs/commands.md` is made): what each does, the words it takes, more of it, and the version that brought it in.
 */
export interface CommandsDoc {
  /** The page of the documentation; a command's part of it is `#<its name>`. */
  docs: string;
  commands: { name: string; shortest: number; summary: string; takes: string; doc: string; since: string }[];
  /** The lines that move to a row (`:5`, `:+3`). */
  lines: { written: string[]; summary: string; doc: string; since: string }[];
}

/** A command, or a line that moves to a row, as a list of keys shows it. */
export interface KeyCommand {
  /** As it is written, the part that may be left out in brackets: `:tabn[ext]`, or `:5 :$`. */
  written: string;
  /** What it does, and the words it takes. */
  what: string;
  /** Where the documentation says more of it (empty for a line). */
  link: string;
}

/** Fetch the commands as people read them. */
export async function fetchCommandsDoc(): Promise<CommandsDoc> {
  const r = await fetch(`keymaps/commands.json?v=${encodeURIComponent(__APP_VERSION__)}`);
  if (!r.ok) throw new Error('The commands could not be loaded.');
  return (await r.json()) as CommandsDoc;
}

/** Whether what came in `since` is in a build of `version`: `unreleased` is (it comes out with it), and up to it. */
export function have(since: string, version: string = __APP_VERSION__): boolean {
  const parts = (v: string) =>
    v
      .split(/[.-]/)
      .slice(0, 3)
      .map((p) => Number(p) || 0);
  if (since === 'unreleased') return true;
  const [a, b] = [parts(since), parts(version)];
  for (let i = 0; i < 3; i++) if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) < (b[i] ?? 0);
  return true;
}

/** Every command a `:` line, or a key bound to one, can run in this build, then the lines that move to a row. */
export function keyCommands(doc: CommandsDoc | null, version: string = __APP_VERSION__): KeyCommand[] {
  if (!doc) return [];
  const what = (e: { summary: string; takes?: string }) => e.summary + (e.takes ? `; takes ${e.takes}` : '');
  return [
    ...doc.commands
      .filter((c) => have(c.since, version))
      .map((c) => ({
        written:
          ':' + c.name.slice(0, c.shortest) + (c.shortest < c.name.length ? `[${c.name.slice(c.shortest)}]` : ''),
        what: what(c),
        link: `${doc.docs}#${c.name}`,
      })),
    ...doc.lines
      .filter((l) => have(l.since, version))
      .map((l) => ({ written: l.written.join(' '), what: what(l), link: '' })),
  ];
}

/** The sections of `settings.json` that hold keys: `general` is `keys`, for every front end; the rest their own. */
export const KEY_SECTIONS = ['general', 'web', 'mobile', 'tui'] as const;
export type KeySection = (typeof KEY_SECTIONS)[number];

/** One action of a section, for the Keybindings tab (`tc_core::keymap::Binding`). */
export interface Binding {
  action: string;
  /** What it does, from the asset's `about`; empty for a name that is no action. */
  about: string;
  /** Its keys by default. */
  defaults: string[];
  /** The keys set in this section (`[]` for none); null where nothing is set. */
  keys: string[] | null;
}

export interface Bindings {
  sections: { id: KeySection; actions: Binding[] }[];
  problems: string[];
}

const NONE: Keymap = { keys: {}, contexts: {}, problems: [] };

/** The default keys of every front end, as the page serves them. */
export async function fetchKeymapDefaults(): Promise<Record<string, unknown>> {
  // With the app's version, so a browser or a cache that kept an older file does not leave new keys out.
  const r = await fetch(`keymaps/default.json?v=${encodeURIComponent(__APP_VERSION__)}`);
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

/** What a key or a `:` line asks for (`tc_core::keymap::Call`): an action, the words after it and a count (`5j`). */
export interface Call {
  action: string;
  args?: string[];
  count?: number;
}

/** The keys pressed that have not run anything yet: a count, and the start of a longer key (`g` of `gg`). */
export interface Pending {
  count?: number;
  keys?: string[];
}

/** A key pressed in `context` after the keys pending: what is pending now, and what to do, if anything. */
export function stepKey(
  keymap: Keymap,
  context: string,
  pending: Pending,
  chord: string,
): { pending: Pending; call?: Call } {
  try {
    return callHelper<{ pending: Pending; call?: Call }>('key_step', keymap.contexts[context] ?? {}, pending, chord);
  } catch {
    return { pending: {} };
  }
}

/** A command line that runs a command (`:tabn Tasks`, `:5`); null for a line that goes to `task` as it is. */
export function commandLine(line: string): Call | null {
  try {
    return callHelper<Call | null>('key_command_line', line);
  } catch {
    return null;
  }
}

/** What is pending, as a status line shows it (`5g`); empty for nothing. */
export function pendingLabel(p: Pending): string {
  const keys = p.keys?.length ? keyLabel(p.keys.join(' ')) : '';
  return `${p.count ?? ''}${keys}`;
}

/** The action a key runs in `context` (the web app has one, `app`), if any. */
export function actionOf(keymap: Keymap, e: KeyboardEvent, context = 'app'): string | null {
  const chord = chordOf(e);
  return (chord && keymap.contexts[context]?.[chord]) ?? null;
}

/** Every section's actions, the user's first; none until the defaults are there. */
export function keyBindings(settings: Record<string, unknown>, defaults: Record<string, unknown> | null): Bindings {
  const none: Bindings = { sections: KEY_SECTIONS.map((id) => ({ id, actions: [] })), problems: [] };
  if (!defaults) return none;
  try {
    return callHelper<Bindings>('keymap_bindings', settings, defaults);
  } catch {
    return none;
  }
}

/**
 * `settings` with an action's keys in a section set (`[]` for none), or taken out (`null`), bound in its group with its
 * default keys it no longer has unbound; the rest as it was.
 */
export function withBinding(
  settings: Record<string, unknown>,
  defaults: Record<string, unknown> | null,
  section: KeySection,
  action: string,
  keys: string[] | null,
): Record<string, unknown> {
  return callHelper<Record<string, unknown>>('keymap_set', settings, section, action, keys, defaults);
}

/**
 * `settings.json` brought to the latest version (`tc_core::settings_version`: the keys bound per group since 2), or
 * `null` when it already is. Every front end does this when it reads the file, and saves what comes back.
 */
export function migrateSettings(
  settings: Record<string, unknown>,
  defaults: Record<string, unknown> | null,
): Record<string, unknown> | null {
  try {
    return callHelper<Record<string, unknown> | null>('settings_migrate', settings, defaults);
  } catch {
    return null;
  }
}

/** A key as typed (`Ctrl-K`), as `settings.json` writes it, or what is wrong with it. */
export function parseKey(text: string): { key: string } | { problem: string } {
  try {
    return callHelper<{ key: string } | { problem: string }>('key_parse', text);
  } catch (e) {
    return { problem: e instanceof Error ? e.message : String(e) };
  }
}

/** A key as a hint shows it: `↵`, `Ctrl+K`, `/`. */
export function keyLabel(chord: string): string {
  try {
    return callHelper<string>('key_label', chord);
  } catch {
    return chord;
  }
}

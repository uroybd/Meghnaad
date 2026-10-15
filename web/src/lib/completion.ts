// Tab completion for the console: commands and reports, attributes and their modifiers,
// tags, task ids, and values (projects, priorities, UDA values, date words). Everything is
// built from what the engine has told us, and each option can carry a short hint. The completing is the
// engine's (`tc_core::complete`, through src/engine/helpers.ts), which every front end shares; the cases are tested there.

import { callHelper } from '../engine/helpers';
import type { UdaDef } from './types';

export interface TaskRef {
  id: number | null;
  uuid: string;
  description: string;
}

export interface Vocab {
  /** Report names, with their descriptions as hints. */
  reports: { name: string; description?: string | null }[];
  projects: string[];
  tags: string[];
  udas: UdaDef[];
  tasks: TaskRef[];
  contexts: string[];
}

export interface Option {
  value: string;
  hint?: string;
}

export interface Completion {
  /** Replace line[start:end] with `common` (or with one option's value). */
  start: number;
  end: number;
  options: Option[];
  /** Longest common prefix of the option values. */
  common: string;
}

/** The commands the console knows, as the engine lists them. */
export const COMMAND_NAMES: string[] = callHelper<string[]>('command_names');

/** The virtual tags Taskwarrior computes (`+OVERDUE`). */
export const VIRTUAL_TAGS: string[] = callHelper<string[]>('virtual_tags');

/** The vocabulary the engine last had; it is handed over again only when it is another one. */
let sent: Vocab | undefined;

/** What the word before the caret could become. Positions are UTF-16 units, as a text field counts them. */
export function complete(line: string, caret: number, v: Vocab): Completion {
  if (v !== sent) {
    callHelper('set_vocab', v);
    sent = v;
  }
  return callHelper<Completion>('complete', line, caret);
}

/** Insert `value` in place of the completed word; add a space unless it ends mid-token. */
export function insert(line: string, c: Completion, value: string, space = true): { line: string; caret: number } {
  return callHelper('insert_completion', line, c, value, space);
}

/**
 * Tab behaviour: complete the shared prefix; when that makes no progress, the caller opens a
 * menu of `options`. A single option is completed in full (plus a space).
 */
export function apply(line: string, c: Completion): { line: string; caret: number; progressed: boolean } {
  return callHelper('apply_completion', line, c);
}

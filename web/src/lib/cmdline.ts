// Turning between a typed command line and the argument array the API takes.
// Splitting and quoting are the engine's own (`tc_core::helpers`, through src/engine/helpers.ts), so what the GUI previews
// is what runs.

import { callHelper } from '../engine/helpers';

/** The words of a command line, as the engine reads them: the engine's own `filter::split_words`. */
export function splitWords(s: string): string[] {
  return callHelper<string[]>('split_words', s);
}

/** A word as it is typed in a command line: as it is when that is safe, else in single quotes. */
export function shellQuote(a: string): string {
  return callHelper<string>('shell_quote', a);
}

/** The command as it would be typed: `task project:Home +work list`. */
export function previewLine(args: string[]): string {
  return ['task', ...args.map(shellQuote)].join(' ');
}

/** Filter words + a report name, as sent to the API. */
export function reportArgs(filter: string, report: string): string[] {
  return [...splitWords(filter), report];
}

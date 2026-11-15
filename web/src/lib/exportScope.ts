import { callHelper } from '../engine/helpers';
import { splitWords } from './cmdline';
import type { ConfigResponse } from './types';

// What the Export button over a table exports, and the context a board's `export` takes: the engine's knowledge
// (`tc_core::table_export`), shared with the phone and the terminal app. A table is a report run with a filter, so its
// export is `<filter> export <report>`, with the context it ran in put in front (`export` itself leaves it out).

type Settings = ConfigResponse['config'];

/** The words of `input` (a line or its args). */
export const wordsOf = (input: { line?: string; args?: string[] }): string[] =>
  input.args ?? splitWords(input.line ?? '');

/**
 * The command line that exports what the table drawn by `input` shows, in the page's context (`pageChoice`, a name or
 * `none`; none: the active one); null when it isn't a report's.
 */
export function exportArgs(
  input: { line?: string; args?: string[] },
  pageChoice: string | undefined,
  cfg: Settings | undefined,
): string[] | null {
  if (!cfg) return null;
  return callHelper<string[] | null>('export_line', wordsOf(input), cfg, pageChoice ?? null);
}

/** The context's read filter for a filter's `export`, in parentheses: the one it names, else the page's, else the active one. */
export function contextWords(filter: string[], pageChoice: string | undefined, cfg: Settings | undefined): string[] {
  if (!cfg) return [];
  return callHelper<string[]>('export_context', filter, cfg, pageChoice ?? null);
}

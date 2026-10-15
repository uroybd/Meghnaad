// Syntax highlighting for a line of Taskwarrior syntax: what each piece is is the engine's, and the pieces to draw are made here.

import { callHelper } from '../engine/helpers';

export type Hl = 'keyword' | 'command' | 'attr' | 'punct' | 'value' | 'str' | 'tag' | 'id' | 'op' | 'config' | 'text';

/** One piece of the line, `[start, end)`, and what it is. Spaces between pieces are not pieces. */
export interface HlToken {
  start: number;
  end: number;
  kind: Hl;
}

/**
 * The pieces of the line: the engine's own (`tc_core::highlight`, through src/engine/helpers.ts), which every front end
 * shares. `commands`: every word that is a command here: Taskwarrior's, the reports, and the user's own. Only the first one
 * found counts.
 */
export function highlight(line: string, commands: ReadonlySet<string>): HlToken[] {
  return callHelper<HlToken[]>('highlight', line, [...commands]);
}

/** The whole line as pieces to draw: each coloured word and the plain text between them, so that nothing is lost or added. */
export function highlightPieces(line: string, commands: ReadonlySet<string>): { text: string; kind: Hl }[] {
  const out: { text: string; kind: Hl }[] = [];
  let at = 0;
  for (const t of highlight(line, commands)) {
    if (t.start > at) out.push({ text: line.slice(at, t.start), kind: 'text' });
    out.push({ text: line.slice(t.start, t.end), kind: t.kind });
    at = t.end;
  }
  if (at < line.length) out.push({ text: line.slice(at), kind: 'text' });
  return out;
}

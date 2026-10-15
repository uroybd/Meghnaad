// The Markdown in a note or a string UDA. The engine parses it (`tc_core::markdown`, the same for every front end) and
// changes the text when a checkbox is ticked; what is here is the shape of its tree and a call each.

import { callHelper } from '../engine/helpers';

/** A piece of the tree: a block (`p`, `h`, `ul`, `ol`, `li`, `q`, `pre`, `hr`) or an inline (`text`, `b`, `i`, `del`, `code`, `a`, `br`). */
export interface MdNode {
  t: string;
  /** The text of `text`, `code` and `pre`. */
  s?: string;
  c?: MdNode[];
  /** The level of `h`; the number a `ol` starts at. */
  n?: number;
  href?: string;
  /** On an `li` that is a checklist item: whether it is ticked. */
  done?: boolean;
  /** On a checklist item: the line of the text it is on, which is what `toggled` takes. */
  line?: number;
  /** The language after the fence of a `pre`. */
  lang?: string;
}

/** The trees of some texts, in order; null for one that is only words (it is drawn as it is). */
export function trees(texts: string[]): (MdNode[] | null)[] {
  return texts.length ? callHelper<(MdNode[] | null)[]>('markdown', texts) : [];
}

/** The text with the checkbox on `line` flipped, or null when that line is not a checklist item. */
export function toggled(text: string, line: number): string | null {
  return callHelper<string | null>('markdown_toggle', text, line);
}

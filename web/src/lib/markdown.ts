// The Markdown in a note or a string UDA. The engine parses it (`tc_core::markdown`, the same for every front end) and
// changes the text when a checkbox is ticked; what is here is the shape of its tree and a call each.

import { callHelper } from '../engine/helpers';
import type { Transformation } from './appsettings';

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

/**
 * The trees of some texts, in order; null for one that is only words (it is drawn as it is). With the field each text is
 * in (`description`, `annotations` or a UDA's name) and the user's transformations, each is transformed first; a
 * description gets a tree only when a transformation changed it. `tasks`: the UUID of the task each text is of, for a
 * rule with a condition (`store.rules` carries the tasks it matches).
 */
export function trees(
  texts: string[],
  fields?: string[],
  rules?: Transformation[],
  tasks?: (string | null)[],
): (MdNode[] | null)[] {
  if (!texts.length) return [];
  return rules?.length && fields
    ? callHelper<(MdNode[] | null)[]>('markdown', texts, fields, rules, tasks ?? [])
    : callHelper<(MdNode[] | null)[]>('markdown', texts);
}

/** What is wrong with some transformations, one line each; `fields` are the string UDAs a rule may name. */
export function transformProblems(rules: Transformation[], fields: string[]): string[] {
  return rules.length ? callHelper<string[]>('transform_check', rules, fields) : [];
}

/** The text with the checkbox on `line` flipped, or null when that line is not a checklist item. */
export function toggled(text: string, line: number): string | null {
  return callHelper<string | null>('markdown_toggle', text, line);
}

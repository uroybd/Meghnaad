// Compound command lines: `a && b`, `a || b`, `a ; b`. They are the shell's, not Taskwarrior's, so the page reads them
// and runs one statement at a time; the engine only ever sees one plain command per request.
//
// As in bash, an operator that is not quoted or escaped always separates statements, so a filter written with `&&` or
// `||` has to be written `and` or `or` (or quoted, `'||'`). Operators bind left to right with no precedence:
// `a && b || c` is `(a && b) || c`, and a statement that is skipped leaves the status as it was.

/** How a statement follows the one before it. */
export type Joiner = '&&' | '||' | ';';

export interface Statement {
  /** The statement as it is typed (its own quoting kept), for the engine to read. */
  line: string;
  /** How it follows the previous statement; `null` for the first. */
  join: Joiner | null;
}

/** A line that is not a well-formed chain (an operator with nothing on one side). */
export class ChainError extends Error {}

/**
 * The statements of a line. A line without operators is one statement, exactly as typed (trimmed). Quotes (`'` and `"`)
 * and a backslash protect an operator the way the engine's own word splitting reads them; a trailing `;` is allowed.
 */
export function splitChain(line: string): Statement[] {
  const parts: { text: string; join: Joiner | null }[] = [{ text: '', join: null }];
  let quote: string | null = null;
  const add = (s: string) => (parts[parts.length - 1].text += s);
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quote) {
      add(c);
      if (c === quote) quote = null;
    } else if (c === "'" || c === '"') {
      quote = c;
      add(c);
    } else if (c === '\\' && i + 1 < line.length) {
      const next = line[i + 1];
      // The engine reads `\'`, `\"`, `\\` and `\ ` itself; an escaped operator character becomes a quoted one.
      add('\'"\\ '.includes(next) ? c + next : '&|;'.includes(next) ? `'${next}'` : c);
      if ('\'"\\ &|;'.includes(next)) i++;
    } else if (c === ';') {
      parts.push({ text: '', join: ';' });
    } else if ((c === '&' || c === '|') && line[i + 1] === c) {
      parts.push({ text: '', join: (c + c) as Joiner });
      i++;
    } else {
      add(c);
    }
  }
  const out: Statement[] = parts.map((p) => ({ line: p.text.trim(), join: p.join }));
  // A trailing `;` ends the last statement, as in a shell.
  if (out.length > 1 && !out[out.length - 1].line && out[out.length - 1].join === ';') out.pop();
  if (out.length === 1) return out[0].line ? out : [];
  out.forEach((s, i) => {
    if (s.line) return;
    const before = s.join;
    const after = out[i + 1]?.join;
    throw new ChainError(
      i === 0
        ? `Nothing before ‘${out[1].join}’.`
        : i === out.length - 1
          ? `Nothing after ‘${before}’.`
          : `Nothing between ‘${before}’ and ‘${after}’.`,
    );
  });
  return out;
}

/**
 * Run the statements in order, as a shell does: `&&` runs the next one only after a success, `||` only after a failure,
 * `;` always. `step(i)` runs statement `i` and says whether it succeeded. A statement that is skipped leaves the status
 * as it was. Returns the status of the last statement that ran.
 */
export async function runSequence(joins: (Joiner | null)[], step: (i: number) => Promise<boolean>): Promise<boolean> {
  let ok = true;
  for (let i = 0; i < joins.length; i++) {
    const join = joins[i];
    if ((join === '&&' && !ok) || (join === '||' && ok)) continue;
    ok = await step(i);
  }
  return ok;
}

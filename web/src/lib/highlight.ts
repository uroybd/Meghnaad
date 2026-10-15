// Syntax highlighting for a line of Taskwarrior syntax (app/…/Highlight.kt has the same rules and the same test table).
// It looks at the shape of each word only: a command is a word the vocabulary knows, `name:value` is an attribute, `+tag` a tag.

export type Hl = 'keyword' | 'command' | 'attr' | 'punct' | 'value' | 'str' | 'tag' | 'id' | 'op' | 'config' | 'text';

/** One piece of the line, `[start, end)`, and what it is. Spaces between pieces are not pieces. */
export interface HlToken {
  start: number;
  end: number;
  kind: Hl;
}

const OPERATORS = new Set(['and', 'or', 'xor', 'not', '!']);
/** After these the rest of the line is a description, where `and` is a word and not an operator. */
const DESCRIBING = new Set(['add', 'log', 'annotate']);
const TAG = /^[+-][A-Za-z_][A-Za-z0-9_.-]*$/;
const NUMBERS = /^[0-9]+([,-][0-9]+)*$/;
const UUID = /^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})?$/;
const ATTRIBUTE = /^([A-Za-z_][A-Za-z0-9_.]*)([:=])([\s\S]*)$/;

/** The words of the line as `[start, end)`: split at spaces, except inside quotes, and after a backslash. */
function words(line: string): [number, number][] {
  const out: [number, number][] = [];
  let i = 0;
  while (i < line.length) {
    if (/\s/.test(line[i])) {
      i++;
      continue;
    }
    const start = i;
    let quote: string | null = null;
    while (i < line.length) {
      const c = line[i];
      if (c === '\\' && i + 1 < line.length) {
        i += 2;
        continue;
      }
      if (quote !== null) {
        if (c === quote) quote = null;
      } else if (c === '"' || c === "'") {
        quote = c;
      } else if (/\s/.test(c)) break;
      i++;
    }
    out.push([start, i]);
  }
  return out;
}

/** `commands`: every word that is a command here: Taskwarrior's, the reports, and the user's own. Only the first one found counts. */
export function highlight(line: string, commands: ReadonlySet<string>): HlToken[] {
  const out: HlToken[] = [];
  let commandSeen = false;
  let describing = false;
  words(line).forEach(([from, to], index) => {
    let s = from;
    let e = to;
    if (index === 0 && line.slice(s, e) === 'task') {
      out.push({ start: s, end: e, kind: 'keyword' });
      return;
    }
    // Brackets around a word are operators of their own.
    while (!describing && s < e && line[s] === '(') {
      out.push({ start: s, end: s + 1, kind: 'op' });
      s++;
    }
    const closers: HlToken[] = [];
    while (!describing && e > s && line[e - 1] === ')') {
      closers.push({ start: e - 1, end: e, kind: 'op' });
      e--;
    }
    if (s < e) {
      const w = line.slice(s, e);
      const attribute = ATTRIBUTE.exec(w);
      if (!describing && OPERATORS.has(w.toLowerCase())) out.push({ start: s, end: e, kind: 'op' });
      else if (w[0] === '"' || w[0] === "'") out.push({ start: s, end: e, kind: 'str' });
      else if (TAG.test(w)) out.push({ start: s, end: e, kind: 'tag' });
      else if (NUMBERS.test(w) || (UUID.test(w) && /[0-9]/.test(w))) out.push({ start: s, end: e, kind: 'id' });
      else if (attribute) {
        const name = attribute[1];
        const config = name === 'rc' || name.startsWith('rc.');
        out.push({ start: s, end: s + name.length, kind: config ? 'config' : 'attr' });
        out.push({ start: s + name.length, end: s + name.length + 1, kind: 'punct' });
        const v = s + name.length + 1;
        if (v < e) {
          out.push({
            start: v,
            end: e,
            kind: config ? 'config' : line[v] === '"' || line[v] === "'" ? 'str' : 'value',
          });
        }
      } else if (!commandSeen && commands.has(w)) {
        commandSeen = true;
        describing = DESCRIBING.has(w);
        out.push({ start: s, end: e, kind: 'command' });
      } else out.push({ start: s, end: e, kind: 'text' });
    }
    out.push(...closers.reverse());
  });
  return out;
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

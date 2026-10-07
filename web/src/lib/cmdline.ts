// Turning between a typed command line and the argument array the API takes.
// `splitWords` mirrors the server's `filter::split_words` so what the GUI previews is what runs.

export function splitWords(s: string): string[] {
  const out: string[] = [];
  let cur = '';
  let inWord = false;
  let quote: string | null = null;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quote) {
      if (c === quote) quote = null;
      else cur += c;
    } else if (c === "'" || c === '"') {
      quote = c;
      inWord = true;
    } else if (c === '\\' && i + 1 < s.length && /['"\\ ]/.test(s[i + 1])) {
      cur += s[++i];
      inWord = true;
    } else if (/\s/.test(c)) {
      if (inWord) {
        out.push(cur);
        cur = '';
        inWord = false;
      }
    } else {
      cur += c;
      inWord = true;
    }
  }
  if (inWord) out.push(cur);
  return out;
}

export function shellQuote(a: string): string {
  if (a !== '' && /^[A-Za-z0-9_@%+=:,./()-]+$/.test(a)) return a;
  return `'${a.replace(/'/g, `'\\''`)}'`;
}

/** The command as it would be typed: `task project:Home +work list`. */
export function previewLine(args: string[]): string {
  return ['task', ...args.map(shellQuote)].join(' ');
}

/** Filter words + a report name, as sent to the API. */
export function reportArgs(filter: string, report: string): string[] {
  return [...splitWords(filter), report];
}

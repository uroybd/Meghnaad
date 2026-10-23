// The paths of the taskrc folder, as the page handles them: which files an upload holds and where they go, and the
// `include` line that names one file from another. (What the engine does with a path is `tc_core::rcfiles`.)

/** The file everything starts from. */
export const ROOT = 'taskrc';
/** What the engine takes: at most this many files of this much text in all (`tc_core::rcfiles`). */
export const MAX_FILES = 64;
export const MAX_FILE_BYTES = 256 * 1024;
export const MAX_TOTAL_BYTES = 1024 * 1024;

/** The `include` line that names `to` from `from`: the path is written from the folder `from` is in. */
export function includeLine(from: string, to: string): string {
  const here = from.split('/').slice(0, -1);
  const there = to.split('/');
  let common = 0;
  while (common < here.length && common < there.length - 1 && here[common] === there[common]) common++;
  return `include ${'../'.repeat(here.length - common)}${there.slice(common).join('/')}`;
}

/** Add an `include` of `to` to the text of `from`, on a line of its own, unless it is already there. */
export function withInclude(text: string, from: string, to: string): string {
  const line = includeLine(from, to);
  if (text.split(/\r?\n/).some((l) => l.trim() === line)) return text;
  return `${text}${text === '' || text.endsWith('\n') ? '' : '\n'}${line}\n`;
}

/** A file picked for upload: its name, the path inside the folder that was picked (if one was), and its text. */
export interface Picked {
  name: string;
  relative?: string;
  text: string;
}

export interface Upload {
  files: { path: string; text: string }[];
  /** What was left out, and why. */
  skipped: string[];
}

/**
 * Which of the picked files to send, and where each goes. The folder that was picked is not part of a path (its files are
 * `taskrc`, `holidays/en-US.rc`), `.taskrc` at the top is `taskrc`, and what is not a taskrc file (hidden files and
 * folders such as `.git`, binary files, anything too large, anything past the most a folder holds) is left out and said.
 */
export function uploadFiles(picked: Picked[]): Upload {
  const out: Upload = { files: [], skipped: [] };
  const seen = new Set<string>();
  let total = 0;
  for (const p of picked) {
    const whole = (p.relative ?? p.name).split('/');
    const parts = p.relative ? whole.slice(1) : whole;
    const label = whole.join('/');
    if (parts.length === 0) continue;
    if (parts.length === 1 && parts[0] === '.taskrc') parts[0] = ROOT;
    else if (parts.some((part) => part.startsWith('.'))) {
      out.skipped.push(`${label} (hidden)`);
      continue;
    }
    const path = parts.join('/');
    if (p.text.includes('\u0000')) out.skipped.push(`${label} (not text)`);
    else if (p.text.length > MAX_FILE_BYTES) out.skipped.push(`${label} (too large)`);
    else if (seen.has(path)) out.skipped.push(`${label} (the same name as another)`);
    else if (out.files.length >= MAX_FILES || total + p.text.length > MAX_TOTAL_BYTES)
      out.skipped.push(`${label} (the folder is full)`);
    else {
      seen.add(path);
      total += p.text.length;
      out.files.push({ path, text: p.text });
    }
  }
  // `taskrc` first, so what a folder starts from is the first thing it lists.
  out.files.sort((a, b) => Number(b.path === ROOT) - Number(a.path === ROOT) || a.path.localeCompare(b.path));
  return out;
}

export interface Row {
  /** The file's path; empty on the row that names a folder. */
  path: string;
  depth: number;
  name: string;
  folder: string | null;
}

/** The files as the folder is drawn: `taskrc` first, each folder's name once above its files, each row with its depth. */
export function listing(paths: string[]): Row[] {
  const sorted = [...paths].sort((a, b) => Number(b === ROOT) - Number(a === ROOT) || a.localeCompare(b));
  const rows: Row[] = [];
  let last = '';
  for (const path of sorted) {
    const parts = path.split('/');
    const folder = parts.length > 1 ? parts.slice(0, -1).join('/') : null;
    if (folder !== null && folder !== last)
      rows.push({ path: '', depth: parts.length - 2, name: parts[parts.length - 2], folder });
    last = folder ?? '';
    rows.push({ path, depth: parts.length - 1, name: parts[parts.length - 1], folder });
  }
  return rows;
}

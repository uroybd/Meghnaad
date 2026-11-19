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

/** A row of the file tree: a folder (open or not; empty when no file is in it) or a file. */
export interface TreeRow {
  kind: 'folder' | 'file';
  /** From the top of the folder: `holidays/en.rc`, or a folder's own path, `holidays`. */
  path: string;
  name: string;
  depth: number;
  /** A folder: whether its rows are shown. */
  open: boolean;
  /** A folder with no file in it, anywhere below: it lives only in the page, and is never saved. */
  empty: boolean;
}

/**
 * The files as a tree: `taskrc` first, then in each folder its folders and then its files, by name. Every folder a path
 * runs through is a row, as are the empty ones the page holds (`empty`); the rows in a folder that is `closed` are left
 * out.
 */
export function tree(paths: string[], empty: string[] = [], closed: ReadonlySet<string> = new Set()): TreeRow[] {
  const folders = new Set<string>();
  const addFolders = (path: string, includeSelf: boolean) => {
    const parts = path.split('/');
    for (let i = 1; i < parts.length + (includeSelf ? 1 : 0); i++) folders.add(parts.slice(0, i).join('/'));
  };
  for (const p of paths) addFolders(p, false);
  for (const f of empty) addFolders(f, true);
  const parent = (p: string) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/')) : '');
  const name = (p: string) => p.slice(p.lastIndexOf('/') + 1);
  const hasFile = (f: string) => paths.some((p) => p.startsWith(`${f}/`));
  const rows: TreeRow[] = [];
  const walk = (dir: string, depth: number) => {
    const byName = (a: string, b: string) => name(a).localeCompare(name(b));
    const files = paths
      .filter((p) => parent(p) === dir)
      .sort((a, b) => Number(b === ROOT) - Number(a === ROOT) || byName(a, b));
    const subs = [...folders].filter((f) => parent(f) === dir).sort(byName);
    const top: string[] = dir === '' ? files.filter((p) => p === ROOT) : [];
    for (const p of top) rows.push({ kind: 'file', path: p, name: name(p), depth, open: false, empty: false });
    for (const f of subs) {
      const open = !closed.has(f);
      rows.push({ kind: 'folder', path: f, name: name(f), depth, open, empty: !hasFile(f) });
      if (open) walk(f, depth + 1);
    }
    for (const p of files.filter((p) => !top.includes(p)))
      rows.push({ kind: 'file', path: p, name: name(p), depth, open: false, empty: false });
  };
  walk('', 0);
  return rows;
}

/** What is wrong with `name` as a new file or folder in the folder `dir`, or `null`. `taken` are the paths there are. */
export function newNameProblem(dir: string, name: string, taken: string[]): string | null {
  const n = name.trim().replace(/^\.\//, '');
  if (!n) return 'a name is needed';
  if (n.startsWith('/') || n.startsWith('~') || n.includes('\\'))
    return 'a name inside the folder, without / or ~ in front';
  const parts = n.split('/');
  if (parts.some((p) => p === '' || p === '.' || p === '..')) return 'no empty part, . or .. in a name';
  if ([...n].some((c) => c < ' ')) return 'no control characters';
  const path = dir ? `${dir}/${n}` : n;
  if (path.length > 200) return 'at most 200 characters';
  if (taken.some((t) => t === path || t.startsWith(`${path}/`))) return `${path} is there already`;
  return null;
}

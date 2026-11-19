// Colour themes: sets of `color.*` lines. Taskwarrior's own are plain files (`include dark-256.theme`), which a
// browser cannot include, so they are served as static files from /themes. One picked in Appearance is saved as a file
// of the taskrc folder, `themes/<id>.rc`, which the main taskrc includes. They are not part of the Worker; the engine
// only ever sees what was saved.

export interface Theme {
  id: string;
  label: string;
  note?: string;
}

/** Taskwarrior's defaults first (what applies when nothing is set, as in `task`), then the app's own, then the rest. */
export const THEMES: Theme[] = [
  { id: 'default', label: 'Taskwarrior default', note: "Taskwarrior's own default colours (the default)" },
  { id: 'meghnaad', label: 'Meghnaad', note: 'soft colours for the web' },
  { id: 'dark-16', label: 'Dark, 16 colours' },
  { id: 'dark-256', label: 'Dark, 256 colours' },
  { id: 'dark-blue-256', label: 'Dark blue' },
  { id: 'dark-gray-256', label: 'Dark gray' },
  { id: 'dark-gray-blue-256', label: 'Dark gray blue' },
  { id: 'dark-green-256', label: 'Dark green' },
  { id: 'dark-red-256', label: 'Dark red' },
  { id: 'dark-violets-256', label: 'Dark violets' },
  { id: 'dark-yellow-green', label: 'Dark yellow green' },
  { id: 'bubblegum-256', label: 'Bubblegum' },
  { id: 'light-16', label: 'Light, 16 colours' },
  { id: 'light-256', label: 'Light, 256 colours' },
  { id: 'solarized-dark-256', label: 'Solarized dark' },
  { id: 'solarized-light-256', label: 'Solarized light' },
  { id: 'no-color', label: 'No colour', note: 'every colour rule switched off' },
];

export const isTheme = (id: string) => THEMES.some((t) => t.id === id);

/** A line that sets a colour or how colours combine (not the `color=on|off` switch). */
const colourLine = (l: string) => /^\s*(color\.|rule\.precedence\.color|rule\.color\.merge)/.test(l);

/** The `name=value` lines of a theme file, without its comments (a `#` starts one anywhere in a line). */
export function themeLines(file: string): string[] {
  return file
    .split('\n')
    .map((l) => l.split('#')[0].trim())
    .filter(Boolean);
}

/** Where a theme picked in Appearance is saved, as a file of the taskrc folder. */
export const themePath = (id: string) => `themes/${id}.rc`;

/** The line of the main taskrc that includes a theme picked in Appearance. */
const themeInclude = (l: string) => /^\s*include\s+themes\/[\w.-]+\.rc\s*(#.*)?$/.test(l);

/**
 * Choosing a theme: its file goes in the folder as `themes/<id>.rc`, as it is, and the main taskrc (`text`) includes it.
 * The main taskrc loses its own colour lines, which would win over the theme, and the include of any theme picked
 * before. Taskwarrior's defaults are what applies when no colour is set, so choosing them only clears those, with no
 * file. The `color=on|off` line and everything else are kept.
 */
export function chooseTheme(
  text: string,
  id: string,
  file: string,
): { main: string; theme: { path: string; text: string } | null } {
  const kept = text.split('\n').filter((l) => !colourLine(l) && !themeInclude(l) && !/^# theme: /.test(l));
  while (kept.length && kept[kept.length - 1].trim() === '') kept.pop();
  if (id === 'default') return { main: kept.join('\n') + (kept.length ? '\n' : ''), theme: null };
  const main = [...kept, ...(kept.length ? [''] : []), `include ${themePath(id)}`].join('\n') + '\n';
  return { main, theme: { path: themePath(id), text: file.endsWith('\n') ? file : `${file}\n` } };
}

/** The theme the main taskrc includes, picked in Appearance, if any. */
export function chosenTheme(text: string): string | null {
  for (const l of text.split('\n').reverse()) {
    const m = /^\s*include\s+themes\/([\w.-]+)\.rc\s*(#.*)?$/.exec(l);
    if (m && isTheme(m[1])) return m[1];
  }
  return null;
}

/** Turn colours on or off: the `color` line of the taskrc. */
export function withColour(text: string, on: boolean): string {
  const kept = text.split('\n').filter((l) => !/^\s*color\s*[=\s]/.test(l));
  while (kept.length && kept[kept.length - 1].trim() === '') kept.pop();
  if (!on) kept.push('color=off');
  return kept.join('\n') + (kept.length ? '\n' : '');
}

/** Whether the text turns colour off. */
export function colourOff(text: string): boolean {
  return text.split('\n').some((l) => /^\s*color\s*[=\s]\s*(off|0|no|false|n)\s*$/i.test(l.split('#')[0]));
}

/** `include dark-256.theme` (a path in front is fine) names one of the bundled themes. */
export function includedTheme(line: string): string | null {
  const m = /^\s*include\s+(?:\S*\/)?([\w.-]+?)\.theme\s*$/.exec(line);
  return m && isTheme(m[1]) ? m[1] : null;
}

/** Replace each `include <bundled>.theme` line by that theme's lines. Other lines, and other includes, stay. */
export async function expandIncludes(text: string, fetchTheme: (id: string) => Promise<string>): Promise<string> {
  const out: string[] = [];
  for (const line of text.split('\n')) {
    const id = includedTheme(line);
    if (!id) {
      out.push(line);
      continue;
    }
    out.push(`# theme: ${id}`, ...themeLines(await fetchTheme(id)));
  }
  return out.join('\n');
}

export async function fetchTheme(id: string): Promise<string> {
  const r = await fetch(`themes/${id}.theme`);
  if (!r.ok) throw new Error(`The ${id} theme could not be loaded.`);
  return r.text();
}

// Colour themes: sets of `color.*` lines. Taskwarrior's own are plain files (`include dark-256.theme`), which a
// browser cannot include, so they are served as static files from /themes and put into the taskrc text when
// picked. They are not part of the Worker; the engine only ever sees the lines that were saved.

export interface Theme {
  id: string;
  label: string;
  note?: string;
}

/** The app's own theme first (it is what applies when nothing is set), then Taskwarrior's. */
export const THEMES: Theme[] = [
  { id: 'meghnaad', label: 'Meghnaad', note: 'soft colours for the web (the default)' },
  { id: 'default', label: 'Taskwarrior default', note: "Taskwarrior's own default colours" },
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

/**
 * `text` with its colour lines replaced by the theme's. The Meghnaad theme is the engine's default, so choosing
 * it just clears the colour lines. The `color=on|off` line and everything else are kept.
 */
export function applyTheme(text: string, id: string, file: string): string {
  const kept = text.split('\n').filter((l) => !colourLine(l));
  while (kept.length && kept[kept.length - 1].trim() === '') kept.pop();
  if (id === 'meghnaad') return kept.join('\n') + (kept.length ? '\n' : '');
  return [...kept, ...(kept.length ? [''] : []), `# theme: ${id}`, ...themeLines(file)].join('\n') + '\n';
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
  const r = await fetch(`/themes/${id}.theme`);
  if (!r.ok) throw new Error(`The ${id} theme could not be loaded.`);
  return r.text();
}

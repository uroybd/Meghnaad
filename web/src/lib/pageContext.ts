import { splitWords } from './cmdline';

// A page's own context. The context set with `context work` is the console's (it is Taskwarrior's, one for the whole
// session); a page may choose another, or none, for what it shows. It is sent the way Taskwarrior takes an override on a
// command line, `rc.context=<name>`, so the engine does nothing new.

/** The pages that choose a context: the report pages. (A board names its context in its filter: `rc.context=this_week`.) */
export type Scope = 'tasks' | 'summary' | 'calendar' | 'burndown' | 'projects' | 'tags';

/** What a page may say it wants: no context at all (as `context none`). */
export const NO_CONTEXT = 'none';

/** The word that makes a command run in `choice`; `choice` is a context's name or `none`. */
export const contextWord = (choice: string): string => `rc.context=${choice}`;

/** `input` run in the context `choice` names; unchanged when the page follows the active one (`undefined`). */
export function withContext<T extends { line?: string; args?: string[] }>(input: T, choice: string | undefined): T {
  if (!choice) return input;
  const word = contextWord(choice);
  return {
    ...input,
    ...(input.args ? { args: [word, ...input.args] } : {}),
    ...(input.line !== undefined ? { line: `${word} ${input.line}` } : {}),
  };
}

const KEY = 'tw-web-page-contexts';

/** The choices kept in this browser (a convenience, like the open tab: not part of anything saved or synced). */
export function loadChoices(): Record<string, string> {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '{}');
    if (!v || typeof v !== 'object' || Array.isArray(v)) return {};
    const kept: Record<string, string> = {};
    for (const [scope, c] of Object.entries(v)) if (typeof c === 'string' && c) kept[scope] = c;
    return kept;
  } catch {
    return {};
  }
}

export function saveChoices(choices: Record<string, string>) {
  try {
    localStorage.setItem(KEY, JSON.stringify(choices));
  } catch {
    /* private mode: the choice lasts until the page is closed */
  }
}

/** The context a filter names with `rc.context=<name>` (the last such word, as the engine reads them), or `undefined` if it names none. */
export function contextOfFilter(filter: string): string | undefined {
  let found: string | undefined;
  for (const w of splitWords(filter)) {
    const m = /^rc\.context[:=](.*)$/.exec(w);
    if (m) found = m[1] || NO_CONTEXT;
  }
  return found;
}

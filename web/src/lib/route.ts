// Every page has an address, so a link can be shared with anyone who can sign in to the same deployment, and the back
// button goes where it did:
//
//   /report/<name>      Tasks, as that report (a report is its own slug: `next`, `list`, `_reviewed`, one of yours)
//   /projects /tags /summary /calendar /burndown /console
//   /boards             the list of boards;  /board/<slug>  one of them
//   /settings           Settings;  /settings/<tab>  one of its tabs;  /settings/taskrc/<path>  one file of the taskrc folder
//   /task/<8 characters>  a task's detail drawer (the first 8 characters of its uuid; the numbers `task 3` uses differ
//                       from one replica to the next and change as tasks finish)
//
// `/` is the home page the settings choose. `Place` is where the app is; `parsePath` and `placePath` turn it into an
// address and back.

/** The tabs of Settings. */
export const SETTINGS_TABS = ['general', 'taskrc', 'tasksh', 'commands', 'hooks', 'urgency'] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];

/** The pages that are just a page. */
export const PLAIN_PAGES = ['projects', 'tags', 'summary', 'calendar', 'burndown', 'console'] as const;
export type PlainPage = (typeof PLAIN_PAGES)[number];

export type Place =
  | { page: 'tasks'; report: string }
  | { page: PlainPage }
  | { page: 'boards'; board: string | null }
  | { page: 'settings'; tab: SettingsTab | null; file?: string }
  | { page: 'task'; prefix: string };

/** The place an address names, or `null` for the front page and for an address that is nothing. */
export function parsePath(path: string): Place | null {
  const prefix = prefixFromPath(path);
  if (prefix) return { page: 'task', prefix };
  const board = boardFromPath(path);
  if (board !== null) return { page: 'boards', board };
  if (isBoardsPath(path)) return { page: 'boards', board: null };
  const report = /^\/report\/([^/]+)\/?$/.exec(path);
  if (report) {
    try {
      const name = decodeURIComponent(report[1]);
      if (name) return { page: 'tasks', report: name };
    } catch {
      return null; // a broken %-escape
    }
    return null;
  }
  const file = /^\/settings\/taskrc\/(.+?)\/?$/.exec(path);
  if (file) {
    const name = rcFileFromPath(file[1]);
    return name ? { page: 'settings', tab: 'taskrc', file: name } : null;
  }
  const settings = /^\/settings(?:\/([a-z]+))?\/?$/.exec(path);
  if (settings) {
    const tab = settings[1] as SettingsTab | undefined;
    if (!tab) return { page: 'settings', tab: null };
    return SETTINGS_TABS.includes(tab) ? { page: 'settings', tab } : null;
  }
  const plain = /^\/([a-z]+)\/?$/.exec(path)?.[1] as PlainPage | undefined;
  return plain && PLAIN_PAGES.includes(plain) ? { page: plain } : null;
}

/** The address of a place. */
export function placePath(p: Place): string {
  switch (p.page) {
    case 'tasks':
      return `/report/${encodeURIComponent(p.report)}`;
    case 'boards':
      return p.board === null ? BOARDS_PATH : boardPath(p.board);
    case 'settings':
      if (p.tab === 'taskrc' && p.file)
        return `/settings/taskrc/${p.file.split('/').map(encodeURIComponent).join('/')}`;
      return p.tab ? `/settings/${p.tab}` : '/settings';
    case 'task':
      return `/task/${p.prefix}`;
    default:
      return `/${p.page}`;
  }
}

// A task's address, `/task/<first 8 characters of its uuid>`, opens its detail drawer. (The first 8 characters are how a
// task is named everywhere else too; the numbers `task 3` uses differ from one replica to the next and change as tasks
// finish.)

/** The address that opens this task. */
export function taskPath(uuid: string): string {
  return `/task/${uuid.slice(0, 8)}`;
}

/** The uuid characters in an address like `/task/1a2b3c4d`, or `null` when the address is not a task's. */
export function prefixFromPath(path: string): string | null {
  const m = /^\/task\/([0-9a-f][0-9a-f-]{7,35})\/?$/i.exec(path);
  return m ? m[1].toLowerCase() : null;
}

/** Everything to put in a message: the address with the site in front. */
export function taskLink(uuid: string, origin: string): string {
  return `${origin}${taskPath(uuid)}`;
}

/**
 * The filter that finds a task by the start of its uuid. A bare `12345678` would be read as task numbers, so the
 * attribute form is used; it works for any prefix.
 */
export function prefixFilter(prefix: string): string {
  return `uuid.startswith:${prefix}`;
}

// A board has an address too, `/board/<slug>`, and the list of boards is `/boards`. A slug is one lower case word
// (`weekly`, `q4-review`), the same rule the engine keeps boards to (`tc_core::board::valid_id`).

/** The list of boards. */
export const BOARDS_PATH = '/boards';

/** The address that opens this board. */
export function boardPath(slug: string): string {
  return `/board/${slug}`;
}

/** The slug in an address like `/board/weekly`, or `null` when the address is not a board's. */
export function boardFromPath(path: string): string | null {
  const m = /^\/board\/([a-z0-9_-]{1,40})\/?$/.exec(path);
  return m ? m[1] : null;
}

/** Whether the address is the list of boards. */
export function isBoardsPath(path: string): boolean {
  return /^\/boards\/?$/.test(path);
}

// One file of the taskrc folder has an address too, `/settings/taskrc/<path>` (`/settings/taskrc/holidays/en-US.rc`),
// written in the folder's own way: parts joined by `/`, none empty, `.` or `..`.

function rcFileFromPath(raw: string): string | null {
  try {
    const parts = raw.split('/').map(decodeURIComponent);
    return parts.some((p) => p === '' || p === '.' || p === '..' || p.includes('\\')) ? null : parts.join('/');
  } catch {
    return null; // a broken %-escape
  }
}

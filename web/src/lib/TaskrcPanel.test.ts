// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { deleteRcFile, getRcFile, getRcFiles, getTaskrcText, importRcFiles, putRcFile } from './api';
import { store } from './store.svelte';
import TaskrcPanel from './TaskrcPanel.svelte';
import type { RcFiles, TaskrcResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  getTaskrcText: vi.fn(),
  getRcFile: vi.fn(),
  getRcFiles: vi.fn(),
  putRcFile: vi.fn(),
  deleteRcFile: vi.fn(),
  importRcFiles: vi.fn(),
  restoreTaskrc: vi.fn(),
  importTaskrcUrl: vi.fn(),
}));

const LIMITS = { files: 64, file_bytes: 262144, total_bytes: 1048576 };
const folder = (): RcFiles => ({
  root: 'taskrc',
  files: [
    {
      path: 'taskrc',
      bytes: 40,
      includes: [{ line: 2, target: 'holidays/en.rc', found: 'holidays/en.rc' }],
      used: true,
    },
    { path: 'holidays/en.rc', bytes: 30, includes: [], used: true },
    { path: 'lonely.rc', bytes: 5, includes: [], used: false },
  ],
  problems: [],
  limits: LIMITS,
});
const saved = (text: string, extra: Partial<TaskrcResponse> = {}): TaskrcResponse => ({
  udas: 0,
  reports: 0,
  contexts: 0,
  blocked: [],
  ignored: [],
  warnings: [],
  commented: [],
  text,
  ...extra,
});

const FILES: Record<string, string> = {
  taskrc: 'bulk=3\ninclude holidays/en.rc\n',
  'holidays/en.rc': 'holiday.towel.name=Towel day\nholiday.towel.date=20260525\n',
  'lonely.rc': 'bulk=1\n',
};

let app: ReturnType<typeof mount> | null = null;
const tick = async () => {
  for (let i = 0; i < 8; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

beforeEach(() => {
  store.settingsFile = null;
  store.config = { config: { settings: {} } } as never;
  vi.spyOn(store, 'settingsChanged').mockResolvedValue(undefined);
  vi.mocked(getRcFiles).mockImplementation(() => Promise.resolve(folder()));
  vi.mocked(getTaskrcText).mockImplementation(() => Promise.resolve(FILES.taskrc));
  vi.mocked(getRcFile).mockImplementation((p) => Promise.resolve(FILES[p] ?? ''));
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

const rows = () => [...document.querySelectorAll('nav li')].map((li) => li.textContent?.replace(/\s+/g, ' ').trim());
const box = () => document.querySelector('textarea') as HTMLTextAreaElement;
const button = (text: string | RegExp) =>
  [...document.querySelectorAll('button')].find((b) =>
    typeof text === 'string' ? b.textContent?.trim() === text : text.test(b.textContent ?? ''),
  ) as HTMLButtonElement;
const type = async (el: HTMLTextAreaElement | HTMLInputElement, value: string) => {
  el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
};

async function start() {
  app = mount(TaskrcPanel, { target: document.body });
  await tick();
}

describe('the list of files', () => {
  it('shows the main taskrc first, each folder once, and flags a file nothing includes', async () => {
    await start();
    expect(rows()).toEqual(['taskrc', 'holidays', 'en.rc', 'lonely.rc unused']);
    expect(box().value).toBe(FILES.taskrc);
    expect(document.querySelector('h3')?.textContent).toBe('taskrc');
  });

  it('opens a file, and keeps what was typed in another while it is away', async () => {
    await start();
    await type(box(), 'bulk=9\n');
    (document.querySelectorAll('.file-row')[2] as HTMLButtonElement).click();
    await tick();
    expect(document.querySelector('h3')?.textContent).toBe('lonely.rc');
    expect(box().value).toBe('bulk=1\n');
    expect(store.settingsFile).toBe('lonely.rc');
    expect(document.querySelectorAll('.file-row .dot')).toHaveLength(1);
    (document.querySelector('.file-row') as HTMLButtonElement).click();
    await tick();
    expect(box().value).toBe('bulk=9\n');
  });

  it('opens the file the address names', async () => {
    store.settingsFile = 'holidays/en.rc';
    await start();
    expect(document.querySelector('h3')?.textContent).toBe('holidays/en.rc');
    expect(box().value).toBe(FILES['holidays/en.rc']);
    expect(getRcFile).toHaveBeenCalledWith('holidays/en.rc');
  });

  it('follows the address when it changes', async () => {
    await start();
    store.settingsFile = 'lonely.rc';
    await tick();
    expect(document.querySelector('h3')?.textContent).toBe('lonely.rc');
  });
});

describe('applying a file', () => {
  it('saves that file, and shows what was kept, the lines commented out and the names removed', async () => {
    vi.mocked(putRcFile).mockResolvedValue(
      saved('bulk=9\n# [unsupported] nonsense=1\n', {
        blocked: ['sync.encryption_secret'],
        commented: [{ line: 2, message: 'not a setting this app reads' }],
      }),
    );
    await start();
    await type(box(), 'bulk=9\nnonsense=1\nsync.encryption_secret=hunter2\n');
    button('Apply').click();
    await tick();
    expect(putRcFile).toHaveBeenCalledWith('taskrc', 'bulk=9\nnonsense=1\nsync.encryption_secret=hunter2\n');
    // The box shows what is kept, not what was typed: the credential is gone from the page too.
    expect(box().value).toBe('bulk=9\n# [unsupported] nonsense=1\n');
    const shown = document.body.textContent ?? '';
    expect(shown).toContain('Removed for safety (1)');
    expect(shown).toContain('sync.encryption_secret');
    expect(shown).not.toContain('hunter2');
    expect(shown).toContain('1 line commented out');
    expect(shown).toContain('not a setting this app reads');
    expect(button('Apply').disabled).toBe(true);
    expect(store.settingsChanged).toHaveBeenCalled();
  });

  it('sends an include of a bundled theme as that theme, as before', async () => {
    vi.mocked(putRcFile).mockResolvedValue(saved('x'));
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('color.due=red\n'))),
    );
    await start();
    await type(box(), 'include dark-256.theme\n');
    button('Apply').click();
    await tick();
    vi.unstubAllGlobals();
    const sent = vi.mocked(putRcFile).mock.calls[0][1];
    expect(sent).not.toContain('include dark-256.theme');
    expect(sent).toContain('color.due=red');
  });
});

describe('including a file in another', () => {
  it('adds the include line, written from the other file, and shows that file', async () => {
    store.settingsFile = 'lonely.rc';
    await start();
    expect(button('Include')).toBeTruthy();
    expect(document.body.textContent).toContain('include lonely.rc');
    button('Include').click();
    await tick();
    expect(document.querySelector('h3')?.textContent).toBe('taskrc');
    expect(box().value).toBe('bulk=3\ninclude holidays/en.rc\ninclude lonely.rc\n');
    // Not saved until Apply.
    expect(putRcFile).not.toHaveBeenCalled();
    expect(button('Apply').disabled).toBe(false);
  });

  it('is not offered for the main taskrc', async () => {
    await start();
    expect(button('Include')).toBeUndefined();
  });
});

describe('deleting a file', () => {
  it('takes a second click, and goes back to the main taskrc', async () => {
    vi.mocked(deleteRcFile).mockResolvedValue(saved(''));
    store.settingsFile = 'lonely.rc';
    await start();
    button('Delete file').click();
    await tick();
    expect(deleteRcFile).not.toHaveBeenCalled();
    button('Click again to delete').click();
    await tick();
    expect(deleteRcFile).toHaveBeenCalledWith('lonely.rc');
    expect(document.querySelector('h3')?.textContent).toBe('taskrc');
    expect(store.settingsFile).toBeNull();
  });

  it('is not offered for the main taskrc', async () => {
    await start();
    expect(button(/Delete file/)).toBeUndefined();
  });
});

describe('uploading', () => {
  function pick(input: HTMLInputElement, files: [string, string, string?][]) {
    const list = files.map(([name, text, relative]) => {
      const f = new File([text], name);
      if (relative) Object.defineProperty(f, 'webkitRelativePath', { value: relative });
      return f;
    });
    Object.defineProperty(input, 'files', { value: list, configurable: true });
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }
  const folderInput = () => document.querySelector('input[webkitdirectory]') as HTMLInputElement;

  it('sends a picked folder with the folder name taken off, and what it left out', async () => {
    vi.mocked(importRcFiles).mockResolvedValue(saved(FILES.taskrc));
    await start();
    pick(folderInput(), [
      ['.taskrc', 'bulk=5\ninclude holidays/en.rc\n', 'dots/.taskrc'],
      ['en.rc', 'holiday.a.name=A\n', 'dots/holidays/en.rc'],
      ['HEAD', 'ref', 'dots/.git/HEAD'],
    ]);
    await tick();
    expect(importRcFiles).toHaveBeenCalledWith(
      [
        { path: 'taskrc', text: 'bulk=5\ninclude holidays/en.rc\n' },
        { path: 'holidays/en.rc', text: 'holiday.a.name=A\n' },
      ],
      'merge',
    );
    expect(document.body.textContent).toContain('dots/.git/HEAD (hidden)');
  });

  it('replaces the folder when asked to', async () => {
    vi.mocked(importRcFiles).mockResolvedValue(saved(FILES.taskrc));
    await start();
    const tickbox = [...document.querySelectorAll('input[type=checkbox]')].find((c) =>
      c.parentElement?.textContent?.includes('Replace what is here'),
    ) as HTMLInputElement;
    tickbox.click();
    await tick();
    pick(folderInput(), [['taskrc', 'bulk=5\n', 'dots/taskrc']]);
    await tick();
    expect(importRcFiles).toHaveBeenCalledWith([{ path: 'taskrc', text: 'bulk=5\n' }], 'replace');
  });

  it('says so when there is nothing to add', async () => {
    await start();
    pick(folderInput(), [['HEAD', 'ref', 'dots/.git/HEAD']]);
    await tick();
    expect(importRcFiles).not.toHaveBeenCalled();
    expect(document.querySelector('[role=alert]')?.textContent).toContain('no taskrc file to add');
  });
});

describe('adding a file', () => {
  it('makes it empty and shows it', async () => {
    vi.mocked(putRcFile).mockResolvedValue(saved(''));
    await start();
    await type(document.querySelector('#rc-new') as HTMLInputElement, 'holidays/de.rc');
    button('Add').click();
    await tick();
    expect(putRcFile).toHaveBeenCalledWith('holidays/de.rc', '');
    expect(document.querySelector('h3')?.textContent).toBe('holidays/de.rc');
  });
});

describe('adding holidays', () => {
  const US = '{"date":"2026-01-01","description":"New Year\'s Day","region":""}\n';
  const fetched = () =>
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) =>
        Promise.resolve(String(url).includes('/2026.json') ? new Response(US) : new Response('', { status: 404 })),
      ),
    );
  const pickCountry = async (code: string) => {
    const select = document.querySelector('.holidays select') as HTMLSelectElement;
    select.value = code;
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await tick();
  };

  it('saves the holiday file, includes it in the main taskrc and turns the calendar on', async () => {
    vi.mocked(putRcFile).mockImplementation((_p, t) => Promise.resolve(saved(t)));
    fetched();
    await start();
    await pickCountry('en-US');
    button('Find holidays').click();
    await tick();
    expect(document.body.textContent).toContain('1 holiday');
    button(/Save/).click();
    await tick();
    vi.unstubAllGlobals();
    const calls = vi.mocked(putRcFile).mock.calls;
    expect(calls[0][0]).toBe('holidays/en-US.rc');
    expect(calls[0][1]).toContain('holiday.en-US1.date=20260101');
    expect(calls[1]).toEqual([
      'taskrc',
      'bulk=3\ninclude holidays/en.rc\ninclude holidays/en-US.rc\ncalendar.holidays=sparse\n',
    ]);
    expect(document.querySelector('h3')?.textContent).toBe('holidays/en-US.rc');
  });

  it('puts the lines in the box, not over it, when the main taskrc has unapplied edits', async () => {
    vi.mocked(putRcFile).mockImplementation((_p, t) => Promise.resolve(saved(t)));
    fetched();
    await start();
    await type(box(), 'bulk=9\n');
    await pickCountry('en-US');
    button('Find holidays').click();
    await tick();
    button(/Save/).click();
    await tick();
    vi.unstubAllGlobals();
    expect(vi.mocked(putRcFile).mock.calls.map((c) => c[0])).toEqual(['holidays/en-US.rc']);
    (document.querySelector('.file-row') as HTMLButtonElement).click();
    await tick();
    expect(box().value).toBe('bulk=9\ninclude holidays/en-US.rc\ncalendar.holidays=sparse\n');
    expect(document.body.textContent).toContain('Apply there keeps them');
  });

  it('is not asked to include a file the taskrc already includes', async () => {
    fetched();
    await start();
    await pickCountry('en-US');
    // en-US is not included in the fixture; a locale whose file is can't be tested without one, so tick it off instead.
    button('Find holidays').click();
    await tick();
    vi.unstubAllGlobals();
    expect(document.body.textContent).toContain('Include it in');
  });
});

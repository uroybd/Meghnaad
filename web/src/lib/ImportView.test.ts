// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import ImportView from './ImportView.svelte';
import { importTasks } from './api';
import type { ImportReport } from './types';

vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), importTasks: vi.fn() }));

const report = (over: Partial<ImportReport> = {}): ImportReport => ({
  applied: false,
  added: 2,
  modified: 1,
  skipped: 4,
  lines: [
    { action: 'add', uuid: '11111111-1111-4111-8111-111111111111', description: 'First' },
    { action: 'mod', uuid: '22222222-2222-4222-8222-222222222222', description: 'Second' },
  ],
  more: 0,
  warnings: ['Warning: task has no description. (task 33333333)'],
  feedback: [],
  ...over,
});

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  vi.clearAllMocks();
});

const settle = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  flushSync();
};

async function choose(content: string, name = 'tasks.json', size?: number) {
  const input = document.querySelector<HTMLInputElement>('input[type=file]')!;
  const file = new File([content], name, { type: 'application/json' });
  if (size !== undefined) Object.defineProperty(file, 'size', { value: size });
  Object.defineProperty(input, 'files', { value: [file], configurable: true });
  input.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

describe('the import view', () => {
  it('checks a chosen file first and writes nothing until asked', async () => {
    const mocked = vi.mocked(importTasks);
    mocked.mockResolvedValueOnce(report());
    app = mount(ImportView, { target: document.body });
    await choose('[{"description":"First"}]');
    expect(mocked).toHaveBeenCalledWith('[{"description":"First"}]', false);
    const text = document.body.textContent ?? '';
    expect(text).toContain('2 new, 1 changed, 4 unchanged');
    expect(text).toContain('First');
    expect(text).toContain('no description');
    const button = [...document.querySelectorAll('button')].find((b) => /Import 3 tasks/.test(b.textContent ?? ''));
    expect(button, 'a button to import the 3 that would change').toBeTruthy();
    expect(mocked).toHaveBeenCalledTimes(1);

    mocked.mockResolvedValueOnce(report({ applied: true }));
    button!.click();
    await settle();
    expect(mocked).toHaveBeenLastCalledWith('[{"description":"First"}]', true);
    expect(document.body.textContent?.replace(/\s+/g, ' ')).toContain('Imported 2 new, 1 changed');
    expect([...document.querySelectorAll('button')].some((b) => /Import 3 tasks/.test(b.textContent ?? ''))).toBe(
      false,
    );
  });

  it('has nothing to offer when every task is already there', async () => {
    vi.mocked(importTasks).mockResolvedValueOnce(
      report({ added: 0, modified: 0, skipped: 5, lines: [], warnings: [] }),
    );
    app = mount(ImportView, { target: document.body });
    await choose('[]');
    expect(document.body.textContent).toContain('Nothing to import.');
    expect(document.querySelector('button.primary')).toBeNull();
  });

  it("shows the server's reason when a file is refused, and offers no import", async () => {
    vi.mocked(importTasks).mockRejectedValueOnce(new Error("Task 2: due: 'garbage' is not a valid date."));
    app = mount(ImportView, { target: document.body });
    await choose('[{}]');
    expect(document.querySelector('[role=alert]')?.textContent).toContain('Task 2: due');
    expect(document.querySelector('button.primary')).toBeNull();
  });

  it('turns a file that is too big away without sending it', async () => {
    app = mount(ImportView, { target: document.body });
    await choose('x', 'big.json', 5 * 1024 * 1024);
    expect(importTasks).not.toHaveBeenCalled();
    expect(document.querySelector('[role=alert]')?.textContent).toMatch(/5\.0 MB.*most one import takes is 3 MB/);
  });
});

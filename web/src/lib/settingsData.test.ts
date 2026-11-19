import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getRcFile, getRcFiles, getTaskrcText, importRcFiles, putRcFile } from './api';
import { FRESH_MS, settingsData } from './settingsData.svelte';
import type { RcFiles, TaskrcResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  getTaskrcText: vi.fn(),
  getRcFile: vi.fn(),
  getRcFiles: vi.fn(),
  putRcFile: vi.fn(),
  importRcFiles: vi.fn(),
}));

const answer = (text: string) => ({ text }) as TaskrcResponse;

beforeEach(() => {
  settingsData.reset();
  vi.mocked(getTaskrcText).mockResolvedValue('bulk=3\n');
  vi.mocked(getRcFile).mockImplementation((p) => Promise.resolve(`# ${p}\n`));
  vi.mocked(getRcFiles).mockResolvedValue({ files: [] } as unknown as RcFiles);
});

afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe('the settings screens read once', () => {
  it('callers at the same moment share one read, and a read is used again for a while', async () => {
    const [a, b] = await Promise.all([settingsData.file('taskrc').get(), settingsData.file('taskrc').get()]);
    expect(a).toBe('bulk=3\n');
    expect(b).toBe('bulk=3\n');
    await settingsData.file('taskrc').get();
    expect(getTaskrcText).toHaveBeenCalledTimes(1);
    expect(await settingsData.file('a.rc').get()).toBe('# a.rc\n');
    expect(getRcFile).toHaveBeenCalledWith('a.rc');
  });

  it('is read again once old, so a change from another device shows', async () => {
    vi.useFakeTimers();
    await settingsData.file('taskrc').get();
    vi.advanceTimersByTime(FRESH_MS + 1);
    await settingsData.file('taskrc').get();
    expect(getTaskrcText).toHaveBeenCalledTimes(2);
  });

  it('a save is taken as read; the list is read again', async () => {
    await settingsData.files.get();
    vi.mocked(putRcFile).mockResolvedValue(answer('bulk=9\n'));
    await settingsData.putRcFile('taskrc', 'bulk=9\n');
    expect(await settingsData.file('taskrc').get()).toBe('bulk=9\n');
    expect(getTaskrcText).not.toHaveBeenCalled();
    await settingsData.files.get();
    expect(getRcFiles).toHaveBeenCalledTimes(2);
  });

  it('an import makes the files it wrote stale, and a change elsewhere all but the ones just saved', async () => {
    await settingsData.file('taskrc').get();
    await settingsData.file('a.rc').get();
    vi.mocked(importRcFiles).mockResolvedValue(answer(''));
    await settingsData.importRcFiles([{ path: 'a.rc', text: 'x' }], 'merge');
    await settingsData.file('taskrc').get();
    await settingsData.file('a.rc').get();
    expect(getTaskrcText).toHaveBeenCalledTimes(1);
    expect(getRcFile).toHaveBeenCalledTimes(2);
    const rev = settingsData.rev;
    settingsData.changed(['a.rc']);
    expect(settingsData.rev).toBe(rev + 1);
    await settingsData.file('taskrc').get();
    await settingsData.file('a.rc').get();
    expect(getTaskrcText).toHaveBeenCalledTimes(2);
    expect(getRcFile).toHaveBeenCalledTimes(2);
  });

  it('a read made stale while on its way is read again next time', async () => {
    let finish: (t: string) => void = () => {};
    vi.mocked(getTaskrcText).mockReturnValueOnce(new Promise((r) => (finish = r)));
    const first = settingsData.file('taskrc').get();
    settingsData.changed();
    finish('old\n');
    expect(await first).toBe('old\n');
    expect(await settingsData.file('taskrc').get()).toBe('bulk=3\n');
    expect(getTaskrcText).toHaveBeenCalledTimes(2);
  });
});

// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CliResponse, CliResult, Row } from './types';

const runCli = vi.hoisted(() => vi.fn());
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli }));

import { fetchTask } from './lookup';
import { store } from './store.svelte';

const answer = (result: CliResult): CliResponse => ({ wrote: false, result, command: null });
const task = { uuid: 'u1', description: 'a task' } as unknown as Row;

beforeEach(() => {
  runCli.mockReset();
  store.rev++; // a fresh cache for each test
});

describe('one task by uuid', () => {
  it('asks once while the tasks stay the same', async () => {
    runCli.mockResolvedValue(answer({ kind: 'info', tasks: [task] } as CliResult));
    expect(await fetchTask('u1')).toBe(task);
    expect(await fetchTask('u1')).toBe(task);
    expect(runCli).toHaveBeenCalledTimes(1);
  });

  it('is null when the task is gone', async () => {
    runCli.mockResolvedValue(answer({ kind: 'info', tasks: [] } as unknown as CliResult));
    expect(await fetchTask('u1')).toBeNull();
  });

  it('says a request failed rather than that the task is gone, and asks again next time', async () => {
    runCli.mockRejectedValueOnce(new Error('could not reach the server'));
    await expect(fetchTask('u1')).rejects.toThrow('could not reach the server');
    runCli.mockResolvedValue(answer({ kind: 'info', tasks: [task] } as CliResult));
    expect(await fetchTask('u1')).toBe(task);
  });
});

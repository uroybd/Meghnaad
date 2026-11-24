// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CliResponse, CliResult } from './types';

const runCli = vi.hoisted(() => vi.fn());
vi.mock('./api', async (original) => ({ ...(await original<typeof import('./api')>()), runCli }));

import { parsePath, PLAIN_PAGES } from './route';
import { parseSettings } from './appsettings';
import { store } from './store.svelte';
import { offer, resetOffered, tools } from './webmcp';

const llms = readFileSync(resolve(__dirname, '../../public/llms.txt'), 'utf8');
const answer = (result: CliResult): CliResponse => ({ wrote: false, result, command: null });
const tool = (name: string) => tools().find((t) => t.name === name)!;

afterEach(() => {
  runCli.mockReset();
  resetOffered();
  delete (document as unknown as { modelContext?: unknown }).modelContext;
});

describe('llms.txt', () => {
  it('names every tool, and only those', () => {
    const named = [...llms.matchAll(/^- `([a-z_]+)`:/gm)].map((m) => m[1]);
    expect(named).toEqual(tools().map((t) => t.name));
  });

  it('gives only addresses the app opens, and every plain page', () => {
    const section = llms.split('## Addresses')[1].split('## ')[0];
    const addresses = [...section.matchAll(/`(\/[^`]*)`/g)].map((m) =>
      m[1]
        .replace('<first 8 characters of a uuid>', '1a2b3c4d')
        .replace('<tab>', 'appearance')
        .replace(/<[^>]+>/g, 'x'),
    );
    expect(addresses.length).toBeGreaterThan(8);
    for (const a of addresses) expect(parsePath(a), a).not.toBeNull();
    for (const p of PLAIN_PAGES) expect(addresses).toContain(`/${p}`);
  });
});

describe('the tools', () => {
  it('are offered once there is a modelContext, and only once', () => {
    expect(offer()).toBe(false);
    const registerTool = vi.fn();
    (document as unknown as { modelContext: unknown }).modelContext = { registerTool };
    expect(offer()).toBe(true);
    expect(offer()).toBe(true);
    expect(registerTool.mock.calls.map((c) => c[0].name)).toEqual(['export', 'places', 'dry_run', 'run', 'open']);
  });

  it('export sends the filter and keeps the fields asked for, with the uuid', async () => {
    runCli.mockResolvedValue(
      answer({ kind: 'json', value: [{ uuid: 'u1', id: 1, description: 'one', due: '20261124T000000Z', urgency: 2 }] }),
    );
    const got = await tool('export').execute({ filter: 'project:Home +errand', fields: ['description'] });
    expect(runCli).toHaveBeenCalledWith({ args: ['rc.verbose=nothing', 'project:Home', '+errand', 'export'] });
    expect(got).toEqual([{ uuid: 'u1', description: 'one' }]);
  });

  it('dry_run writes out task commands and runs nothing', async () => {
    store.appSettings = parseSettings(JSON.stringify({ taskCommands: [{ name: 'inbox', command: 'modify +inbox' }] }));
    expect(await tool('dry_run').execute({ line: '3 inbox && 3 start' })).toEqual([
      { line: '3 modify +inbox', join: null },
      { line: '3 start', join: '&&' },
    ]);
    expect(runCli).not.toHaveBeenCalled();
  });

  it('open refuses an address that is no page', async () => {
    expect(await tool('open').execute({ address: '/nowhere/at/all' })).toEqual({
      kind: 'error',
      message: 'No page at /nowhere/at/all.',
    });
  });
});

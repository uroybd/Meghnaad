// The real engine (the WebAssembly the page runs), completing an iterative task in a zone that is not UTC: the next
// date keeps its time of day across a change of the clocks, as `task` 3.6.0 gives it on a machine in that zone. In
// WebAssembly TaskChampion's own zone is UTC, so this is the engine's port doing it (`tc_core::iter`).
import { describe, expect, it } from 'vitest';
import { Engine } from 'tc-engine';

async function run(e: Engine, line: string, zone?: string): Promise<{ result: unknown; error?: string }> {
  const now = Date.now() / 1000;
  return JSON.parse(await e.run(JSON.stringify({ line, tz: -18000, zone, now })));
}

/** The due dates of the tasks `export` gives, by status. */
async function dues(e: Engine): Promise<Record<string, string>> {
  const out = (await run(e, 'export')) as { result: { text: string } };
  const tasks = JSON.parse(out.result.text) as { status: string; due: string }[];
  return Object.fromEntries(tasks.map((t) => [t.status, t.due]));
}

describe('an iterative task in the browser engine', () => {
  // 2027-03-10 00:00 in New York (UTC-5); the clocks go forward on the 14th.
  const due = 1_804_654_800;

  it('comes back at midnight a month on, in the zone named', async () => {
    const e = new Engine();
    expect(
      (await run(e, `add Rent iter:monthly iter_type:fixed due:${due}`, 'America/New_York')).error,
    ).toBeUndefined();
    await run(e, '1 done', 'America/New_York');
    // 2027-04-10 00:00 EDT (UTC-4).
    expect(await dues(e)).toEqual({ completed: '20270310T050000Z', iterative: '20270410T040000Z' });
  });

  it('keeps the offset when no zone is named', async () => {
    const e = new Engine();
    await run(e, `add Rent iter:monthly iter_type:fixed due:${due}`);
    await run(e, '1 done');
    expect((await dues(e)).iterative).toBe('20270410T050000Z');
  });
});

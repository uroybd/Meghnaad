// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getConfig, runCli } from './api';
import { review } from './review.svelte';
import { store } from './store.svelte';
import { REVIEW_FILTER, REVIEW_SORT } from './tasksh';
import type { ConfigResponse } from './types';

vi.mock('./api', async (original) => ({
  ...(await original<typeof import('./api')>()),
  runCli: vi.fn(),
  getConfig: vi.fn(),
}));

const QUIET = ['rc.confirmation:no', 'rc.verbose:nothing'];
const REPORT = ['rc.color=off', 'rc.detection=off', 'rc._forcecolor=off', 'rc.verbose=nothing', '_reviewed'];

function config(opts: { uda?: boolean; report?: boolean } = { uda: true, report: true }): ConfigResponse {
  const col = (spec: string) => ({ spec, name: spec, format: null, label: spec, kind: 'string' });
  return {
    config: { udas: opts.uda ? { reviewed: { name: 'reviewed', type: 'date', label: 'Reviewed' } } : {} },
    reports: opts.report
      ? [{ name: '_reviewed', description: 'x', columns: [col('uuid')], filter: REVIEW_FILTER, sort: REVIEW_SORT }]
      : [],
  } as unknown as ConfigResponse;
}

/** A pretend server: the `_reviewed` list, each task's `info`, and every write going through. */
function server(uuids: string[], opts: { gone?: string[]; failOn?: string } = {}) {
  const calls: string[][] = [];
  vi.mocked(runCli).mockImplementation(async ({ args = [] }) => {
    calls.push(args);
    const text = { result: { kind: 'text', lines: [] }, wrote: false, feedback: [] };
    if (args.includes('_reviewed'))
      return { result: { kind: 'report', rows: uuids.map((uuid) => ({ uuid })) }, wrote: false } as never;
    if (args.includes('_tags') || args.includes('_projects')) return text as never;
    if (args[1] === 'info') {
      const uuid = args[0];
      const tasks = opts.gone?.includes(uuid) ? [] : [{ uuid, id: 1, description: `task ${uuid}` }];
      return { result: { kind: 'info', tasks }, wrote: false } as never;
    }
    if (args[0] === 'rc.confirmation:no' && args[2] === 'config')
      return { result: { kind: 'text', lines: ['Config modified.'] }, wrote: false } as never;
    if (opts.failOn && args.includes(opts.failOn))
      return { result: { kind: 'error', message: 'Nope.' }, wrote: false } as never;
    return { result: { kind: 'changed', message: 'Modified 1 task.', tasks: [] }, wrote: true } as never;
  });
  /** The commands a review sent, without the housekeeping around them. */
  return {
    sent: () =>
      calls
        .filter((a) => a[0] === QUIET[0] && a[2] !== 'config')
        .map((a) => a.slice(3).join(' ').replace(/^/, `${a[2]} `)),
    calls,
  };
}

const said = () =>
  store.entries.map((e) => {
    const r = e.result;
    return e.title || (r?.kind === 'text' ? r.lines.join(' ') : r?.kind === 'error' ? r.message : '');
  });

beforeEach(() => {
  review.close();
  store.entries = [];
  store.editing = null;
  store.config = config();
  vi.mocked(getConfig).mockResolvedValue(config());
  vi.mocked(runCli).mockReset();
});

describe('a review in the console', () => {
  it('says so when no task needs review', async () => {
    server([]);
    await review.start({ surface: 'console' });
    expect(said()).toEqual(['There are no tasks needing review.']);
    expect(review.phase).toBe('idle');
  });

  it('goes through the list: Enter marks reviewed, s skips, c completes, and the end says how many', async () => {
    const s = server(['u1', 'u2', 'u3']);
    await review.start({ surface: 'console' });
    expect(vi.mocked(runCli).mock.calls[0][0].args).toEqual(REPORT);
    expect(review.phase).toBe('running');
    expect(review.captures).toBe(true);
    expect(said().at(-1)).toBe('review [1 of 3] task u1');

    await review.respond(''); // Enter
    expect(said().at(-1)).toBe('review [2 of 3] task u2');
    await review.respond('s');
    await review.respond('c');
    expect(s.sent()).toEqual(['u1 modify reviewed:now', 'u3 done']);
    expect(said().filter((x) => /^(Marked|Skipped|Completed|End)/.test(x))).toEqual([
      'Marked as reviewed.',
      'Skipped',
      'Completed.',
      'End of review. 2 out of 3 tasks reviewed.',
    ]);
    expect(review.phase).toBe('idle');
  });

  it('sends ordinary commands, quietly, for the task shown', async () => {
    server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.respond('r');
    const write = vi
      .mocked(runCli)
      .mock.calls.map(([i]) => i.args)
      .find((a) => a?.includes('modify'));
    expect(write).toEqual([...QUIET, 'u1', 'modify', 'reviewed:now']);
  });

  it('counts a deleted task as reviewed, and q stops', async () => {
    const s = server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.respond('d');
    await review.respond('q');
    expect(s.calls.some((a) => a.join(' ') === `${QUIET.join(' ')} u1 delete`)).toBe(true);
    expect(said().at(-1)).toBe('End of review. 1 out of 2 tasks reviewed.');
    expect(review.phase).toBe('idle');
  });

  it('stops after the number asked for', async () => {
    server(['u1', 'u2', 'u3', 'u4', 'u5']);
    await review.start({ limit: 2, surface: 'console' });
    expect(said().at(-1)).toBe('review [1 of 2] task u1');
    await review.respond('');
    await review.respond('');
    expect(said().at(-1)).toBe('End of review. 2 out of 2 tasks reviewed.');
  });

  it('changes a task with m, then shows the same one again', async () => {
    const s = server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.respond('m');
    expect(review.phase).toBe('modifying');
    expect(review.hint).toContain('Enter modification args');
    await review.respond('+tag project:X');
    expect(s.calls.some((a) => a.join(' ') === `${QUIET.join(' ')} u1 modify +tag project:X`)).toBe(true);
    expect(review.phase).toBe('running');
    expect(review.current).toBe(0);
    expect(said().filter((x) => x === 'review [1 of 2] task u1')).toHaveLength(2);
    // Nothing typed is no change.
    await review.respond('m');
    await review.respond('  ');
    expect(said().at(-1)).toBe('No changes.');
  });

  it('opens the editor for e, and when it is closed marks the task reviewed and shows it again', async () => {
    const s = server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.respond('e');
    expect(review.phase).toBe('editing');
    expect(store.editing?.row.uuid).toBe('u1');
    expect(review.captures).toBe(false);
    await review.editClosed();
    expect(s.calls.some((a) => a.join(' ') === `${QUIET.join(' ')} u1 modify reviewed:now`)).toBe(true);
    expect(review.phase).toBe('running');
    expect(review.current).toBe(0);
    expect(said().at(-1)).toBe('review [1 of 2] task u1');
  });

  it('does not know a word it was not told', async () => {
    const s = server(['u1']);
    await review.start({ surface: 'console' });
    const before = s.calls.length;
    await review.respond('zzz');
    expect(said().at(-1)).toBe("Command 'zzz' is not recognized.");
    expect(s.calls.length).toBe(before);
  });

  it('stays on the task, and says why, when a command is refused', async () => {
    server(['u1', 'u2'], { failOn: 'reviewed:now' });
    await review.start({ surface: 'console' });
    await review.respond('');
    expect(said().at(-1)).toBe('Nope.');
    expect(review.current).toBe(0);
    expect(review.reviewed).toBe(0);
    expect(review.phase).toBe('running');
  });

  it('passes over a task that is no longer there', async () => {
    server(['u1', 'u2'], { gone: ['u1'] });
    await review.start({ surface: 'console' });
    expect(said().at(-1)).toBe('review [2 of 2] task u2');
  });

  it('will not begin a second review while one is going', async () => {
    server(['u1']);
    await review.start({ surface: 'console' });
    await review.start({ surface: 'console' });
    expect(said().at(-1)).toBe('A review is already in progress.');
  });
});

describe('the setup', () => {
  beforeEach(() => {
    store.config = config({ uda: false, report: false });
  });

  it("asks first, in tasksh's words, and stops when the answer is no", async () => {
    const s = server(['u1']);
    await review.start({ surface: 'console' });
    expect(review.phase).toBe('setup');
    expect(said().at(-1)).toBe(
      "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed? (y/n)",
    );
    await review.respond('maybe');
    expect(said().at(-1)).toContain('(y/n)');
    await review.respond('n');
    expect(said().at(-1)).toBe('Review needs the reviewed UDA and the _reviewed report.');
    expect(s.calls.some((a) => a.includes('config'))).toBe(false);
    expect(review.phase).toBe('idle');
  });

  it('writes the UDA, then asks about the report and writes it with the columns first, then starts', async () => {
    const s = server(['u1']);
    vi.mocked(getConfig).mockResolvedValue(config());
    await review.start({ surface: 'console' });
    await review.respond('y');
    expect(said().at(-1)).toBe(
      "Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed? (y/n)",
    );
    await review.respond('yes');
    const written = s.calls.filter((a) => a[2] === 'config').map((a) => a.slice(3));
    expect(written.map((w) => w[0])).toEqual([
      'uda.reviewed.type',
      'uda.reviewed.label',
      'report._reviewed.columns',
      'report._reviewed.description',
      'report._reviewed.sort',
      'report._reviewed.filter',
    ]);
    // Only the tasksh names are touched, and the settings were read again before the list was asked for.
    for (const [name] of written) expect(name).toMatch(/^(uda\.reviewed\.|report\._reviewed\.)/);
    expect(getConfig).toHaveBeenCalled();
    expect(review.phase).toBe('running');
    expect(said().at(-1)).toBe('review [1 of 1] task u1');
  });

  it('says what is there when it is not the way tasksh needs it, before changing it', async () => {
    store.config = {
      config: { udas: { reviewed: { name: 'reviewed', type: 'string', label: null } } },
      reports: [{ name: '_reviewed', description: null, columns: [{ spec: 'id' }], filter: 'x', sort: null }],
    } as unknown as ConfigResponse;
    server(['u1']);
    await review.start({ surface: 'console' });
    expect(said().at(-1)).toContain("The 'reviewed' UDA is of type 'string' now");
    await review.respond('n');
  });
});

describe('a review in the card', () => {
  it('keeps its state for the card to draw, and the summary until the card is closed', async () => {
    server(['u1', 'u2']);
    await review.start({ surface: 'card' });
    expect(store.entries).toEqual([]);
    expect(review.captures).toBe(false);
    expect(review.row?.uuid).toBe('u1');
    expect([review.current, review.total]).toEqual([0, 2]);
    await review.act('reviewed');
    await review.act('skip');
    expect(review.phase).toBe('done');
    expect(review.message).toBe('End of review. 1 out of 2 tasks reviewed.');
    expect(store.entries).toEqual([]);
    review.close();
    expect(review.phase).toBe('idle');
  });

  it('is answered with buttons: the setup question, then the review', async () => {
    store.config = config({ uda: true, report: false });
    server(['u1']);
    await review.start({ surface: 'card' });
    expect(review.phase).toBe('setup');
    expect(review.question).toContain("'_reviewed' report");
    await review.answerSetup(true);
    expect(review.phase).toBe('running');
  });

  it('ignores a choice while another is being carried out', async () => {
    server(['u1', 'u2']);
    await review.start({ surface: 'card' });
    const first = review.act('reviewed');
    await review.act('reviewed'); // refused: the first is still running
    await first;
    expect(review.reviewed).toBe(1);
  });
});

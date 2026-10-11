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
const RECURRING = 'This is a recurring task.  Do you want to modify all pending recurrences of this same task?';

function config(opts: { uda?: boolean; report?: boolean } = { uda: true, report: true }): ConfigResponse {
  const col = (spec: string) => ({ spec, name: spec, format: null, label: spec, kind: 'string' });
  return {
    config: { udas: opts.uda ? { reviewed: { name: 'reviewed', type: 'date', label: 'Reviewed' } } : {} },
    reports: opts.report
      ? [{ name: '_reviewed', description: 'x', columns: [col('uuid')], filter: REVIEW_FILTER, sort: REVIEW_SORT }]
      : [],
  } as unknown as ConfigResponse;
}

type Ask = { ask: 'extras' | 'plain' | 'permission'; items?: { key: string; question: string }[]; message?: string };

/** A pretend server: the `_reviewed` list, each task's `info`, and every write going through (or asking first). */
function server(uuids: string[], opts: { gone?: string[]; failOn?: string; asks?: Ask } = {}) {
  const calls: { args: string[]; extras?: string[]; confirmed?: boolean; approved?: string[] }[] = [];
  vi.mocked(runCli).mockImplementation(async (input) => {
    const { args = [] } = input;
    calls.push({ args, extras: input.extras, confirmed: input.confirmed, approved: input.approved });
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
    const a = opts.asks;
    const answered =
      a && (a.ask === 'extras' ? input.extras !== undefined : a.ask === 'plain' ? input.confirmed : input.approved);
    if (a && !answered && args.some((w) => ['modify', 'done', 'delete'].includes(w))) {
      return {
        result: { kind: 'confirm', ask: a.ask, message: a.message ?? 'Sure?', items: a.items ?? [] },
        wrote: false,
      } as never;
    }
    return { result: { kind: 'changed', message: 'Modified 1 task.', tasks: [] }, wrote: true } as never;
  });
  /** The commands a review sent for a task, without the housekeeping around them. */
  return {
    sent: () =>
      calls.filter((c) => c.args[0] === QUIET[0] && c.args[2] !== 'config').map((c) => c.args.slice(2).join(' ')),
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

  it('goes through the list on key presses: Enter marks reviewed, s skips, c completes, and the end says how many', async () => {
    const s = server(['u1', 'u2', 'u3']);
    await review.start({ surface: 'console' });
    expect(vi.mocked(runCli).mock.calls[0][0].args).toEqual(REPORT);
    expect(review.phase).toBe('running');
    expect(review.captures).toBe(true);
    expect(said().at(-1)).toBe('review [1 of 3] task u1');

    await review.press('Enter');
    expect(said().at(-1)).toBe('review [2 of 3] task u2');
    await review.press('s');
    await review.press('c');
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
    await review.press('r');
    const write = vi
      .mocked(runCli)
      .mock.calls.map(([i]) => i.args)
      .find((a) => a?.includes('modify'));
    expect(write).toEqual([...QUIET, 'u1', 'modify', 'reviewed:now']);
  });

  it('counts a deleted task as reviewed, and q stops', async () => {
    const s = server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.press('d');
    await review.press('q');
    expect(s.sent()[0]).toBe('u1 delete');
    expect(said().at(-1)).toBe('End of review. 1 out of 2 tasks reviewed.');
    expect(review.phase).toBe('idle');
  });

  it('stops after the number asked for', async () => {
    server(['u1', 'u2', 'u3', 'u4', 'u5']);
    await review.start({ limit: 2, surface: 'console' });
    expect(said().at(-1)).toBe('review [1 of 2] task u1');
    await review.press('Enter');
    await review.press('Enter');
    expect(said().at(-1)).toBe('End of review. 2 out of 2 tasks reviewed.');
  });

  it('changes a task with m (the changes are typed), then shows the same one again', async () => {
    const s = server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.press('m');
    expect(review.phase).toBe('modifying');
    expect(review.hint).toContain('Enter modification args');
    await review.press('x'); // not a key here: what is typed goes to the field
    expect(review.phase).toBe('modifying');
    await review.respond('+tag project:X');
    expect(s.sent()).toContain('u1 modify +tag project:X');
    expect(review.phase).toBe('running');
    expect(review.current).toBe(0);
    expect(said().filter((x) => x === 'review [1 of 2] task u1')).toHaveLength(2);
    // Nothing typed is no change.
    await review.press('m');
    await review.respond('  ');
    expect(said().at(-1)).toBe('No changes.');
  });

  it('opens the editor for e, and when it is closed marks the task reviewed and shows it again', async () => {
    const s = server(['u1', 'u2']);
    await review.start({ surface: 'console' });
    await review.press('e');
    expect(review.phase).toBe('editing');
    expect(store.editing?.row.uuid).toBe('u1');
    expect(review.captures).toBe(false);
    await review.editClosed();
    expect(s.sent()).toContain('u1 modify reviewed:now');
    expect(review.phase).toBe('running');
    expect(review.current).toBe(0);
    expect(said().at(-1)).toBe('review [1 of 2] task u1');
  });

  it('ignores a key that means nothing, as keys are: no message, nothing sent', async () => {
    const s = server(['u1']);
    await review.start({ surface: 'console' });
    const before = s.calls.length;
    const lines = said().length;
    await review.press('z');
    await review.press('Tab');
    expect(s.calls.length).toBe(before);
    expect(said().length).toBe(lines);
  });

  it('stays on the task, and says why, when a command is refused', async () => {
    server(['u1', 'u2'], { failOn: 'reviewed:now' });
    await review.start({ surface: 'console' });
    await review.press('Enter');
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

describe('a recurring task', () => {
  const recurring: Ask = { ask: 'extras', items: [{ key: 'rec:u1', question: RECURRING }] };

  it('is asked about, as tasksh shows it, before it is changed: yes changes every pending recurrence', async () => {
    const s = server(['u1', 'u2'], { asks: recurring });
    await review.start({ surface: 'console' });
    await review.press('Enter');
    expect(review.phase).toBe('asking');
    expect(review.captures).toBe(true);
    expect(review.question).toBe(RECURRING);
    expect(said().at(-1)).toBe(`${RECURRING} (y/n)`);
    expect(review.current).toBe(0);

    await review.press('y');
    const resent = s.calls.filter((c) => c.args.includes('modify'));
    expect(resent).toHaveLength(2);
    expect(resent[1].extras).toEqual(['rec:u1']);
    expect(said().at(-1)).toBe('review [2 of 2] task u2');
    expect(review.reviewed).toBe(1);
  });

  it('with no, only this one is changed, and the review goes on', async () => {
    const s = server(['u1', 'u2'], { asks: recurring });
    await review.start({ surface: 'console' });
    await review.press('Enter');
    await review.press('n');
    const resent = s.calls.filter((c) => c.args.includes('modify'));
    expect(resent[1].extras).toEqual([]);
    expect(review.phase).toBe('running');
    expect(review.reviewed).toBe(1);
    expect(review.current).toBe(1);
  });

  it('is asked for done and delete as well, and another key than y or n waits', async () => {
    const s = server(['u1', 'u2'], { asks: recurring });
    await review.start({ surface: 'console' });
    await review.press('c');
    expect(review.phase).toBe('asking');
    await review.press('q'); // neither: still waiting
    await review.press('Enter');
    expect(review.phase).toBe('asking');
    await review.press('y');
    expect(s.calls.filter((c) => c.args.includes('done')).at(-1)!.extras).toEqual(['rec:u1']);
    expect(review.reviewed).toBe(1);
    await review.press('d');
    expect(review.phase).toBe('asking');
    await review.press('n');
    expect(s.calls.filter((c) => c.args.includes('delete')).at(-1)!.extras).toEqual([]);
  });

  it('asks each question of the step in turn', async () => {
    const two: Ask = {
      ask: 'extras',
      items: [
        { key: 'rec:u1', question: 'First?' },
        { key: 'dep:u9', question: 'Second?' },
      ],
    };
    const s = server(['u1'], { asks: two });
    await review.start({ surface: 'console' });
    await review.press('Enter');
    expect(review.question).toBe('First?');
    await review.press('y');
    expect(review.phase).toBe('asking');
    expect(review.question).toBe('Second?');
    await review.press('n');
    const last = s.calls.filter((c) => c.args.includes('modify')).at(-1)!;
    expect(last.extras).toEqual(['rec:u1']);
  });

  it('a plain question is answered with confirmed, and no leaves the task as it was', async () => {
    const s = server(['u1'], { asks: { ask: 'plain', message: 'Really?' } });
    await review.start({ surface: 'console' });
    await review.press('Enter');
    expect(review.question).toBe('Really?');
    await review.press('n');
    expect(said().at(-1)).toBe('Cancelled.');
    expect(review.phase).toBe('running');
    expect(review.reviewed).toBe(0);
    await review.press('Enter');
    await review.press('y');
    expect(s.calls.filter((c) => c.args.includes('modify')).at(-1)!.confirmed).toBe(true);
    expect(review.reviewed).toBe(1);
  });

  it('can be quit while it is being asked', async () => {
    server(['u1', 'u2'], { asks: recurring });
    await review.start({ surface: 'console' });
    await review.press('Enter');
    review.quit();
    expect(review.phase).toBe('idle');
  });
});

describe('the setup', () => {
  beforeEach(() => {
    store.config = config({ uda: false, report: false });
  });

  it("asks first, in tasksh's words, answered with y or n, and stops when the answer is no", async () => {
    const s = server(['u1']);
    await review.start({ surface: 'console' });
    expect(review.phase).toBe('setup');
    expect(said().at(-1)).toBe(
      "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed? (y/n)",
    );
    const lines = said().length;
    await review.press('x'); // neither
    expect(said().length).toBe(lines);
    await review.press('n');
    expect(said().at(-1)).toBe('Review needs the reviewed UDA and the _reviewed report.');
    expect(s.calls.some((c) => c.args.includes('config'))).toBe(false);
    expect(review.phase).toBe('idle');
  });

  it('writes the UDA, then asks about the report and writes it with the columns first, then starts', async () => {
    const s = server(['u1']);
    vi.mocked(getConfig).mockResolvedValue(config());
    await review.start({ surface: 'console' });
    await review.press('y');
    expect(said().at(-1)).toBe(
      "Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed? (y/n)",
    );
    await review.press('Y');
    const written = s.calls.filter((c) => c.args[2] === 'config').map((c) => c.args.slice(3));
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
    await review.press('n');
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

  it("asks Taskwarrior's question with Yes and No", async () => {
    server(['u1', 'u2'], { asks: { ask: 'extras', items: [{ key: 'rec:u1', question: RECURRING }] } });
    await review.start({ surface: 'card' });
    await review.act('reviewed');
    expect(review.phase).toBe('asking');
    expect(review.question).toBe(RECURRING);
    await review.answer(true);
    expect(review.phase).toBe('running');
    expect(review.current).toBe(1);
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

describe('moving a review to the card', () => {
  it('carries on in the card from the same place, and the console stops taking the keys', async () => {
    server(['u1', 'u2', 'u3']);
    await review.start({ surface: 'console' });
    await review.press('Enter');
    expect(review.captures).toBe(true);
    review.toCard();
    expect(review.surface).toBe('card');
    expect(review.captures).toBe(false);
    expect(review.phase).toBe('running');
    expect(review.row?.uuid).toBe('u2');
    expect([review.current, review.reviewed, review.total]).toEqual([1, 1, 3]);
    const lines = store.entries.length;
    await review.act('reviewed');
    expect(review.row?.uuid).toBe('u3');
    expect(store.entries.length).toBe(lines); // nothing more is printed in the console
  });

  it('does nothing when there is no review in the console to move', () => {
    review.toCard();
    expect(review.phase).toBe('idle');
    expect(review.surface).toBe('card');
  });
});

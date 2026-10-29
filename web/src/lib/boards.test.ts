import { describe, expect, it, vi } from 'vitest';
import {
  copyBoard,
  copySection,
  effectiveFilter,
  describeBy,
  describeMeasure,
  describeSpan,
  firstFree,
  formatValue,
  freshId,
  largestGroups,
  loadBoard,
  moveBy,
  moveSeries,
  newBoard,
  newSection,
  normalizeBoards,
  orderChart,
  orderGrids,
  describeRows,
  parseTasks,
  rangeLabel,
  viewed,
  stampEpoch,
  SPAN_PRESETS,
  seriesOf,
  place,
  reorder,
  resizeFlow,
  flowed,
  MIN_WIDTH,
  resizeBy,
  stacked,
  zoneNow,
  type AxisRange,
  type Board,
  type Chart,
  type Section,
} from './boards';
import { compactDuration } from './dates';
import type { CliResponse } from './types';

const sec = (id: string, x: number, y: number, w: number, h: number, extra: Partial<Section> = {}): Section => ({
  id,
  title: id,
  x,
  y,
  w,
  h,
  type: 'number',
  measure: { fn: 'count' },
  ...extra,
});

const board = (sections: Section[], extra: Partial<Board> = {}): Board => ({ id: 'b', name: 'B', sections, ...extra });
const at = (sections: Section[], id: string) => {
  const s = sections.find((x) => x.id === id)!;
  return { x: s.x, y: s.y, w: s.w, h: s.h };
};

describe('ids', () => {
  it('are one lower case word, made unique', () => {
    expect(freshId('Weekly Review!', [])).toBe('weekly-review');
    expect(freshId('Weekly Review!', ['weekly-review', 'weekly-review-2'])).toBe('weekly-review-3');
    expect(freshId('???', [])).toBe('board');
  });

  it('start a new board with a name and no sections', () => {
    expect(newBoard('  My week ', [])).toEqual({ id: 'my-week', name: 'My week', sections: [] });
  });
});

describe('the filter a section runs', () => {
  it('is the board’s and its own, each kept apart by parentheses', () => {
    const s = sec('a', 0, 0, 3, 2, { filter: '+a or +b' });
    expect(effectiveFilter(board([s], { filter: 'project:Home' }), s)).toBe('( project:Home ) ( +a or +b )');
    expect(effectiveFilter(board([s]), s)).toBe('( +a or +b )');
    // A section can leave the board's filter out, and is then asked about its own tasks only.
    const own = { ...s, ignoreBoardFilter: true };
    expect(effectiveFilter(board([own], { filter: 'project:Home' }), own)).toBe('( +a or +b )');
    expect(effectiveFilter(board([sec('c', 0, 0, 1, 1)], { filter: '  ' }), sec('c', 0, 0, 1, 1))).toBe('');
  });
});

describe('the zone', () => {
  it('starts the week on Monday only when the taskrc says so', () => {
    expect(zoneNow({ weekstart: 'Monday' }, 5).week_starts_monday).toBe(true);
    expect(zoneNow({ weekstart: 'sunday' }, 5).week_starts_monday).toBe(false);
    expect(zoneNow(undefined, 5)).toMatchObject({ week_starts_monday: false, now: 5 });
    // The engine reads whole seconds.
    expect(Number.isInteger(zoneNow(undefined).now)).toBe(true);
  });
});

describe('the grid', () => {
  const two = [sec('a', 0, 0, 6, 2), sec('b', 6, 0, 6, 2)];

  it('puts a moved section where it was dropped and pushes the one it lands on down', () => {
    const out = place(two, 'b', { x: 0, y: 0, w: 6, h: 2 });
    expect(at(out, 'b')).toEqual({ x: 0, y: 0, w: 6, h: 2 });
    expect(at(out, 'a').y).toBe(2);
  });

  it('moves and resizes by a step, and stops at the edges', () => {
    expect(at(moveBy(two, 'a', -1, -1), 'a')).toEqual({ x: 0, y: 0, w: 6, h: 2 });
    expect(at(moveBy(two, 'b', 1, 0), 'b').x).toBe(6);
    expect(at(resizeBy(two, 'a', 2, 1), 'a')).toMatchObject({ w: 8, h: 3 });
    expect(at(resizeBy(two, 'a', -9, -9), 'a')).toMatchObject({ w: MIN_WIDTH, h: 1 });
  });

  it('keeps everything else about a section as it was', () => {
    const out = moveBy([sec('a', 0, 0, 3, 2, { title: 'Open', filter: '+x' })], 'a', 1, 0);
    expect(out[0]).toMatchObject({ title: 'Open', filter: '+x', x: 1 });
  });

  it('reads top to bottom, left to right', () => {
    const out = stacked([sec('c', 0, 4, 12, 1), sec('b', 6, 0, 6, 2), sec('a', 0, 0, 6, 2)]);
    expect(out.map((s) => s.id)).toEqual(['a', 'b', 'c']);
  });

  it('puts a new section in the first free place', () => {
    expect(firstFree([], 6, 3)).toEqual({ x: 0, y: 0, w: 6, h: 3 });
    expect(firstFree([sec('a', 0, 0, 6, 3)], 6, 3)).toEqual({ x: 6, y: 0, w: 6, h: 3 });
    const b = board([sec('number', 0, 0, 3, 2)]);
    expect(newSection(b, 'number')).toMatchObject({ id: 'number-2', x: 3, y: 0, w: 3, h: 2 });
    expect(newSection(b, 'line').by).toEqual({ time: { field: 'entry', step: 'week' } });
    expect(newSection(b, 'table').report).toBe('next');
    // A heatmap is a column for each week and a row for each weekday, as strong as the tasks added then.
    const heat = newSection(b, 'heatmap');
    expect(heat).toMatchObject({
      by: { time: { field: 'entry', step: 'week' } },
      rows: { time: { field: 'entry', part: 'weekday' } },
    });
    expect(heat.series).toBeUndefined();
  });

  it('copies a section into the first free place, with an id and a title of its own', () => {
    const original = { ...sec('bar', 0, 0, 6, 5), title: 'Open', by: { field: 'project' }, series: [{ label: 'a' }] };
    const b = board([original]);
    const copy = copySection(b, original as never);
    expect(copy).toMatchObject({ id: 'bar-2', title: 'Open copy', x: 6, y: 0, w: 6, h: 5, by: { field: 'project' } });
    // Nothing is shared with the original: changing the copy leaves it alone.
    expect(copy.series).toEqual([{ label: 'a' }]);
    expect(copy.series).not.toBe(original.series);
    // A copy of a copy is `bar-3`, not `bar-2-2`.
    const again = copySection(board([original, copy]), copy);
    expect(again.id).toBe('bar-3');
  });

  it('copies a board with a name and an address of its own, and its sections as they are', () => {
    const original = { ...board([sec('a', 0, 0, 6, 2)]), id: 'week', name: 'My week', filter: 'project:Home' };
    const copy = copyBoard(original, [original]);
    expect(copy).toMatchObject({ id: 'my-week-copy', name: 'My week copy', filter: 'project:Home' });
    expect(copy.sections).toEqual(original.sections);
    expect(copy.sections).not.toBe(original.sections);
    expect(copyBoard(original, [original, copy]).id).toBe('my-week-copy-2');
  });

  it('is put in a new order, with the widths it had', () => {
    const three = [sec('a', 0, 0, 6, 2), sec('b', 6, 0, 3, 3), sec('c', 0, 3, 12, 1)];
    const out = reorder(three, 'b', -1);
    expect(stacked(out).map((s) => s.id)).toEqual(['b', 'a', 'c']);
    expect(out.map((s) => s.w)).toEqual([6, 3, 12]);
    // The first cannot go up, nor the last down.
    expect(reorder(three, 'a', -1)).toEqual(three);
    expect(reorder(three, 'c', 1)).toEqual(three);
  });

  it('flows onto the columns of a phone, several numbers to a row', () => {
    const row = [
      sec('a', 0, 0, 3, 2),
      sec('b', 3, 0, 3, 2),
      sec('c', 6, 0, 3, 2),
      sec('d', 9, 0, 3, 2),
      sec('e', 0, 2, 12, 4),
    ];
    expect(flowed(row, 4).map((s) => [s.id, s.x, s.y, s.w])).toEqual([
      ['a', 0, 0, 1],
      ['b', 1, 0, 1],
      ['c', 2, 0, 1],
      ['d', 3, 0, 1],
      ['e', 0, 2, 4],
    ]);
    // Six columns hold three of those, not four.
    expect(flowed(row, 6).find((s) => s.id === 'd')).toMatchObject({ x: 0, y: 2, w: 2 });
  });

  it('stacks tables and charts on a phone: half a row, or a whole one from half the grid', () => {
    const home = [
      sec('table', 0, 0, 6, 8, { type: 'table' }),
      sec('bar', 6, 0, 6, 4, { type: 'bar' }),
      sec('pie', 6, 4, 3, 4, { type: 'pie' }),
      sec('done', 9, 4, 3, 2),
    ];
    expect(flowed(home, 4).map((s) => [s.id, s.x, s.y, s.w])).toEqual([
      ['table', 0, 0, 4],
      ['bar', 0, 8, 4],
      ['pie', 0, 12, 2],
      ['done', 2, 12, 1],
    ]);
  });

  it('is resized in the phone’s columns and kept in the board’s twelve, never under two', () => {
    const row = [sec('a', 0, 0, 3, 2), sec('b', 3, 0, 3, 2)];
    expect(resizeFlow(row, 'a', 4, 1, 1).find((s) => s.id === 'a')).toMatchObject({ w: 6, h: 3 });
    expect(resizeFlow(row, 'a', 6, -5, 0).find((s) => s.id === 'a')!.w).toBe(MIN_WIDTH);
  });
});

describe('boards from a saved file', () => {
  it('are tidied by the engine, and what is wrong is named', () => {
    const n = normalizeBoards([
      { id: 'ok', name: 'OK', sections: [{ id: 's', type: 'number', x: 0, y: 5, w: 3, h: 2 }] },
      { id: 'Bad Id', name: 'x' },
    ]);
    expect(n.boards).toHaveLength(1);
    // Floated up to the first free row.
    expect(n.boards[0].sections[0]).toMatchObject({ y: 0, measure: { fn: 'count' } });
    expect(n.errors).toHaveLength(1);
    expect(normalizeBoards('nope').errors[0]).toMatch(/must be a list/);
  });
});

describe('the order of a chart', () => {
  const chart = (rows: [string, (number | null)[]][]): Chart => ({
    unit: 'number',
    labels: rows.map(([k]) => ({ key: k, label: k })),
    datasets: [0, 1].map((d) => ({
      label: `s${d}`,
      data: rows.map(([, v]) => v[d] ?? null),
      counts: rows.map(() => 1),
      total: null,
      tasks: 0,
    })),
  });
  const keys = (c: Chart, s: Section) => orderChart(c, s).labels.map((l) => l.key);
  const c = chart([
    ['b', [2, 1]],
    ['a', [5, 0]],
    ['c', [null, null]],
    ['d', [1, 1]],
  ]);
  const by = (extra: Partial<Section> = {}) => sec('x', 0, 0, 1, 1, { by: { field: 'p' }, ...extra });

  it('puts the largest total first, and what has nothing to measure last', () => {
    // b adds up to 3, a to 5, d to 2.
    expect(keys(c, by())).toEqual(['a', 'b', 'd', 'c']);
    expect(keys(c, by({ sort: 'value+' }))).toEqual(['d', 'b', 'a', 'c']);
  });

  it('sorts by name, and keeps the first few, in every series alike', () => {
    const out = orderChart(c, by({ sort: 'key-', limit: 2 }));
    expect(out.labels.map((l) => l.key)).toEqual(['d', 'c']);
    expect(out.datasets[0].data).toEqual([1, null]);
    expect(out.datasets[1].data).toEqual([1, null]);
    expect(out.datasets[1].counts).toEqual([1, 1]);
  });

  it('keeps the latest periods of a date in time order, and the first of any other order', () => {
    const t = sec('x', 0, 0, 1, 1, { by: { time: { field: 'due', step: 'day' } }, limit: 2 });
    expect(keys(c, t)).toEqual(['c', 'd']);
    expect(keys(c, { ...t, sort: 'key-' })).toEqual(['d', 'c']);
    expect(keys(c, { ...t, sort: 'key+' })).toEqual(['a', 'b']);
  });

  it('leaves a date in time order, unless asked', () => {
    const t = sec('x', 0, 0, 1, 1, { by: { time: { field: 'due', step: 'day' } } });
    expect(keys(c, t)).toEqual(['b', 'a', 'c', 'd']);
    expect(keys(c, { ...t, sort: 'value-' })[0]).toBe('a');
  });
});

describe('the series of a chart', () => {
  it('are the section’s own when it lists none, and named after it', () => {
    const s = sec('x', 0, 0, 1, 1, { title: 'Open', measure: { fn: 'sum', field: 'points' } });
    expect(seriesOf(s)).toEqual([{ label: 'Open', measure: { fn: 'sum', field: 'points' } }]);
    expect(seriesOf({ ...s, series: [{ label: 'a' }, { label: 'b' }] }).map((x) => x.label)).toEqual(['a', 'b']);
  });

  it('add their filter to the board’s and the section’s', () => {
    const s = sec('x', 0, 0, 1, 1, { filter: 'status:completed', series: [{ label: 'f', filter: 'outcome:failed' }] });
    expect(effectiveFilter(board([s], { filter: 'project:Home' }), s, s.series![0])).toBe(
      '( project:Home ) ( status:completed ) ( outcome:failed )',
    );
  });
});

describe('numbers on a screen', () => {
  it('are plain, durations are Taskwarrior’s, and nothing is a dash', () => {
    expect(formatValue(12, 'number', compactDuration)).toBe('12');
    expect(formatValue(3.14159, 'number', compactDuration)).toBe('3.14');
    expect(formatValue(7200, 'duration', compactDuration)).toBe('2h');
    expect(formatValue(null, 'number', compactDuration)).toBe('–');
  });
});

describe('loading a board', () => {
  const FILE = JSON.stringify([
    { project: 'Home', urgency: 2 },
    { project: 'Home', urgency: 4 },
    { project: 'Work', urgency: 9 },
  ]);
  const respond = (result: CliResponse['result']): CliResponse => ({ wrote: false, result, command: null });

  it('runs one export for sections with the same filter, and a table’s report on its own', async () => {
    const run = vi.fn(async ({ args }: { args: string[] }) => {
      if (args.at(-1) === 'export') return respond({ kind: 'file', name: 't.json', mime: '', text: FILE, count: 3 });
      return respond({ kind: 'text', lines: ['a table'] });
    });
    const b = board(
      [
        sec('n', 0, 0, 3, 2),
        sec('by', 3, 0, 9, 4, { type: 'bar', by: { field: 'project' } }),
        sec('sum', 0, 2, 3, 2, { measure: { fn: 'sum', field: 'urgency' } }),
        sec('t', 0, 4, 12, 3, { type: 'table', report: 'list' }),
        sec('own', 0, 7, 3, 2, { filter: 'project:Home' }),
      ],
      { filter: '+work' },
    );
    const data = await loadBoard(b, { tz: 0, week_starts_monday: true, now: 1_790_000_000 }, run);
    const exports = run.mock.calls.filter(([i]) => i.args.at(-1) === 'export').map(([i]) => i.args.join(' '));
    expect(exports).toEqual(['( +work ) export', '( +work ) ( project:Home ) export']);
    expect(run.mock.calls.some(([i]) => i.args.at(-1) === 'list')).toBe(true);

    expect(data.n).toMatchObject({ kind: 'answer', answer: { total: 3, tasks: 3 } });
    expect(data.sum).toMatchObject({ kind: 'answer', answer: { total: 15 } });
    expect(data.by).toMatchObject({
      kind: 'chart',
      chart: { labels: [{ label: 'Home' }, { label: 'Work' }], datasets: [{ label: 'by', data: [2, 1] }] },
    });
    expect(data.t).toMatchObject({ kind: 'table', result: { kind: 'text' } });
  });

  it('writes a date range into every filter that uses it, and says when a name is not one', async () => {
    const run = vi.fn(async ({ args }: { args: string[] }) =>
      args.at(-1) === 'export'
        ? respond({ kind: 'file', name: 't.json', mime: '', text: FILE, count: 3 })
        : respond({ kind: 'text', lines: ['a table'] }),
    );
    const b = board(
      [
        sec('n', 0, 0, 3, 2, { filter: 'end.after:$daterange.start and end.before:$daterange.end' }),
        sec('t', 0, 2, 12, 3, { type: 'table', report: 'list', filter: 'due.before:$daterange.end' }),
        sec('line', 3, 0, 9, 4, {
          type: 'line',
          by: { time: { field: 'end', step: 'day' } },
          series: [{ label: 'a', filter: 'scheduled.after:$daterange.start' }],
        }),
        sec('bad', 0, 5, 3, 2, { filter: 'due.after:$nope.start' }),
      ],
      { ranges: [{ name: 'daterange', start: 'sow', end: 'eow' }] },
    );
    // 2026-10-07 12:30 UTC, a Wednesday.
    const data = await loadBoard(b, { tz: 0, week_starts_monday: true, now: 1_791_376_200 }, run);
    const ran = run.mock.calls.map(([i]) => i.args.join(' '));
    expect(ran).toContain('( end.after:20261005T000000Z and end.before:20261011T235959Z ) export');
    expect(ran).toContain('( due.before:20261011T235959Z ) list');
    expect(ran).toContain('( scheduled.after:20261005T000000Z ) export');
    expect(ran.some((a) => a.includes('$'))).toBe(false);
    expect(data.n.kind).toBe('answer');
    expect(data.t).toMatchObject({ kind: 'table', filter: '( due.before:20261011T235959Z )' });
    expect(data.bad).toEqual({ kind: 'error', message: "this board has no date range called 'nope'" });
  });

  it('keeps as many groups as a date range covers, when Keep is its count', async () => {
    const run = vi.fn(async ({ args }: { args: string[] }) =>
      args.at(-1) === 'export'
        ? respond({ kind: 'file', name: 't.json', mime: '', text: FILE, count: 3 })
        : respond({ kind: 'text', lines: [] }),
    );
    const per = (id: string, limit: Section['limit']) =>
      sec(id, 0, 0, 6, 4, { type: 'bar', by: { time: { field: 'end', step: 'day' } }, limit });
    const b = board(
      [
        per('days', '$daterange.day_count'),
        per('hours', '$daterange.hour_count'),
        per('n', 5),
        per('bad', '$nope.day_count'),
      ],
      {
        ranges: [{ name: 'daterange', start: 'sow', end: 'eow' }],
      },
    );
    const data = await loadBoard(b, { tz: 0, week_starts_monday: true, now: 1_791_376_200 }, run);
    expect(data.days).toMatchObject({ kind: 'chart', limit: 7 });
    expect(data.hours).toMatchObject({ kind: 'chart', limit: 168 });
    expect(data.n).toMatchObject({ kind: 'chart', limit: 5 });
    expect(data.bad).toEqual({ kind: 'error', message: "this board has no date range called 'nope'" });
  });

  it('cuts a chart by the largest group of a range, which Keep may count too', async () => {
    const run = vi.fn(async ({ args }: { args: string[] }) =>
      args.at(-1) === 'export'
        ? respond({ kind: 'file', name: 't.json', mime: '', text: FILE, count: 3 })
        : respond({ kind: 'text', lines: [] }),
    );
    const per = (id: string, step: string) =>
      sec(id, 0, 0, 6, 4, {
        type: 'bar',
        by: { time: { field: 'end', step } },
        limit: '$daterange.lg_group.count',
      });
    const week = { tz: 0, week_starts_monday: true, now: 1_791_376_200 };
    const b = board(
      [per('auto', '$daterange.lg_group.name'), per('plain', 'week'), per('bad', '$nope.lg_group.name')],
      {
        ranges: [{ name: 'daterange', start: 'sow', end: 'eow' }],
      },
    );
    const data = await loadBoard(b, week, run);
    // A week is days, and 7 of them are kept; a plain step is as it was.
    expect(data.auto).toMatchObject({ kind: 'chart', limit: 7 });
    expect(data.plain).toMatchObject({ kind: 'chart', limit: 7 });
    expect(data.bad).toEqual({ kind: 'error', message: "this board has no date range called 'nope'" });
    // A month is weeks.
    const month = await loadBoard({ ...b, ranges: [{ name: 'daterange', start: 'som', end: 'eom' }] }, week, run);
    expect(month.auto).toMatchObject({ kind: 'chart', limit: 5 });
    expect(largestGroups(b.ranges!, week)).toEqual({ daterange: 'day' });
    expect(
      largestGroups(
        [
          { name: 'm', start: 'som', end: 'eom' },
          { name: 'x', start: 'later', end: 'eom' },
        ],
        week,
      ),
    ).toEqual({ m: 'week' });
  });

  it('shows a range by the dates typed on the page, and a name as a label', () => {
    const b = board([], { filter: 'a', ranges: [{ name: 'sprint_week', start: 'sow', end: 'eow' }] });
    expect(viewed(b, undefined, undefined)).toEqual(b);
    const v = viewed(b, 'b', { sprint_week: { start: 'som', end: 'eom' } });
    expect(v.filter).toBe('b');
    expect(v.ranges).toEqual([{ name: 'sprint_week', start: 'som', end: 'eom' }]);
    expect(rangeLabel('sprint_week')).toBe('Sprint week');
    expect(rangeLabel('daterange')).toBe('Daterange');
  });

  it('says what went wrong in one section without losing the others', async () => {
    const run = async ({ args }: { args: string[] }) =>
      args.includes('nope')
        ? respond({ kind: 'error', message: "'nope' is not a report" })
        : respond({ kind: 'file', name: 't.json', mime: '', text: FILE, count: 3 });
    const b = board([
      sec('ok', 0, 0, 3, 2),
      sec('bad', 3, 0, 3, 2, { type: 'table', report: 'nope' }),
      sec('worse', 6, 0, 3, 2, { measure: { fn: 'sum', field: 'project' } }),
    ]);
    const data = await loadBoard(b, { tz: 0, week_starts_monday: false, now: 0 }, run);
    expect(data.ok.kind).toBe('answer');
    expect(data.bad).toEqual({ kind: 'error', message: "'nope' is not a report" });
    expect(data.worse).toMatchObject({ kind: 'error', message: expect.stringContaining('not a number') });
  });
});

describe('a chart with series', () => {
  const respond = (result: CliResponse['result']): CliResponse => ({ wrote: false, result, command: null });
  const file = (rows: object[]) =>
    respond({ kind: 'file', name: 't.json', mime: '', text: JSON.stringify(rows), count: rows.length });

  it('runs a filter for each series and lays them on one axis', async () => {
    const run = vi.fn(async ({ args }: { args: string[] }) => {
      const text = args.join(' ');
      return text.includes('outcome:failed')
        ? file([{ end: '20261013T100000Z' }])
        : file([{ end: '20261005T100000Z' }, { end: '20261006T100000Z' }, { end: '20261020T100000Z' }]);
    });
    const done = sec('done', 0, 0, 8, 5, {
      type: 'line',
      stacked: true,
      filter: 'status:completed',
      by: { time: { field: 'end', step: 'week' } },
      series: [
        { label: 'Succeeded', filter: 'outcome:success or outcome:' },
        { label: 'Failed', filter: 'outcome:failed', color: '#e5534b' },
      ],
    });
    const data = await loadBoard(board([done]), { tz: 0, week_starts_monday: true, now: 1_790_000_000 }, run);
    expect(run.mock.calls.map(([i]) => i.args.join(' '))).toEqual([
      '( status:completed ) ( outcome:success or outcome: ) export',
      '( status:completed ) ( outcome:failed ) export',
    ]);
    expect(data.done.kind).toBe('chart');
    if (data.done.kind !== 'chart') return;
    expect(data.done.chart.labels.map((l) => l.label)).toEqual(['2026-10-05', '2026-10-12', '2026-10-19']);
    expect(data.done.chart.datasets.map((d) => [d.label, d.data])).toEqual([
      ['Succeeded', [2, 0, 1]],
      ['Failed', [0, 1, 0]],
    ]);
  });

  it('names the series that could not be read', async () => {
    const run = async ({ args }: { args: string[] }) =>
      args.join(' ').includes('nope')
        ? respond({ kind: 'error', message: 'bad filter' })
        : file([{ end: '20261005T100000Z' }]);
    const s = sec('c', 0, 0, 6, 4, {
      type: 'bar',
      by: { time: { field: 'end', step: 'day' } },
      series: [{ label: 'ok' }, { label: 'broken', filter: 'nope' }],
    });
    const data = await loadBoard(board([s]), { tz: 0, week_starts_monday: true, now: 0 }, run);
    expect(data.c).toEqual({ kind: 'error', message: 'broken: bad filter' });
  });
});

describe('a series of states', () => {
  const respond = (result: CliResponse['result']): CliResponse => ({ wrote: false, result, command: null });
  const file = (rows: object[]) =>
    respond({ kind: 'file', name: 't.json', mime: '', text: JSON.stringify(rows), count: rows.length });

  it('draws a burndown over a range of the board, with a place for every day of it up to today', async () => {
    const rows = [
      { entry: '20260930T100000Z', end: '20261006T100000Z' },
      { entry: '20261006T150000Z' },
      { entry: '20260101T100000Z', end: '20260102T100000Z' },
    ];
    const run = async () => file(rows);
    const [pending] = SPAN_PRESETS.map((p) => p.span);
    const over = (id: string, range: AxisRange) =>
      sec(id, 0, 0, 8, 5, {
        type: 'line',
        by: { time: { field: 'entry', step: 'day', range } },
        series: [{ label: 'Pending', span: pending }],
      });
    const b = board(
      [over('mine', 'week'), over('own', { start: '2026-10-05', end: '2026-10-06T23:59' }), over('nope', 'x')],
      {
        ranges: [{ name: 'week', start: 'sow', end: 'eow' }],
      },
    );
    // Wednesday 2026-10-07.
    const data = await loadBoard(b, { tz: 0, week_starts_monday: true, now: 1_791_376_200 }, run);
    if (data.mine.kind !== 'chart' || data.own.kind !== 'chart') throw new Error('not charts');
    expect(data.mine.chart.labels).toHaveLength(7);
    expect(data.mine.chart.datasets[0].data).toEqual([1, 1, 1, null, null, null, null]);
    expect(data.own.chart.labels.map((l) => l.label)).toEqual(['2026-10-05', '2026-10-06']);
    expect(data.nope).toMatchObject({ kind: 'error', message: expect.stringContaining("no date range called 'x'") });
    // The range typed on the page for the board moves the axis too.
    const moved = viewed(b, undefined, { week: { start: '2026-10-12', end: '2026-10-14T23:59' } });
    const next = await loadBoard(moved, { tz: 0, week_starts_monday: true, now: 1_791_376_200 }, run);
    if (next.mine.kind !== 'chart') throw new Error('not a chart');
    expect(next.mine.chart.labels.map((l) => l.label)).toEqual(['2026-10-12', '2026-10-13', '2026-10-14']);
  });

  it('lays a heatmap out as a grid for each series, the columns the same in each, and stacked as one', async () => {
    const rows = [
      { end: '20261005T090000Z', project: 'A' },
      { end: '20261005T150000Z', project: 'B' },
      { end: '20261006T090000Z', project: 'A' },
      { end: '20261012T090000Z', project: 'A' },
    ];
    const run = async () => file(rows);
    const heat = (id: string, extra: Partial<Section> = {}) =>
      sec(id, 0, 0, 8, 5, {
        type: 'heatmap',
        by: { time: { field: 'end', step: 'week' } },
        rows: { time: { field: 'end', part: 'weekday' } },
        ...extra,
      });
    const two = [
      { label: 'A', filter: 'project:A' },
      { label: 'B', filter: 'project:B' },
    ];
    const b = board([
      heat('one'),
      heat('two', { series: two }),
      heat('piled', { series: two, stacked: true }),
      heat('flat', { series: two, rows: undefined }),
      heat('line', { series: two, rows: undefined, stacked: true }),
    ]);
    const data = await loadBoard(b, { tz: 0, week_starts_monday: true, now: 1_791_376_200 }, run);
    if (data.one.kind !== 'chart' || data.two.kind !== 'chart' || data.piled.kind !== 'chart') throw new Error('no');
    // The columns are the weeks, the rows the days of the week from Monday.
    expect(data.one.grids).toHaveLength(1);
    expect(data.one.chart.labels.map((l) => l.label)).toEqual(['2026-10-05', '2026-10-12']);
    expect(data.one.chart.datasets.map((d) => d.label)).toEqual(['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']);
    expect(data.one.chart.datasets[0].data).toEqual([2, 1]);
    // A series is a grid of its own; stacked they are added into one.
    expect(data.two.grids?.map((g) => g.label)).toEqual(['A', 'B']);
    expect(data.piled.grids).toHaveLength(1);
    // (The test's tasks come the same for every filter, so the two series are each what the one was.)
    expect(data.piled.grids?.[0].datasets[0].data).toEqual([4, 2]);
    expect(data.two.grids?.[1].datasets[0].data).toEqual([2, 1]);
    // With no rows a series is a row of one grid, and stacked the series are one row.
    if (data.flat.kind !== 'chart' || data.line.kind !== 'chart') throw new Error('no');
    expect(data.flat.grids).toHaveLength(1);
    expect(data.flat.chart.datasets.map((d) => d.label)).toEqual(['A', 'B']);
    expect(data.line.chart.datasets.map((d) => d.label)).toEqual(['A / B']);
    // Three tasks in the first week and one in the second, in each of the two series.
    expect(data.line.chart.datasets[0].data).toEqual([6, 2]);
  });

  it('is sent to the engine with its span, and counts what is open at the end of each week', async () => {
    const rows = [
      { entry: '20261006T100000Z', start: '20261008T100000Z', end: '20261014T100000Z' },
      { entry: '20261013T100000Z' },
    ];
    const run = async () => file(rows);
    const [pending, started, done] = SPAN_PRESETS.map((p) => p.span);
    const burndown = sec('b', 0, 0, 8, 5, {
      type: 'bar',
      stacked: true,
      by: { time: { field: 'end', step: 'week' } },
      series: [
        { label: 'Pending', span: pending },
        { label: 'Started', span: started },
        { label: 'Done', span: done },
      ],
    });
    const now = Date.UTC(2026, 9, 15, 12) / 1000;
    const data = await loadBoard(board([burndown]), { tz: 0, week_starts_monday: true, now }, run);
    expect(data.b.kind).toBe('chart');
    if (data.b.kind !== 'chart') return;
    expect(data.b.chart.labels.map((l) => l.label)).toEqual(['2026-10-05', '2026-10-12']);
    expect(data.b.chart.datasets.map((d) => [d.label, d.data])).toEqual([
      ['Pending', [0, 1]],
      ['Started', [1, 0]],
      ['Done', [0, 1]],
    ]);
  });

  it('is named by Taskwarrior’s states, and written in words', () => {
    expect(SPAN_PRESETS.map((p) => p.id)).toEqual(['pending', 'started', 'done']);
    expect(describeSpan({ from: 'entry', to: 'end', unless: 'start' })).toBe(
      'open at the end of each period, from entry until end, unless start has passed',
    );
    expect(describeSpan({ from: 'end' })).toBe('open at the end of each period, from end');
  });

  it('is refused by the engine where there is no date to group by', async () => {
    const run = async () => file([{ entry: '20261006T100000Z' }]);
    const s = sec('b', 0, 0, 8, 5, {
      type: 'bar',
      by: { field: 'project' },
      series: [{ label: 'x', span: { from: 'entry' } }],
    });
    const data = await loadBoard(board([s]), { tz: 0, week_starts_monday: true, now: 0 }, run);
    expect(data.b).toMatchObject({ kind: 'error', message: expect.stringContaining('needs a date') });
  });
});

describe('what a card says about itself', () => {
  it('reads the tasks of an export, as an array or one to a line', () => {
    expect(parseTasks('[{"id":1},{"id":2}]')).toHaveLength(2);
    expect(parseTasks('{"id":1}\n{"id":2}\n')).toHaveLength(2);
    expect(parseTasks('')).toEqual([]);
    expect(parseTasks('[\n]\n')).toEqual([]);
    expect(parseTasks('{"id":1}\nnot json\n{"id":3}')).toHaveLength(2);
  });

  it('reads a date as export writes it, and nothing else as one', () => {
    expect(stampEpoch('20261007T123000Z')).toBe(Date.UTC(2026, 9, 7, 12, 30) / 1000);
    for (const v of ['2026-10-07', '20261007', 5, null, 'Home']) expect(stampEpoch(v)).toBeNull();
  });

  it('moves a series up or down the stack, and a series with no colour keeps the one its place gave it', () => {
    const a = { label: 'A' };
    const b = { label: 'B', color: 'red' };
    const c = { label: 'C' };
    const moved = moveSeries([a, b, c], 2, -1);
    expect(moved.map((x) => x.label)).toEqual(['A', 'C', 'B']);
    // C was third (the third default colour) and B is red; A, which did not move, takes the default of its place.
    expect(moved[1].color).toBe('green');
    expect(moved[2].color).toBe('red');
    expect(moved[0]).toBe(a);
    // The edges stay, and nothing is changed in what was given.
    expect(moveSeries([a, b], 0, -1)).toEqual([a, b]);
    expect(moveSeries([a, b], 1, 1)).toEqual([a, b]);
    expect(moveSeries([a, b], 5, -1)).toEqual([a, b]);
    expect(c).toEqual({ label: 'C' });
  });

  it('orders the columns of every grid of a heatmap together, so they line up', () => {
    const grid = (label: string, values: number[]): Chart => ({
      label,
      unit: 'number',
      labels: [
        { key: 'a', label: 'a' },
        { key: 'b', label: 'b' },
        { key: 'c', label: 'c' },
      ],
      datasets: [{ label: 'row', data: values, counts: values, total: null, tasks: 0 }],
    });
    const section = sec('h', 0, 0, 6, 4, { type: 'heatmap', by: { field: 'project' }, sort: 'value-' });
    // By what the grids add up to: b (9), then c (4), then a (3).
    const ordered = orderGrids([grid('x', [1, 5, 4]), grid('y', [2, 4, 0])], section, null);
    expect(ordered.map((g) => g.labels.map((l) => l.key))).toEqual([
      ['b', 'c', 'a'],
      ['b', 'c', 'a'],
    ]);
    expect(ordered[1].datasets[0].data).toEqual([4, 0, 2]);
    expect(orderGrids(ordered, section, 2)[0].labels).toHaveLength(2);
    expect(orderGrids([], section, null)).toEqual([]);
  });

  it('says what it measures and groups by', () => {
    expect(describeMeasure({ fn: 'count' })).toBe('Count of tasks');
    expect(describeMeasure({ fn: 'sum', field: 'points' })).toBe('Sum of points');
    expect(describeMeasure({ fn: 'max', field: 'urgency' })).toBe('Largest of urgency');
    expect(describeBy({ field: 'project' })).toBe('project');
    expect(describeBy({ time: { field: 'end', step: 'week' } })).toBe('end, per week');
    expect(describeBy({ time: { field: 'end', step: 'day', range: 'sprint_week' } })).toBe(
      'end, per day, over Sprint week',
    );
    expect(describeBy({ time: { field: 'end', step: 'day', range: { start: 'sow', end: 'eow' } } })).toBe(
      'end, per day, over sow to eow',
    );
    expect(describeBy(undefined)).toBeNull();
    expect(describeRows({ time: { field: 'end', part: 'weekday' } })).toBe('weekday of end');
    expect(describeRows({ time: { field: 'due', part: 'monthday' } })).toBe('day of the month of due');
    expect(describeRows({ field: 'project' })).toBe('the value of project');
    expect(describeRows(undefined)).toBeNull();
  });
});

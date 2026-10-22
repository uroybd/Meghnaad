// Boards: named pages of sections (a number, a table or a chart each) on a 12-column grid, kept in `settings.json` under
// `boards`. What a board file means (which sections are valid, where each sits, the numbers a section shows) is the
// engine's (`tc_core::board`, through src/engine/helpers.ts), which every front end shares; this file asks it and runs
// the commands that give it tasks. A section's data is `<filter> export`, Taskwarrior's own JSON, so nothing here reads
// the replica.

import { callHelper } from '../engine/helpers';
import { runCli, tzOffsetSeconds } from './api';
import { splitWords } from './cmdline';
import type { CliResponse, CliResult } from './types';

export const COLUMNS = 12;
/** The narrowest a section may be, in the 12 columns (one column of a phone's six or four). */
export const MIN_WIDTH = 2;
/** The height of one grid row, in pixels. */
export const ROW_PX = 56;
/** The gap between sections, in pixels. */
export const GAP_PX = 12;

export type SectionType = 'number' | 'table' | 'bar' | 'line' | 'area' | 'pie' | 'donut' | 'radar' | 'polar';
export type Fn = 'count' | 'sum' | 'avg' | 'min' | 'max';
export type Step = 'day' | 'week' | 'month' | 'quarter' | 'year';

export interface Measure {
  fn: Fn;
  /** What to add up or compare; `count` counts tasks and needs none. */
  field?: string;
}

/** One of the two: the value of an attribute, or a date cut into days, weeks, months, quarters or years. */
export interface By {
  field?: string;
  time?: { field: string; step: Step };
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * One series of a chart, as Chart.js has a dataset: a label, and what it is of. Its filter is added to the board's and the
 * section's, its measure is the section's when it has none, and its colour is the chart's own when it has none.
 */
export interface Series {
  label: string;
  filter?: string;
  measure?: Measure;
  /** A Taskwarrior colour name (`red`, `bright blue`), drawn in the page's palette, or `#rrggbb`. */
  color?: string;
  /** Count the tasks in a state as of the end of each period, not what happened in it. */
  span?: Span;
}

/**
 * A state as of the end of each period, as `task burndown` counts them: a task is in it while it has `from` and has not `to`
 * and has not `unless` yet.
 */
export interface Span {
  from: string;
  to?: string;
  unless?: string;
}

/** The states of `task burndown`, and how a span writes each. */
export const SPAN_PRESETS: { id: string; label: string; span: Span }[] = [
  { id: 'pending', label: 'Pending at the end of each period', span: { from: 'entry', to: 'end', unless: 'start' } },
  { id: 'started', label: 'Started at the end of each period', span: { from: 'start', to: 'end' } },
  { id: 'done', label: 'Done by the end of each period (adds up)', span: { from: 'end' } },
];

/** The span in words: "from entry until end, unless start has passed". */
export function describeSpan(span: Span): string {
  const to = span.to ? ` until ${span.to}` : '';
  const unless = span.unless ? `, unless ${span.unless} has passed` : '';
  return `open at the end of each period, from ${span.from}${to}${unless}`;
}

export interface Section extends Rect {
  id: string;
  title: string;
  type: SectionType;
  /** A table shows this report's columns. */
  report?: string;
  filter?: string;
  /** Ask about this section's own tasks only: the board's filter is not added. */
  ignoreBoardFilter?: boolean;
  measure: Measure;
  by?: By;
  /** A chart's series; none is one series, the section's own measure. */
  series?: Series[];
  /** The series are piled on each other (bars) or added up (lines), not side by side. */
  stacked?: boolean;
  /** `value-`, `value+`, `key+` or `key-`: how the groups of a chart are ordered. A date is in time order by default. */
  sort?: string;
  /** Keep this many groups, after sorting. */
  limit?: number;
}

/** A named pair of dates any filter on the board can use: `$name.start` and `$name.end`. Each is a date as Taskwarrior reads one. */
export interface DateRange {
  name: string;
  start: string;
  end: string;
}

/** A range worked out for now: the two moments (epoch seconds), or why it cannot be. */
export interface ResolvedRange {
  name: string;
  start?: number;
  end?: number;
  error?: string;
}

export interface Board {
  id: string;
  name: string;
  description?: string;
  /** Added to every section's filter. */
  filter?: string;
  ranges?: DateRange[];
  sections: Section[];
}

export interface Group {
  /** The value grouped by; for a date, the first second of the period. */
  key: string;
  label: string;
  /** Nothing to measure in the group, as opposed to zero. */
  value: number | null;
  /** The tasks in the group. */
  count: number;
}

export interface Answer {
  unit: 'number' | 'duration';
  total: number | null;
  tasks: number;
  groups: Group[];
}

/** A place on a chart's axis: a value of the attribute, or a period. */
export interface ChartLabel {
  key: string;
  label: string;
}

/** One series' value at each label (`null`: nothing to measure there) and the tasks behind each. */
export interface Dataset {
  label: string;
  data: (number | null)[];
  counts: number[];
  total: number | null;
  tasks: number;
}

/** What a chart draws, in Chart.js's shape: the labels along the axis and a dataset for each series. */
export interface Chart {
  unit: 'number' | 'duration';
  labels: ChartLabel[];
  datasets: Dataset[];
}

export interface Cell extends Rect {
  id: string;
  /** A table or a chart: a phone gives it half a row or a whole one, not its share of a quarter. */
  wide?: boolean;
}

export interface Normalized {
  boards: Board[];
  errors: string[];
}

export const SECTION_TYPES: { id: SectionType; label: string }[] = [
  { id: 'number', label: 'Number' },
  { id: 'bar', label: 'Bar chart' },
  { id: 'line', label: 'Line chart' },
  { id: 'area', label: 'Area chart' },
  { id: 'pie', label: 'Pie chart' },
  { id: 'donut', label: 'Donut chart' },
  { id: 'radar', label: 'Radar chart' },
  { id: 'polar', label: 'Polar area chart' },
  { id: 'table', label: 'Table' },
];

export const FNS: { id: Fn; label: string }[] = [
  { id: 'count', label: 'Count of tasks' },
  { id: 'sum', label: 'Sum of' },
  { id: 'avg', label: 'Average of' },
  { id: 'min', label: 'Smallest' },
  { id: 'max', label: 'Largest' },
];

export const STEPS: Step[] = ['day', 'week', 'month', 'quarter', 'year'];

/** The boards in a saved `boards` value: the valid ones, tidied, and a line for everything dropped. */
export function normalizeBoards(value: unknown): Normalized {
  return callHelper<Normalized>('board_normalize', value);
}

export const cellOf = (s: Rect & { id: string; type?: string }): Cell => ({
  id: s.id,
  x: s.x,
  y: s.y,
  w: s.w,
  h: s.h,
  ...(s.type && s.type !== 'number' ? { wide: true } : {}),
});
export const cellsOf = (sections: Section[]): Cell[] => sections.map(cellOf);

/** `sections` with the places in `cells` (matched by id). */
export function withCells(sections: Section[], cells: Cell[]): Section[] {
  const at = new Map(cells.map((c) => [c.id, c]));
  return sections.map((s) => {
    const c = at.get(s.id);
    return c ? { ...s, x: c.x, y: c.y, w: c.w, h: c.h } : s;
  });
}

/** `id` at `rect`, what it overlaps pushed down, everything floated up. */
export function place(sections: Section[], id: string, rect: Rect): Section[] {
  return withCells(sections, callHelper<Cell[]>('board_place', cellsOf(sections), id, rect));
}

/** One keyboard step: `id` moved by `dx` columns and `dy` rows. */
export function moveBy(sections: Section[], id: string, dx: number, dy: number): Section[] {
  return withCells(sections, callHelper<Cell[]>('board_move_by', cellsOf(sections), id, dx, dy));
}

/** One keyboard step: `id` wider by `dw` columns and taller by `dh` rows. */
export function resizeBy(sections: Section[], id: string, dw: number, dh: number): Section[] {
  return withCells(sections, callHelper<Cell[]>('board_resize_by', cellsOf(sections), id, dw, dh));
}

/** Everything floated up to the first free row. */
export function compact(sections: Section[]): Section[] {
  return withCells(sections, callHelper<Cell[]>('board_compact', cellsOf(sections)));
}

/** The ids in the order a narrow screen shows them: top to bottom, left to right. */
export function stackOrder(sections: Section[]): string[] {
  return callHelper<string[]>('board_stack', cellsOf(sections));
}

/** Where a new section of this size goes. */
export function firstFree(sections: Section[], w: number, h: number): Rect {
  return callHelper<Rect>('board_first_free', cellsOf(sections), w, h);
}

/** The sections in the order a narrow screen shows them. */
export function stacked(sections: Section[]): Section[] {
  const by = new Map(sections.map((s) => [s.id, s]));
  return stackOrder(sections).flatMap((id) => by.get(id) ?? []);
}

/**
 * `id` one place earlier (-1) or later (1) in the reading order. The sections keep their widths and are laid out again in the
 * new order, so it holds on the 12 columns as on a phone.
 */
export function reorder(sections: Section[], id: string, delta: -1 | 1): Section[] {
  return withCells(sections, callHelper<Cell[]>('board_reorder', cellsOf(sections), id, delta));
}

/** The columns a screen of this width (in pixels) has: 12, or 6, or 4 on a narrow one. */
export const columnsFor = (narrow: boolean, small: boolean) => (narrow ? (small ? 4 : 6) : COLUMNS);

/** The sections in reading order, placed on a screen of `cols` columns (`x` and `w` are that screen's columns). */
export function flowed(sections: Section[], cols: number): Section[] {
  const by = new Map(sections.map((s) => [s.id, s]));
  return callHelper<Cell[]>('board_flow', cellsOf(sections), cols).flatMap((c) => {
    const s = by.get(c.id);
    return s ? [{ ...s, x: c.x, y: c.y, w: c.w, h: c.h }] : [];
  });
}

/** One step of resizing `id` on a screen of `cols` columns: `dcols` of its columns wider, `dh` rows taller. */
export function resizeFlow(sections: Section[], id: string, cols: number, dcols: number, dh: number): Section[] {
  return withCells(sections, callHelper<Cell[]>('board_resize_flow', cellsOf(sections), id, cols, dcols, dh));
}

/** A board's slug is one lower case word: letters a to z, digits, `-` and `_`, up to 40 of them. */
export const SLUG = /^[a-z0-9_-]{1,40}$/;

/**
 * Why `slug` cannot be the address of the board being edited, or null. `others` are the slugs of the other boards, which it
 * must not be the same as.
 */
export function slugProblem(slug: string, others: Iterable<string>): string | null {
  if (!slug) return 'needs an address';
  if (!SLUG.test(slug)) return 'an address is lower case letters a to z, digits, - and _, up to 40';
  for (const o of others) if (o === slug) return `there is already a board at /board/${slug}`;
  return null;
}

/** An id built from `base` that is not in `taken`: `weekly`, `weekly-2`, … */
export function freshId(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  const stem =
    base
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 30) || 'board';
  if (!used.has(stem)) return stem;
  for (let n = 2; ; n++) if (!used.has(`${stem}-${n}`)) return `${stem}-${n}`;
}

const DEFAULT_SIZE: Record<SectionType, { w: number; h: number }> = {
  number: { w: 3, h: 2 },
  bar: { w: 6, h: 5 },
  line: { w: 6, h: 5 },
  area: { w: 6, h: 5 },
  pie: { w: 4, h: 5 },
  donut: { w: 4, h: 5 },
  radar: { w: 4, h: 5 },
  polar: { w: 4, h: 5 },
  table: { w: 12, h: 6 },
};

/** A new section of this type, in the first free place. */
export function newSection(board: Board, type: SectionType): Section {
  const { w, h } = DEFAULT_SIZE[type];
  const label = SECTION_TYPES.find((t) => t.id === type)?.label ?? type;
  const base: Section = {
    id: freshId(
      type,
      board.sections.map((s) => s.id),
    ),
    title: label,
    ...firstFree(board.sections, w, h),
    type,
    measure: { fn: 'count' },
  };
  if (type === 'line' || type === 'area') base.by = { time: { field: 'entry', step: 'week' } };
  else if (type !== 'number' && type !== 'table') base.by = { field: 'project' };
  if (type === 'table') base.report = 'next';
  return base;
}

export function newBoard(name: string, boards: Board[]): Board {
  return {
    id: freshId(
      name,
      boards.map((b) => b.id),
    ),
    name: name.trim(),
    sections: [],
  };
}

/**
 * The filter a section runs: the board's (unless the section ignores it), then its own, then a series', each in parentheses
 * so an `or` in one cannot leak into the next.
 */
export function effectiveFilter(board: Board, section: Section, series?: Series): string {
  return [section.ignoreBoardFilter ? undefined : board.filter, section.filter, series?.filter]
    .map((f) => f?.trim())
    .filter((f): f is string => !!f)
    .map((f) => `( ${f} )`)
    .join(' ');
}

/** A range's name as a label: `sprint_week` is `Sprint week`. The name itself is what a filter writes. */
export function rangeLabel(name: string): string {
  const words = name.replace(/_+/g, ' ').trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** What the board's date ranges are right now, each worked out in the page's zone and week. */
export function resolveRanges(ranges: DateRange[], zone: Zone): ResolvedRange[] {
  return ranges.length ? callHelper<ResolvedRange[]>('board_ranges', ranges, zone) : [];
}

/** `filter` with every `$name.start` and `$name.end` written as the moment it means; throws why it cannot be. */
export function substituteRanges(filter: string, ranges: DateRange[], zone: Zone): string {
  if (!filter.includes('$')) return filter;
  const got = callHelper<{ filter: string } | { message: string }>('board_substitute', filter, ranges, zone);
  if ('message' in got) throw new Error(got.message);
  return got.filter;
}

/** The board as it is looked at: with the filter and the range values typed on the page, where there are some. */
export function viewed(
  board: Board,
  filter: string | undefined,
  ranges: Record<string, Omit<DateRange, 'name'>> | undefined,
): Board {
  return {
    ...board,
    filter: filter ?? board.filter,
    ...(board.ranges ? { ranges: board.ranges.map((r) => ({ ...r, ...ranges?.[r.name] })) } : {}),
  };
}

export interface Zone {
  tz: number;
  week_starts_monday: boolean;
  now: number;
}

/** What the engine needs to cut dates into periods: the zone now in force, the first day of the week, and today. */
export function zoneNow(settings: Record<string, string> | undefined, now = Math.floor(Date.now() / 1000)): Zone {
  return {
    tz: tzOffsetSeconds(),
    // As the engine reads `weekstart`: Monday only if it says so.
    week_starts_monday: (settings?.weekstart ?? '').trim().toLowerCase() === 'monday',
    now,
  };
}

/** The numbers for a section from an `export` text. */
export function aggregate(text: string, section: Section, zone: Zone): Answer {
  const query: { measure: Measure; by?: By } = { measure: section.measure };
  if (section.by) query.by = section.by;
  return callHelper<Answer>('board_aggregate', text, query, zone);
}

/** Whether a type of section is drawn as a chart (not a figure or a table). */
export const isChartType = (t: SectionType): boolean => t !== 'number' && t !== 'table';

/** A pie, a donut or a polar chart has one series: its slices are the values. */
export const isRoundType = (t: SectionType): boolean => t === 'pie' || t === 'donut' || t === 'polar';

/** The series of a chart section: its own, or the one the section itself measures. */
export function seriesOf(section: Section): Series[] {
  return section.series?.length ? section.series : [{ label: section.title, measure: section.measure }];
}

/**
 * The chart with its labels in the order the section asks for (the largest total first for a value, time order for a
 * date), and as many as it keeps (for a date in time order, the latest). A label's total is what its series add up to, so a stacked bar is as tall as it is sorted.
 */
export function orderChart(chart: Chart, section: Section): Chart {
  const sort = section.sort ?? (section.by?.time ? null : 'value-');
  const total = (i: number) => chart.datasets.reduce((sum, d) => sum + (d.data[i] ?? 0), 0);
  const nothing = (i: number) => chart.datasets.every((d) => d.data[i] === null);
  let order = chart.labels.map((_, i) => i);
  if (sort) {
    const sign = sort.endsWith('-') ? -1 : 1;
    if (sort.startsWith('key')) order.sort((a, b) => sign * chart.labels[a].key.localeCompare(chart.labels[b].key));
    // A label with nothing to measure goes last, whichever way it is sorted.
    else
      order.sort((a, b) =>
        nothing(a) || nothing(b) ? Number(nothing(a)) - Number(nothing(b)) : sign * (total(a) - total(b)),
      );
  }
  if (section.limit && section.limit > 0) {
    // A date in time order keeps the latest periods (a week is `per day`, keep 7); any other order keeps its first.
    order = section.by?.time && !sort ? order.slice(-section.limit) : order.slice(0, section.limit);
  }
  return {
    unit: chart.unit,
    labels: order.map((i) => chart.labels[i]),
    datasets: chart.datasets.map((d) => ({
      ...d,
      data: order.map((i) => d.data[i]),
      counts: order.map((i) => d.counts[i]),
    })),
  };
}

/** What one filter got: the filter it ran, and the tasks it picked, as the text `export` printed them. */
export interface Source {
  /** The series (or, for a number, the section) it is for. */
  label: string;
  filter: string;
  text: string;
}

/** What a section got: its result, and (for the info panel) where the result came from. */
export type SectionData =
  | { kind: 'answer'; answer: Answer; sources: Source[] }
  | { kind: 'chart'; chart: Chart; sources: Source[] }
  | { kind: 'table'; result: CliResult; filter: string }
  | { kind: 'error'; message: string };

type Run = (input: { args: string[] }) => Promise<CliResponse>;

/**
 * Every section's data. Sections that run the same filter share one `export`; a table runs its report, as the Tasks
 * page does, with the section's filter.
 */
export async function loadBoard(board: Board, zone: Zone, run: Run = runCli): Promise<Record<string, SectionData>> {
  const exports = new Map<string, Promise<CliResult>>();
  /** The filter a section (or one of its series) runs, with the board's date ranges in it. */
  const filterOf = (s: Section, x?: Series) => substituteRanges(effectiveFilter(board, s, x), board.ranges ?? [], zone);
  const exportOf = (filter: string) => {
    let p = exports.get(filter);
    if (!p) exports.set(filter, (p = run({ args: [...splitWords(filter), 'export'] }).then((r) => r.result)));
    return p;
  };
  /** The tasks a filter picks, as the text `export` prints; or why they could not be had. */
  const tasksOf = async (filter: string): Promise<{ text: string } | { error: string }> => {
    const result = await exportOf(filter);
    if (result.kind === 'error') return { error: result.message };
    if (result.kind !== 'file') return { error: 'The tasks could not be read.' };
    return { text: result.text };
  };
  const one = async (s: Section): Promise<[string, SectionData]> => {
    try {
      if (s.type === 'table') {
        const filter = filterOf(s);
        const result = (await run({ args: [...splitWords(filter), s.report || 'next'] })).result;
        return [
          s.id,
          result.kind === 'error' ? { kind: 'error', message: result.message } : { kind: 'table', result, filter },
        ];
      }
      if (s.type === 'number') {
        const filter = filterOf(s);
        const got = await tasksOf(filter);
        if ('error' in got) return [s.id, { kind: 'error', message: got.error }];
        const sources = [{ label: s.title, filter, text: got.text }];
        return [s.id, { kind: 'answer', answer: aggregate(got.text, s, zone), sources }];
      }
      // A chart: each series runs its own filter (those that share one share its export), and they are laid on one axis.
      const inputs: { label: string; tasks: string; measure: Measure; span?: Span }[] = [];
      const sources: Source[] = [];
      for (const x of seriesOf(s)) {
        const filter = filterOf(s, x);
        const got = await tasksOf(filter);
        if ('error' in got) return [s.id, { kind: 'error', message: `${x.label}: ${got.error}` }];
        inputs.push({ label: x.label, tasks: got.text, measure: x.measure ?? s.measure, span: x.span });
        sources.push({ label: x.label, filter, text: got.text });
      }
      return [s.id, { kind: 'chart', chart: callHelper<Chart>('board_chart', inputs, s.by ?? null, zone), sources }];
    } catch (e) {
      return [s.id, { kind: 'error', message: e instanceof Error ? e.message : String(e) }];
    }
  };
  return Object.fromEntries(await Promise.all(board.sections.map(one)));
}

/** A value for a screen: a duration the way Taskwarrior writes one, a number without trailing noise. */
export function formatValue(
  value: number | null,
  unit: Answer['unit'],
  compactDuration: (s: number) => string,
): string {
  if (value === null) return '–';
  if (unit === 'duration') return compactDuration(value);
  return Number.isInteger(value) ? String(value) : String(Math.round(value * 100) / 100);
}

/** A line saying what a chart shows, for a screen reader. */
export function describeChart(title: string, chart: Chart, fmt: (v: number | null) => string): string {
  if (!chart.labels.length) return `${title}: nothing to show.`;
  const shown = chart.labels.slice(0, 5).map((l, i) => {
    const values = chart.datasets.map((d) =>
      chart.datasets.length > 1 ? `${d.label} ${fmt(d.data[i])}` : fmt(d.data[i]),
    );
    return `${l.label} ${values.join(', ')}`;
  });
  const series =
    chart.datasets.length > 1
      ? `, ${chart.datasets.length} series (${chart.datasets.map((d) => d.label).join(', ')})`
      : '';
  return `${title}: ${chart.labels.length} ${chart.labels.length === 1 ? 'place' : 'places'}${series}. ${shown.join('; ')}${chart.labels.length > 5 ? '; …' : ''}.`;
}

/** A task as `export` prints it. */
export type ExportedTask = Record<string, unknown>;

/** The tasks in an `export` text: a JSON array (`json.array` on), or one task per line. */
export function parseTasks(text: string): ExportedTask[] {
  const t = text.trim();
  if (!t) return [];
  try {
    const v: unknown = JSON.parse(t);
    return Array.isArray(v) ? (v as ExportedTask[]) : [v as ExportedTask];
  } catch {
    return t
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .flatMap((l) => {
        try {
          return [JSON.parse(l) as ExportedTask];
        } catch {
          return [];
        }
      });
  }
}

/** A date as `export` prints it (`20261007T123000Z`) in seconds since 1970, or null for anything else. */
export function stampEpoch(v: unknown): number | null {
  const m = typeof v === 'string' ? /^(\d{4})(\d\d)(\d\d)T(\d\d)(\d\d)(\d\d)Z$/.exec(v) : null;
  return m ? Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]) / 1000 : null;
}

const FN_WORDS: Record<Fn, string> = { count: 'Count', sum: 'Sum', avg: 'Average', min: 'Smallest', max: 'Largest' };

/** What a measure computes, in words: "Count of tasks", "Sum of points". */
export function describeMeasure(m: Measure): string {
  return m.fn === 'count' ? 'Count of tasks' : `${FN_WORDS[m.fn]} of ${m.field || '…'}`;
}

/** What a chart is grouped by, in words: "project", "end, per week"; null when it is not grouped. */
export function describeBy(by: By | undefined): string | null {
  if (by?.time) return `${by.time.field}, per ${by.time.step}`;
  return by?.field || null;
}

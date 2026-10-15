// Report cells: what a column spec like `due.relative` or `description.truncated` shows for a task is the engine's
// (`tc_core::cells`); what is here is the zone the browser is in, and the styling hints a row has.

import { callHelper } from '../engine/helpers';
import type { DateFmt } from './dateformat';
import type { Markers } from './journal';
import type { MdNode } from './markdown';
import type { Column, Row, UdaDef } from './types';

export interface Cell {
  text: string;
  /** The notes under the text (annotations): when each was made, its text, the moment that names it, and its Markdown. */
  notes?: { at: string; text: string; entry: number; blocks?: MdNode[] }[];
  /** The text as Markdown, in a string UDA that uses any. */
  blocks?: MdNode[];
  cls?: string;
  /** A project path as coloured parts, so the dots between them can be muted. */
  segments?: { text: string }[];
  /** Tags, each shown as a pill. */
  chips?: string[];
  /** The task's uuid, for the copy tooltip on an ID cell. */
  uuid?: string;
  /** Other tasks this cell names (their uuids), each drawn as a link to its card. */
  links?: string[];
}

export type UrgencyLevel = 'low' | 'mid' | 'high' | 'crit';

/**
 * How pressing a score is. Taskwarrior's `next` tag alone is worth 15 and a high priority 6, so
 * 15 and over means something is really on fire, and under 5 is background.
 */
export function urgencyLevel(u: number): UrgencyLevel {
  return u >= 15 ? 'crit' : u >= 10 ? 'high' : u >= 5 ? 'mid' : 'low';
}

export interface Ctx {
  /** Epoch seconds "now". */
  now: number;
  /** Viewer's UTC offset in seconds; omit to use the browser's. */
  tz?: number;
  udas?: Record<string, UdaDef>;
  /** Taskwarrior `dateformat*` patterns for table cells and for the notes under a description. */
  dates?: { report?: DateFmt; annotation?: DateFmt };
  /** `recurrence.indicator` (default R): what the `recur.indicator` column shows. */
  recurIndicator?: string;
  /** `active.indicator` (*), `tag.indicator` (+) and `dependency.indicator` (D). */
  indicators?: { active?: string; tag?: string; dependency?: string };
  /** `journal.time` marker texts: those annotations are bookkeeping and are left out of lists. */
  journal?: Markers | null;
}

export function shortUuid(u: string): string {
  return u.slice(0, 8);
}

/** A project path as its parts (`Home.Kitchen` is `Home` and `Kitchen`). */
export function projectSegments(path: string): { text: string }[] {
  return path.split('.').map((s) => ({ text: s }));
}

const DATE_PROPS = ['entry', 'start', 'end', 'due', 'wait', 'scheduled', 'until', 'modified'] as const;
const FORTNIGHT = 14 * 86400;

/** The browser's offset east of UTC at a moment (it changes with summer time). */
const browserOffset = (ts: number) => -new Date(ts * 1000).getTimezoneOffset() * 60;

/**
 * The offsets the browser's zone had over the moments in the rows, as `[from, offset]` pairs: the engine writes the dates
 * but cannot ask the browser which offset applied, so it is told. Samples a fortnight apart, and finds the exact moment of
 * a change by halving.
 */
function offsetsFor(rows: Row[], now: number): [number, number][] {
  let lo = now;
  let hi = now;
  const see = (t: number | null | undefined) => {
    if (t == null) return;
    lo = Math.min(lo, t);
    hi = Math.max(hi, t);
  };
  for (const r of rows) {
    for (const p of DATE_PROPS) see(r[p]);
    for (const a of r.annotations) see(a.entry);
  }
  const out: [number, number][] = [[lo, browserOffset(lo)]];
  for (let t = lo; t < hi;) {
    const next = Math.min(t + FORTNIGHT, hi);
    const here = out[out.length - 1][1];
    if (browserOffset(next) !== here) {
      let a = t;
      let b = next;
      while (b - a > 1) {
        const mid = Math.floor((a + b) / 2);
        if (browserOffset(mid) === here) a = mid;
        else b = mid;
      }
      out.push([b, browserOffset(b)]);
    }
    t = next;
  }
  return out;
}

/** The cells of a table, and which of its columns have nothing in them. */
export interface Table {
  /** `cells[row][column]` */
  cells: Cell[][];
  /** Per column: nothing in it for any row. Without `print.empty.columns` a table leaves such a column out. */
  empty: boolean[];
}

/**
 * What every cell of a table shows, and which columns are empty: the engine works them out (`tc_core::cells`), all in one
 * call. Without a `tz` the browser's zone applies, at each moment's own offset.
 */
export function formatTable(columns: Column[], rows: Row[], ctx: Ctx): Table {
  if (!rows.length || !columns.length) return { cells: rows.map(() => []), empty: columns.map(() => true) };
  const udas = Object.fromEntries(
    Object.entries(ctx.udas ?? {}).map(([k, u]) => [k, { type: u.type, indicator: u.indicator }]),
  );
  const wire = {
    now: ctx.now,
    tz: ctx.tz ?? browserOffset(ctx.now),
    offsets: ctx.tz == null ? offsetsFor(rows, ctx.now) : [],
    udas,
    dates: ctx.dates,
    recur_indicator: ctx.recurIndicator,
    indicators: ctx.indicators,
    journal: ctx.journal,
  };
  const cols = columns.map(({ name, format, kind }) => ({ name, format, kind }));
  type Wire = Omit<Cell, 'segments'> & { segments?: string[] };
  const table = callHelper<{ cells: Wire[][]; empty: boolean[] }>('format_cells', cols, rows, wire);
  return {
    empty: table.empty,
    cells: table.cells.map((r) =>
      r.map((c) => (c.segments ? { ...c, segments: c.segments.map((text) => ({ text })) } : (c as Cell))),
    ),
  };
}

/** Just the cells of a table (see [formatTable]). */
export function formatCells(columns: Column[], rows: Row[], ctx: Ctx): Cell[][] {
  return formatTable(columns, rows, ctx).cells;
}

/** CSS classes for a whole row. */
export function rowClass(row: Row): string {
  const c: string[] = [];
  if (row.status === 'completed' || row.status === 'deleted') c.push('done', row.status);
  if (row.waiting) c.push('waiting');
  if (row.blocked) c.push('blocked');
  if (row.start != null && row.status === 'pending') c.push('active');
  return c.join(' ');
}

/** A UDA's display label, falling back to its name. */
export function udaLabel(def: UdaDef): string {
  return def.label || def.name;
}

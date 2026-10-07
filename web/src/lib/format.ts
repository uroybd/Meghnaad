// Renders a report cell from a column spec like `due.relative` or `description.truncated`,
// following Taskwarrior's column formats where they make sense on the web.

import { compactDuration, formatMoment } from './dates';
import type { DateFmt } from './dateformat';
import { visibleNotes, type Markers } from './journal';
import type { Column, Row, UdaDef } from './types';

export interface Cell {
  text: string;
  /** Extra lines under the text (annotations). */
  lines?: string[];
  cls?: string;
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
  /** `journal.time` marker texts: those annotations are bookkeeping and are left out of lists. */
  journal?: Markers | null;
}

const DATE_PROPS = ['entry', 'start', 'end', 'due', 'wait', 'scheduled', 'until', 'modified'] as const;
type DateProp = (typeof DATE_PROPS)[number];

export function shortUuid(u: string): string {
  return u.slice(0, 8);
}

function dateCell(ts: number | null, name: string, format: string | null, ctx: Ctx): Cell {
  if (name === 'start' && format === 'active') return { text: ts == null ? '' : '*' };
  if (ts == null) return { text: '' };
  switch (format) {
    case 'relative':
      return { text: compactDuration(ts - ctx.now) };
    case 'countdown':
      return { text: compactDuration(ts - ctx.now) };
    case 'remaining':
      return { text: ts > ctx.now ? compactDuration(ts - ctx.now) : '' };
    case 'age':
      return { text: compactDuration(ctx.now - ts) };
    case 'epoch':
      return { text: String(ts) };
    case 'iso':
      return { text: new Date(ts * 1000).toISOString().replace(/\.\d{3}Z$/, 'Z') };
    case 'julian':
      return { text: (ts / 86400 + 2440587.5).toFixed(5) };
    default:
      return { text: formatMoment(ts, ctx.tz, ctx.dates?.report) };
  }
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, Math.max(0, n - 1)) + '…';
}

function descriptionCell(row: Row, format: string | null, ctx: Ctx): Cell {
  const shown = visibleNotes(row.annotations, ctx.journal);
  const notes = shown.map((a) => `${formatMoment(a.entry, ctx.tz, ctx.dates?.annotation)} ${a.text}`);
  switch (format) {
    case 'desc':
      return { text: row.description };
    case 'oneline':
      return { text: [row.description, ...shown.map((a) => a.text)].join(' ') };
    case 'truncated':
      return { text: truncate(row.description, 40) };
    case 'truncated_count':
      return { text: truncate(row.description, 40) + (notes.length ? ` [${notes.length}]` : '') };
    case 'count':
      return { text: row.description + (notes.length ? ` [${notes.length}]` : '') };
    default:
      // Taskwarrior's default shows annotations beneath the description.
      return { text: row.description, lines: notes.length ? notes : undefined };
  }
}

function projectCell(p: string | null, format: string | null): Cell {
  if (!p) return { text: '' };
  const parts = p.split('.');
  if (format === 'parent') return { text: parts.length > 1 ? parts.slice(0, -1).join('.') : p };
  if (format === 'indented') return { text: '  '.repeat(parts.length - 1) + parts[parts.length - 1] };
  return { text: p };
}

/** UDA values for date-typed UDAs are stored as epoch seconds. */
function udaCell(row: Row, col: Column, ctx: Ctx): Cell {
  const raw = row.extra[col.name];
  if (raw == null) return { text: '' };
  const def = ctx.udas?.[col.name];
  if (def?.type === 'date' || col.kind === 'date') {
    const ts = Number(raw);
    return Number.isFinite(ts) ? dateCell(ts, col.name, col.format, ctx) : { text: raw };
  }
  if (def?.indicator && col.format === 'indicator') return { text: def.indicator };
  return { text: raw };
}

export function cell(col: Column, row: Row, ctx: Ctx): Cell {
  const f = col.format;
  const name = col.name;

  if ((DATE_PROPS as readonly string[]).includes(name)) {
    const c = dateCell(row[name as DateProp], name, f, ctx);
    if (name === 'due' && row.virtual_tags.includes('OVERDUE')) c.cls = 'overdue';
    return c;
  }

  switch (name) {
    case 'id':
      return row.id != null ? { text: String(row.id) } : { text: shortUuid(row.uuid), cls: 'dim' };
    case 'uuid':
      return { text: f === 'short' ? shortUuid(row.uuid) : row.uuid };
    case 'status': {
      const waiting = row.virtual_tags.includes('WAITING');
      if (f === 'short') return { text: waiting ? 'W' : (row.status[0] ?? '').toUpperCase() };
      return { text: waiting ? 'waiting' : row.status };
    }
    case 'description':
      return descriptionCell(row, f, ctx);
    case 'project':
      return projectCell(row.project, f);
    case 'priority':
      return { text: row.priority ?? '', cls: row.priority ? `pri-${row.priority.toLowerCase()}` : undefined };
    case 'tags': {
      if (f === 'count') return { text: row.tags.length ? `[${row.tags.length}]` : '' };
      if (f === 'indicator') return { text: row.tags.length ? '+' : '' };
      return { text: row.tags.join(f === 'list' ? ',' : ' ') };
    }
    case 'depends': {
      if (f === 'indicator') return { text: row.depends.length ? 'D' : '' };
      if (f === 'count') return { text: row.depends.length ? `[${row.depends.length}]` : '' };
      return { text: row.depends.map(shortUuid).join(' ') };
    }
    case 'urgency':
      return { text: f === 'integer' ? String(Math.round(row.urgency)) : row.urgency.toFixed(2) };
    case 'recur':
      return { text: f === 'indicator' ? (row.recur ? (ctx.recurIndicator ?? 'R') : '') : (row.recur ?? '') };
    case 'parent':
      return { text: row.parent ? shortUuid(row.parent) : '' };
    case 'annotations':
      return { text: f === 'count' ? String(row.annotations.length || '') : row.annotations.map((a) => a.text).join('; ') };
    default:
      // A UDA, an orphan, or something Taskwarrior has that we don't render specially.
      return udaCell(row, col, ctx);
  }
}

/** CSS classes for a whole row. */
export function rowClass(row: Row): string {
  const c: string[] = [];
  if (row.status === 'completed' || row.status === 'deleted') c.push('done');
  if (row.virtual_tags.includes('WAITING')) c.push('waiting');
  if (row.blocked) c.push('blocked');
  if (row.start != null && row.status === 'pending') c.push('active');
  return c.join(' ');
}

/** A UDA's display label, falling back to its name. */
export function udaLabel(def: UdaDef): string {
  return def.label || def.name;
}

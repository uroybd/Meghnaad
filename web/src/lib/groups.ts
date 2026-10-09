// Group headers for a report whose sort has a `/` (`outcome+/`, `project+/`): Taskwarrior leaves a blank line
// where the group changes; the web draws a header above each group saying what its tasks have in common.
import { cell, udaLabel, type Ctx } from './format';
import { parseSort } from './sortSpec';
import type { Column, ReportResult, Row } from './types';

const NONE = '(none)';

const capitalise = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** The column to read a sort key's value with: the report's own when it shows it, else a plain one. */
function columnFor(result: ReportResult, name: string, ctx: Ctx): Column {
  const shown = result.columns.find((c) => c.name === name);
  if (shown) return shown;
  const uda = ctx.udas?.[name];
  return { spec: name, name, format: null, kind: 'string', label: uda ? udaLabel(uda) : capitalise(name) };
}

/**
 * One entry per row: the text of the header that goes above it, or null when it continues the group above.
 * Empty when the report's sort has no `/`. A header reads `Outcome: fail`, and `Outcome: fail · Proj: Y` when
 * several columns break the table.
 */
export function groupHeads(result: ReportResult, ctx: Ctx): (string | null)[] {
  const keys = parseSort(result.sort).filter((k) => k.brk);
  if (keys.length === 0) return [];
  const cols = keys.map((k) => columnFor(result, k.column, ctx));
  const head = (row: Row) =>
    cols
      .map((c) => {
        const value = cell(c, row, ctx).text.trim();
        return `${c.label}: ${value || NONE}`;
      })
      .join(' · ');
  return result.rows.map((row, i) => (i === 0 || result.breaks[i] ? head(row) : null));
}

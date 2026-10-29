// The chart of a board section as Chart.js draws it. The engine's `Chart` (labels, and a dataset of values for each
// series, `tc_core::board::aggregate::chart`) already has Chart.js's shape, so this is mostly a change of names and the
// look: colours, axes, tooltips and where the legend goes. It has no Chart.js code of its own and no canvas, so it is
// tested on its own; `chartjs.ts` is what loads the library.

import type { ChartConfiguration, ChartOptions } from 'chart.js';
import type { Chart } from './boards';
import { blendColors, chartColor, defaultSeriesColor } from './colors';

export type ChartKind = 'bar' | 'line' | 'area' | 'pie' | 'donut' | 'radar' | 'polar' | 'heatmap';

/** A pie, a donut or a polar chart has one series, and its slices are the labels. */
export const isRound = (kind: ChartKind): boolean => kind === 'pie' || kind === 'donut' || kind === 'polar';

export interface Theme {
  /** Text, ticks and legend. */
  text: string;
  /** Grid lines and the edges of slices. */
  line: string;
}

export interface ChartInput {
  chart: Chart;
  kind: ChartKind;
  /** The labels are periods in time order (so a bar chart stands up, rather than lies down). */
  time: boolean;
  stacked: boolean;
  /** A series' own colour, by position: a Taskwarrior colour name or `#rrggbb`. The page's palette gives the rest. */
  colors?: (string | undefined)[];
  /** The colour of each label's value, by position, where the taskrc gives it one (`color.uda.<name>.<value>`). */
  labelColors?: (string | undefined)[];
  /** A heatmap's rows are the series (each in its colour), not rows of one series' grid. */
  rowsAreSeries?: boolean;
  /** The page is dark, so the names are drawn in the palette for it. */
  dark: boolean;
  /** A value as it is written (a duration the way Taskwarrior writes one). */
  format: (v: number | null) => string;
  theme: Theme;
}

/** `#rgb` or `#rrggbb` with an opacity (0 to 255), for the fill under a line. */
export function withAlpha(color: string, alpha: number): string {
  const hex = color.startsWith('#') ? color.slice(1) : '';
  const full = hex.length === 3 ? [...hex].map((c) => c + c).join('') : hex.slice(0, 6);
  if (full.length !== 6) return color;
  return `#${full}${Math.round(Math.min(Math.max(alpha, 0), 255))
    .toString(16)
    .padStart(2, '0')}`;
}

const JS_TYPE = {
  heatmap: 'matrix',
  bar: 'bar',
  line: 'line',
  area: 'line',
  pie: 'pie',
  donut: 'doughnut',
  radar: 'radar',
  polar: 'polarArea',
} as const;

/** What Chart.js hands an external tooltip: the chart (for where its canvas is) and the tooltip model (what to say and where). */
interface TipContext {
  chart: { canvas: HTMLCanvasElement };
  tooltip: { opacity: number; caretX: number; caretY: number; title?: string[]; body?: { lines: string[] }[] };
}

/**
 * A heatmap's tooltip as an element of the page, put beside the cell and kept inside the window, in the page's own colours. There is
 * one, shared, and it is hidden when the pointer leaves a cell.
 */
function pageTooltip({ chart, tooltip }: TipContext): void {
  let el = document.getElementById('heat-tip');
  if (!el) {
    el = document.createElement('div');
    el.id = 'heat-tip';
    el.setAttribute('role', 'tooltip');
    Object.assign(el.style, {
      position: 'fixed',
      zIndex: '1000',
      pointerEvents: 'none',
      padding: '6px 9px',
      borderRadius: '6px',
      fontSize: '12px',
      lineHeight: '1.45',
      whiteSpace: 'nowrap',
    });
    document.body.appendChild(el);
  }
  if (!tooltip.opacity) {
    el.style.display = 'none';
    return;
  }
  const css = getComputedStyle(document.documentElement);
  el.style.background = css.getPropertyValue('--panel').trim() || '#1c1d17';
  el.style.color = css.getPropertyValue('--text').trim() || '#e9e9e2';
  el.style.border = `1px solid ${css.getPropertyValue('--line').trim() || '#444'}`;
  el.style.boxShadow = '0 4px 14px rgba(0,0,0,0.25)';
  el.replaceChildren();
  const title = document.createElement('div');
  title.style.fontWeight = '600';
  title.textContent = (tooltip.title ?? []).join(' ');
  el.appendChild(title);
  for (const line of (tooltip.body ?? []).flatMap((b) => b.lines)) {
    const row = document.createElement('div');
    row.textContent = line;
    el.appendChild(row);
  }
  el.style.display = 'block';
  const box = chart.canvas.getBoundingClientRect();
  const x = box.left + tooltip.caretX + 14;
  const y = box.top + tooltip.caretY + 14;
  const { width, height } = el.getBoundingClientRect();
  el.style.left = `${Math.max(4, Math.min(x, window.innerWidth - width - 4))}px`;
  el.style.top = `${Math.max(4, y + height > window.innerHeight - 4 ? box.top + tooltip.caretY - height - 10 : y)}px`;
}

/** A cell of a heatmap: its column, its row and what it measures, and the colour it is in. */
interface Cell {
  x: string;
  y: string;
  v: number | null;
  color: string;
  /** A stacked cell: what each series had. */
  parts?: { label: string; v: number | null }[];
}

/**
 * A heatmap as the matrix plugin draws it: one dataset of cells, a column for each label and a row for each row of the chart, a
 * cell as strong as its value is large. The colour of a cell is its series': a row for each series has each row in its own, a
 * stacked cell is the colours of the series it is made of in the proportions of what they had, and any other grid is one series'.
 * A cell with nothing to measure is faint. The cells are squares.
 */
function heatmapConfig(input: ChartInput): ChartConfiguration {
  const { chart, format, theme, dark } = input;
  const xs = chart.labels.map((l) => l.label);
  const ys = chart.datasets.map((d) => d.label);
  const colorOf = (i: number) => chartColor(input.colors?.[i], dark) ?? defaultSeriesColor(i, dark);
  const colorFor = (row: number, col: number): string => {
    const parts = chart.datasets[row].parts;
    if (parts?.length)
      return blendColors(
        parts.map((_, k) => colorOf(k)),
        parts.map((p) => p.data[col] ?? 0),
      );
    return colorOf(input.rowsAreSeries ? row : 0);
  };
  const cells: Cell[] = chart.datasets.flatMap((d, row) =>
    xs.map((x, col) => ({
      x,
      y: ys[row],
      v: d.data[col] ?? null,
      color: colorFor(row, col),
      ...(d.parts?.length ? { parts: d.parts.map((p) => ({ label: p.label, v: p.data[col] ?? null })) } : {}),
    })),
  );
  const values = cells.flatMap((c) => (c.v === null ? [] : [c.v]));
  const low = Math.min(0, ...values);
  const high = Math.max(0, ...values);
  const strength = (v: number | null) => (v === null || high === low ? 0 : (v - low) / (high - low));
  // A square as large as the chart area allows for the columns and the rows it has.
  const side = (c: { chart: { chartArea?: { width: number; height: number } } }) =>
    Math.max(
      1,
      Math.min(
        (c.chart.chartArea?.width ?? 0) / Math.max(xs.length, 1),
        (c.chart.chartArea?.height ?? 0) / Math.max(ys.length, 1),
      ) - 2,
    );
  // The first row is at the top, as the rows of a table are, and a vertical category axis starts at the bottom: so the rows are listed
  // last first. The names of a few rows are all there; of many, every so many.
  const axis = (labels: string[], rows = false) => ({
    type: 'category' as const,
    labels: rows ? [...labels].reverse() : labels,
    offset: true,
    grid: { display: false },
    ticks: { color: theme.text, autoSkip: !rows || labels.length > 14, maxRotation: 0 },
  });
  const options: ChartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    animation: false,
    plugins: {
      legend: { display: false },
      tooltip: {
        // On the page, not on the canvas: a stacked cell has a few lines to say, and a short heatmap has no room for them.
        enabled: false,
        external: pageTooltip,
        callbacks: {
          title: (items) => {
            const raw = items[0]?.raw as Cell | undefined;
            return raw ? `${raw.y} · ${raw.x}` : '';
          },
          // A stacked cell says what each series had, under the total.
          label: (ctx) => {
            const cell = ctx.raw as Cell;
            return cell.parts
              ? [`Total ${format(cell.v)}`, ...cell.parts.map((p) => `${p.label}: ${format(p.v)}`)]
              : format(cell.v);
          },
        },
      },
    },
    scales: { x: axis(xs), y: axis(ys, true) },
  };
  return {
    type: 'matrix',
    data: {
      datasets: [
        {
          label: ys.join(', '),
          data: cells,
          backgroundColor: (ctx: { raw?: unknown }) => {
            const cell = ctx.raw as Cell | undefined;
            const v = cell?.v ?? null;
            // From a pale wash to the full colour; a cell with no value is the grid's own line colour.
            return v === null || !cell
              ? withAlpha(theme.line, 0x66)
              : withAlpha(cell.color, Math.round(0x26 + strength(v) * (0xff - 0x26)));
          },
          borderColor: theme.line,
          borderWidth: 1,
          borderRadius: 2,
          width: side,
          height: side,
        },
      ],
    },
    options,
  } as unknown as ChartConfiguration;
}

/** What Chart.js is given for this chart. */
export function chartConfig(input: ChartInput): ChartConfiguration {
  const { chart, kind, time, stacked, format, theme, dark } = input;
  if (kind === 'heatmap') return heatmapConfig(input);
  const round = isRound(kind);
  const many = chart.datasets.length > 1;
  const colorOf = (i: number) => chartColor(input.colors?.[i], dark) ?? defaultSeriesColor(i, dark);
  const horizontal = kind === 'bar' && !time;

  const datasets = chart.datasets.map((d, i) => {
    if (round) {
      // The slices have a colour each; the one series has no colour of its own.
      return {
        label: d.label,
        data: d.data.map((v) => v ?? 0),
        backgroundColor: chart.labels.map((_, j) => input.labelColors?.[j] ?? defaultSeriesColor(j, dark)),
        borderColor: theme.line,
        borderWidth: 1,
      };
    }
    const c = colorOf(i);
    if (kind === 'bar') {
      // One series of values: a bar of a value with a colour of its own in the taskrc has it. Several series are told apart by theirs.
      const own = !many && !time && input.labelColors?.some(Boolean);
      const fill = own ? chart.labels.map((_, j) => input.labelColors?.[j] ?? c) : c;
      return { label: d.label, data: d.data, backgroundColor: fill, borderRadius: 2, maxBarThickness: 48 };
    }
    const filled = kind === 'area' || kind === 'radar';
    return {
      label: d.label,
      data: d.data,
      borderColor: c,
      backgroundColor: filled ? withAlpha(c, 0x40) : c,
      // A stacked area lies between the line under it and its own ('-1', the one before); the first, and any area that is not stacked, down to the axis.
      fill: filled && stacked && kind === 'area' && i > 0 ? '-1' : filled,
      borderWidth: 2,
      pointRadius: 3,
      spanGaps: false,
    };
  });

  const total = (d: { data: (number | null)[] }) => d.data.reduce<number>((s, v) => s + (v ?? 0), 0);
  const options: ChartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    animation: false,
    plugins: {
      legend: {
        display: many || round,
        position: round ? 'right' : 'top',
        labels: { color: theme.text, boxWidth: 10, boxHeight: 10, usePointStyle: false },
      },
      tooltip: {
        callbacks: {
          label: (ctx) => {
            const raw = typeof ctx.raw === 'number' ? ctx.raw : null;
            if (round) {
              const all = total(chart.datasets[ctx.datasetIndex] ?? { data: [] });
              const pct = all > 0 && raw !== null ? ` (${Math.round((raw / all) * 100)}%)` : '';
              return `${ctx.label}: ${format(raw)}${pct}`;
            }
            return many ? `${ctx.dataset.label}: ${format(raw)}` : format(raw);
          },
        },
      },
    },
  };

  if (kind === 'radar' || kind === 'polar') {
    options.scales = {
      r: {
        beginAtZero: true,
        grid: { color: theme.line },
        angleLines: { color: theme.line },
        pointLabels: { color: theme.text },
        ticks: { color: theme.text, backdropColor: 'transparent', callback: (v) => format(Number(v)) },
      },
    };
  } else if (!round) {
    // The axis along the labels, and the axis of the values (the other way round for bars that lie down).
    const along = { stacked, grid: { display: false }, ticks: { color: theme.text, autoSkip: true, maxRotation: 0 } };
    const across = {
      stacked,
      beginAtZero: true,
      grid: { color: theme.line },
      ticks: { color: theme.text, callback: (v: string | number) => format(Number(v)) },
    };
    options.scales = horizontal ? { y: along, x: across } : { x: along, y: across };
    if (horizontal) options.indexAxis = 'y';
  } else if (kind === 'donut') {
    (options as { cutout?: string }).cutout = '58%';
  }

  return {
    type: JS_TYPE[kind],
    data: { labels: chart.labels.map((l) => l.label), datasets },
    options,
  } as ChartConfiguration;
}

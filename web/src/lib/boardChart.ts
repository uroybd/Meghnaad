// The chart of a board section as Chart.js draws it. The engine's `Chart` (labels, and a dataset of values for each
// series, `tc_core::board::aggregate::chart`) already has Chart.js's shape, so this is mostly a change of names and the
// look: colours, axes, tooltips and where the legend goes. It has no Chart.js code of its own and no canvas, so it is
// tested on its own; `chartjs.ts` is what loads the library.

import type { ChartConfiguration, ChartOptions } from 'chart.js';
import type { Chart } from './boards';
import { chartColor, defaultSeriesColor } from './colors';

export type ChartKind = 'bar' | 'line' | 'area' | 'pie' | 'donut' | 'radar' | 'polar';

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
  bar: 'bar',
  line: 'line',
  area: 'line',
  pie: 'pie',
  donut: 'doughnut',
  radar: 'radar',
  polar: 'polarArea',
} as const;

/** What Chart.js is given for this chart. */
export function chartConfig(input: ChartInput): ChartConfiguration {
  const { chart, kind, time, stacked, format, theme, dark } = input;
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
        backgroundColor: chart.labels.map((_, j) => defaultSeriesColor(j, dark)),
        borderColor: theme.line,
        borderWidth: 1,
      };
    }
    const c = colorOf(i);
    if (kind === 'bar')
      return { label: d.label, data: d.data, backgroundColor: c, borderRadius: 2, maxBarThickness: 48 };
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

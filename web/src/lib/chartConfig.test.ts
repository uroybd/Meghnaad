import { describe, expect, it } from 'vitest';
import { chartConfig, isRound, withAlpha, type ChartInput, type ChartKind } from './boardChart';
import { defaultSeriesColor, namedColorHex } from './colors';
import type { Chart } from './boards';

const chart = (series = 2): Chart => ({
  unit: 'number',
  labels: [
    { key: 'a', label: 'Home' },
    { key: 'b', label: 'Work' },
    { key: 'c', label: '(none)' },
  ],
  datasets: Array.from({ length: series }, (_, d) => ({
    label: `s${d}`,
    data: [1 + d, 3, null],
    counts: [1, 1, 0],
    total: null,
    tasks: 0,
  })),
});
const theme = { text: '#111', line: '#ddd' };
const input = (kind: ChartKind, extra: Partial<ChartInput> = {}): ChartInput => ({
  chart: chart(),
  kind,
  time: false,
  stacked: false,
  dark: false,
  format: (v) => (v === null ? '–' : `${v}!`),
  theme,
  ...extra,
});
// Chart.js's types are wide; the tests look at plain objects.
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the configuration is read here as plain data
type Plain = Record<string, any>;
const cfg = (i: ChartInput) =>
  chartConfig(i) as unknown as { type: string; data: { labels: string[]; datasets: Plain[] }; options: Plain };

describe('a chart for Chart.js', () => {
  it('has the labels and a dataset for each series, as the engine gives them', () => {
    const c = cfg(input('bar'));
    expect(c.data.labels).toEqual(['Home', 'Work', '(none)']);
    expect(c.data.datasets.map((d) => d.label)).toEqual(['s0', 's1']);
    // Nothing to measure stays nothing, so a gap is a gap.
    expect(c.data.datasets[0].data).toEqual([1, 3, null]);
  });

  it('names Chart.js’s type for each of ours', () => {
    const type = (k: ChartKind) => cfg(input(k, { chart: chart(1) })).type;
    const kinds: ChartKind[] = ['bar', 'line', 'area', 'pie', 'donut', 'radar', 'polar'];
    expect(kinds.map(type)).toEqual(['bar', 'line', 'line', 'pie', 'doughnut', 'radar', 'polarArea']);
  });

  it('colours each series in turn from the page’s palette, or as it asks by name or by value', () => {
    const c = cfg(input('bar', { colors: [undefined, '#112233'] }));
    expect(c.data.datasets.map((d) => d.backgroundColor)).toEqual([defaultSeriesColor(0, false), '#112233']);
    const named = cfg(input('bar', { colors: ['bright red', 'Cyan'] }));
    expect(named.data.datasets.map((d) => d.backgroundColor)).toEqual([
      namedColorHex('bright red', false),
      namedColorHex('cyan', false),
    ]);
    // A name that is not one is the default, not a broken colour.
    expect(cfg(input('bar', { colors: ['puce'] })).data.datasets[0].backgroundColor).toBe(defaultSeriesColor(0, false));
  });

  it('draws the names in the palette for a dark page', () => {
    const light = cfg(input('bar', { colors: ['red', 'blue'] })).data.datasets.map((d) => d.backgroundColor);
    const dark = cfg(input('bar', { colors: ['red', 'blue'], dark: true })).data.datasets.map((d) => d.backgroundColor);
    expect(dark).not.toEqual(light);
    expect(dark).toEqual([namedColorHex('red', true), namedColorHex('blue', true)]);
  });

  it('fills under an area and a radar, and not under a line', () => {
    const fill = (k: ChartKind) => cfg(input(k)).data.datasets[0].fill;
    expect([fill('line'), fill('area'), fill('radar')]).toEqual([false, true, true]);
    expect(cfg(input('area', { colors: ['#2da44e'] })).data.datasets[0].backgroundColor).toBe('#2da44e40');
  });

  it('stacks both axes of bars together, and lays a category down', () => {
    const o = cfg(input('bar', { stacked: true })).options;
    expect(o.indexAxis).toBe('y');
    expect(o.scales.x.stacked).toBe(true);
    expect(o.scales.y.stacked).toBe(true);
    // A date stands up.
    const t = cfg(input('bar', { time: true })).options;
    expect(t.indexAxis).toBeUndefined();
    expect(t.scales.x.stacked).toBe(false);
    expect(t.scales.y.beginAtZero).toBe(true);
  });

  it('writes the values on the axis and in the tooltip the way the section does', () => {
    const o = cfg(input('line', { time: true })).options;
    expect(o.scales.y.ticks.callback(5)).toBe('5!');
    const tip = o.plugins.tooltip.callbacks.label;
    expect(tip({ raw: 3, dataset: { label: 's1' }, datasetIndex: 1 })).toBe('s1: 3!');
    expect(tip({ raw: null, dataset: { label: 's1' }, datasetIndex: 1 })).toBe('s1: –');
    // One series needs no name in front of it.
    const one = cfg(input('line', { chart: chart(1), time: true })).options.plugins.tooltip.callbacks.label;
    expect(one({ raw: 2, dataset: { label: 'x' }, datasetIndex: 0 })).toBe('2!');
  });

  it('shows a legend for several series and for slices, not for one series', () => {
    const legend = (k: ChartKind, series: number) => cfg(input(k, { chart: chart(series) })).options.plugins.legend;
    expect(legend('bar', 1).display).toBe(false);
    expect(legend('bar', 2).display).toBe(true);
    expect(legend('pie', 1)).toMatchObject({ display: true, position: 'right' });
  });

  it('gives a round chart a colour for each slice and a percentage in the tooltip', () => {
    expect(isRound('donut')).toBe(true);
    expect(isRound('radar')).toBe(false);
    const c = cfg(input('donut', { chart: chart(1) }));
    expect(c.data.datasets[0].backgroundColor).toEqual([0, 1, 2].map((i) => defaultSeriesColor(i, false)));
    // Nothing to measure is nothing of the pie.
    expect(c.data.datasets[0].data).toEqual([1, 3, 0]);
    expect(c.options.cutout).toBe('58%');
    expect(c.options.scales).toBeUndefined();
    expect(c.options.plugins.tooltip.callbacks.label({ label: 'Home', raw: 1, datasetIndex: 0 })).toBe(
      'Home: 1! (25%)',
    );
  });

  it('gives a radar and a polar chart a radial scale', () => {
    for (const k of ['radar', 'polar'] as const) {
      const o = cfg(input(k, { chart: chart(1) })).options;
      expect(Object.keys(o.scales)).toEqual(['r']);
      expect(o.scales.r.beginAtZero).toBe(true);
    }
  });

  it('follows the page’s colours', () => {
    const o = cfg(input('bar', { time: true })).options;
    expect(o.scales.x.ticks.color).toBe('#111');
    expect(o.scales.y.grid.color).toBe('#ddd');
  });
});

describe('a colour with an opacity', () => {
  it('is #rrggbb and two hex digits, from #rgb too', () => {
    expect(withAlpha('#2da44e', 0x40)).toBe('#2da44e40');
    expect(withAlpha('#abc', 255)).toBe('#aabbccff');
    expect(withAlpha('red', 0x40)).toBe('red');
  });
});

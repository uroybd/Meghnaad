// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import type { ChartKind } from './boardChart';
import type { Chart } from './boards';
import { reactive } from '../reactive.svelte';
import BoardChart from './BoardChart.svelte';

// jsdom has no canvas, so Chart.js is a stand-in that notes what it is asked.
const made: FakeChart[] = [];
class FakeChart {
  config: { type: string };
  data: unknown;
  options: unknown;
  updates: string[] = [];
  destroyed = false;
  constructor(
    public canvas: HTMLCanvasElement,
    config: { type: string; data: unknown; options: unknown },
  ) {
    this.config = config;
    this.data = config.data;
    this.options = config.options;
    made.push(this);
  }
  update(mode: string) {
    this.updates.push(mode);
  }
  destroy() {
    this.destroyed = true;
  }
}
vi.mock('./chartjs', () => ({ Chart: FakeChart }));

const chart = (n: number): Chart => ({
  unit: 'number',
  labels: Array.from({ length: n }, (_, i) => ({ key: String(i), label: `L${i}` })),
  datasets: [{ label: 's', data: Array.from({ length: n }, (_, i) => i), counts: [], total: null, tasks: 0 }],
});
const props = (extra: object = {}) => ({
  chart: chart(3),
  kind: 'bar' as ChartKind,
  time: false,
  format: (v: number | null) => String(v),
  summary: 'a summary',
  ...extra,
});
// The chart library arrives by a dynamic import, which takes more than a few microtasks.
const settle = async () => {
  await new Promise((r) => setTimeout(r, 15));
  flushSync();
};

let app: ReturnType<typeof mount> | null = null;
beforeEach(() => {
  made.length = 0;
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

describe('a chart on a board', () => {
  it('makes a Chart.js chart on its canvas, and tells a screen reader what it shows', async () => {
    app = mount(BoardChart, { target: document.body, props: props() });
    await settle();
    expect(made).toHaveLength(1);
    expect(made[0].canvas).toBe(document.querySelector('canvas'));
    expect(made[0].config.type).toBe('bar');
    const box = document.querySelector('.box')!;
    expect(box.getAttribute('role')).toBe('img');
    expect(box.getAttribute('aria-label')).toBe('a summary');
  });

  it('draws nothing, and says so, when there is nothing to show', async () => {
    app = mount(BoardChart, { target: document.body, props: props({ chart: chart(0) }) });
    await settle();
    expect(made).toHaveLength(0);
    expect(document.body.textContent).toContain('Nothing to show.');
    expect(document.querySelector('canvas')).toBeNull();
  });

  it('is updated in place when its data changes, and made again when its type does', async () => {
    const p = reactive(props());
    app = mount(BoardChart, { target: document.body, props: p });
    await settle();
    p.chart = chart(5);
    await settle();
    expect(made).toHaveLength(1);
    expect(made[0].updates).toEqual(['none']);
    expect((made[0].data as { labels: string[] }).labels).toHaveLength(5);

    p.kind = 'pie';
    await settle();
    expect(made[0].destroyed).toBe(true);
    expect(made).toHaveLength(2);
    expect(made[1].config.type).toBe('pie');
  });

  it('lets the chart go when it leaves the page', async () => {
    app = mount(BoardChart, { target: document.body, props: props() });
    await settle();
    unmount(app);
    app = null;
    expect(made[0].destroyed).toBe(true);
  });
});

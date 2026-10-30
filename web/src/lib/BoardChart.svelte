<script lang="ts">
  import { onMount } from 'svelte';
  import type { Chart as ChartJs } from 'chart.js';
  import { chartConfig, type ChartKind, type Theme } from './boardChart';
  import type { Chart } from './boards';
  import { scheme } from './scheme.svelte';

  /**
   * A chart of a board section, drawn by Chart.js (loaded when the first chart is drawn) on a canvas that fills its box and
   * is redrawn as the section is resized. What it draws, and how, is `chartConfig`; this keeps the canvas and the library's
   * chart in step with it.
   */
  let {
    chart,
    kind,
    time,
    stacked = false,
    colors = [],
    labelColors = [],
    rowsAreSeries = false,
    format,
    summary,
  }: {
    chart: Chart;
    kind: ChartKind;
    /** The labels are periods in time order. */
    time: boolean;
    stacked?: boolean;
    /** A series' own colour, by position (a colour name or `#rrggbb`); the page's palette where there is none. */
    colors?: (string | undefined)[];
    /** The taskrc's colour for each label's value (a UDA's `color.uda.<name>.<value>`), by position. */
    labelColors?: (string | undefined)[];
    /** A heatmap's rows are the series, each in its colour. */
    rowsAreSeries?: boolean;
    /** A value as it is written (a duration the way Taskwarrior writes one). */
    format: (v: number | null) => string;
    /** What a screen reader hears instead of the picture. */
    summary: string;
  } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let boxWidth = $state(0);
  let boxHeight = $state(0);

  /**
   * What the canvas is given to fill. A heatmap's cells are squares, so its chart area is as many of them as it has columns and
   * rows across and down (with room for the labels, which are estimated: a pixel or two is not seen); the rest is left empty.
   * Anything else fills the box.
   */
  const frame = $derived.by(() => {
    if (kind !== 'heatmap' || !boxWidth || !boxHeight || !chart.labels.length) return 'width:100%;height:100%';
    const rows = Math.max(chart.datasets.length, 1);
    const columns = Math.max(chart.labels.length, 1);
    const names = Math.max(...chart.datasets.map((d) => d.label.length), 1);
    const left = Math.min(names * 7 + 18, boxWidth * 0.4);
    const bottom = 34;
    const side = Math.max(8, Math.floor(Math.min((boxWidth - left - 8) / columns, (boxHeight - bottom - 8) / rows)));
    return `width:${left + columns * side + 8}px;height:${bottom + rows * side + 8}px`;
  });
  let instance: ChartJs | null = null;
  let failed = $state<string | null>(null);

  // The page's own colours, read when it is light or dark (Chart.js is given colours, not variables).
  function readTheme(): Theme {
    const css = getComputedStyle(document.documentElement);
    return {
      text: css.getPropertyValue('--dim').trim() || '#75756f',
      line: css.getPropertyValue('--line').trim() || '#e2e2dd',
    };
  }

  const config = $derived.by(() => {
    void scheme.dark;
    return chartConfig({
      chart,
      kind,
      time,
      stacked,
      colors,
      labelColors,
      rowsAreSeries,
      dark: scheme.dark,
      format,
      theme: readTheme(),
    });
  });

  // Draw: make the chart the first time, or when the type changes (Chart.js cannot change it), else update it in place.
  $effect(() => {
    const c = config;
    if (!canvas || !chart.labels.length) return;
    let live = true;
    void import('./chartjs')
      .then(({ Chart: Lib }) => {
        if (!live || !canvas) return;
        if (instance && (instance.config as { type?: string }).type !== c.type) {
          instance.destroy();
          instance = null;
        }
        if (instance) {
          instance.data = c.data;
          instance.options = c.options ?? {};
          instance.update('none');
        } else {
          instance = new Lib(canvas, c);
        }
        failed = null;
      })
      .catch((e: unknown) => {
        failed = e instanceof Error ? e.message : String(e);
      });
    return () => {
      live = false;
    };
  });

  onMount(() => () => {
    instance?.destroy();
    instance = null;
  });
</script>

<div
  class="box"
  class:heat={kind === 'heatmap'}
  bind:clientWidth={boxWidth}
  bind:clientHeight={boxHeight}
  role={chart.labels.length ? 'img' : undefined}
  aria-label={chart.labels.length ? summary : undefined}
>
  {#if !chart.labels.length}
    <p class="dim empty">Nothing to show.</p>
  {:else if failed}
    <p class="err" role="alert">The chart could not be drawn: {failed}</p>
  {:else}
    <div class="frame" style={frame}><canvas bind:this={canvas}></canvas></div>
  {/if}
</div>

<style>
  /* It fills the section's body, which is positioned; Chart.js sizes the canvas to it and follows it as it changes. */
  .box {
    position: absolute;
    inset: 0;
  }
  /* A heatmap is as big as its squares make it: it sits in the middle of the card, across and up and down. */
  .box.heat {
    display: flex;
    overflow: auto;
  }
  .frame {
    position: relative;
    /* Auto margins centre it when there is room, and start it at the top left when there is not (a big heatmap scrolls). */
    margin: auto;
    flex: none;
  }
  .empty {
    margin: 8px 0;
  }
</style>

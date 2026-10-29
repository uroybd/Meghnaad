<script lang="ts">
  import BoardChart from './BoardChart.svelte';
  import type { ChartKind } from './boardChart';
  import { describeChart, formatValue, orderChart, orderGrids, type Section, type SectionData } from './boards';
  import { valueColors } from './colors';
  import { compactDuration } from './dates';
  import { Info } from './icons';
  import ResultView from './ResultView.svelte';
  import { scheme } from './scheme.svelte';
  import { store, type Entry } from './store.svelte';

  /** What one section of a board shows: a number, a table, or a chart, with its title above. */
  let {
    section,
    data,
    editing = false,
    compact = false,
    oninfo,
  }: {
    section: Section;
    data: SectionData | undefined;
    editing?: boolean;
    /** Too narrow for buttons: while the layout is edited the section's menu has the information button too. */
    compact?: boolean;
    oninfo?: () => void;
  } = $props();

  const answer = $derived(data?.kind === 'answer' ? data.answer : null);
  /** A heatmap with rows: a grid for each series, the columns the same in each. */
  const grids = $derived(data?.kind === 'chart' && data.grids ? orderGrids(data.grids, section, data.limit) : null);
  const chart = $derived(grids?.[0] ?? (data?.kind === 'chart' ? orderChart(data.chart, section, data.limit) : null));
  /**
   * The colours a heatmap's grid is drawn in: with rows, a grid is one series' (all of them, when they are stacked into one); with no
   * rows each row is a series'.
   */
  const heatColors = (grid: number): (string | undefined)[] =>
    section.rows && !section.stacked ? [section.series?.[grid]?.color] : (section.series?.map((x) => x.color) ?? []);
  const unit = $derived(answer?.unit ?? chart?.unit ?? 'number');
  const fmt = (v: number | null) => formatValue(v, unit, compactDuration);
  const total = $derived(answer ? fmt(answer.total) : '');
  const summary = $derived(chart ? describeChart(section.title, chart, fmt) : section.title);

  // A table is drawn by the component the Tasks page uses, which wants an entry like the ones the console keeps.
  const entry = $derived<Entry | null>(
    data?.kind === 'table'
      ? {
          id: 0,
          input: { args: [] },
          title: section.report || 'next',
          result: data.result,
          loading: false,
          failure: null,
          at: 0,
        }
      : null,
  );
</script>

<section class="section" aria-label={section.title}>
  <header class:editing class:compact={editing && compact}>
    <h3 class="title">{section.title}</h3>
    {#if !(editing && compact)}
      <button
        class="ghost info"
        id="board-info-{section.id}"
        aria-label="Information about {section.title}"
        title="What this shows: the filter, the tasks it got, and the values"
        disabled={!data}
        onclick={() => oninfo?.()}><Info size={15} /></button
      >
    {/if}
  </header>
  <div class="body">
    {#if !data}
      <p class="dim">Loading…</p>
    {:else if data.kind === 'error'}
      <p class="err" role="alert">{data.message}</p>
    {:else if entry}
      <ResultView {entry} />
    {:else if answer}
      <p class="big" aria-label="{section.title}: {total}">{total}</p>
      <p class="dim small">
        {answer.tasks}
        {answer.tasks === 1 ? 'task' : 'tasks'}
      </p>
    {:else if chart}
      {#if grids}
        <!-- Each grid is a series' colour; stacked, they are one (the first series') -->
        <div class="grids">
          {#each grids as g, i (i)}
            <div class="grid">
              {#if grids.length > 1}<span class="dim small">{g.label}</span>{/if}
              <div class="plot">
                <BoardChart
                  chart={g}
                  kind="heatmap"
                  time={!!section.by?.time}
                  colors={heatColors(i)}
                  rowsAreSeries={!section.rows && !section.stacked}
                  format={fmt}
                  summary={describeChart(g.label || section.title, g, fmt)}
                />
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <BoardChart
          {chart}
          kind={section.type as ChartKind}
          time={!!section.by?.time}
          stacked={!!section.stacked}
          colors={section.series?.map((x) => x.color)}
          labelColors={valueColors(section.by?.field, chart.labels, store.config?.colors, scheme.dark)}
          format={fmt}
          {summary}
        />
      {/if}
    {/if}
  </div>
</section>

<style>
  .section {
    display: flex;
    flex-direction: column;
    height: 100%;
    /* On a narrow screen the cell around it is as tall as its row count asks and nothing more. */
    min-height: inherit;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 0 0 6px;
  }
  /* While the layout is edited, the tools of the section sit over the top right corner. */
  header.editing {
    padding-right: 96px;
  }
  /* One ⋯ button over the corner instead of the row of tools. */
  header.compact {
    padding-right: 40px;
  }
  .title {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--dim);
  }
  .info {
    flex: none;
    padding: 2px 4px;
    color: var(--dim);
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  /* A heatmap's grids share the body, each with a plot that is positioned for the chart to fill. */
  .grids {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
  }
  .grid {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 90px;
  }
  .plot {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .big {
    margin: 4px 0 0;
    font-size: 34px;
    font-weight: 650;
    line-height: 1.1;
  }
  .small {
    margin: 2px 0 0;
    font-size: 12px;
  }
</style>

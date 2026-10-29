<script lang="ts">
  import BoardChart from './BoardChart.svelte';
  import type { ChartKind } from './boardChart';
  import { describeChart, formatValue, orderChart, type Section, type SectionData } from './boards';
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
  const chart = $derived(data?.kind === 'chart' ? orderChart(data.chart, section, data.limit) : null);
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

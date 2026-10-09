<script lang="ts">
  import { formatFor } from './dateformat';
  import { formatMoment } from './dates';
  import { store } from './store.svelte';
  import type { BurndownBar, BurndownResult } from './types';

  /** `burndown.*`: pending, started and done over time, as stacked bars. */
  let { result }: { result: BurndownResult } = $props();

  const settings = $derived(store.config?.config.settings);
  const dateFmt = $derived(formatFor('report', settings));

  // The drawing, in SVG units: a plot area with the y axis on its left and two lines of labels below.
  const STEP = 30;
  const BAR = 22;
  const PLOT_H = 240;
  const LEFT = 46;
  const PAD_TOP = 12;
  const LABELS_H = 46;
  const top = $derived(Math.max(result.y_labels[2], 1));
  const width = $derived(LEFT + result.bars.length * STEP + 8);
  const height = PAD_TOP + PLOT_H + LABELS_H;

  const done = (b: BurndownBar) => b.done + result.carryover_done;
  const h = (n: number) => (Math.max(n, 0) / top) * PLOT_H;
  const x = (i: number) => LEFT + i * STEP + (STEP - BAR) / 2;
  const base = PAD_TOP + PLOT_H;

  const empty = $derived(result.bars.every((b) => b.pending + b.started + done(b) === 0));
  /** The month or year label is only written where it changes. */
  const majors = $derived(result.bars.map((b, i) => (i === 0 || b.major !== result.bars[i - 1].major ? b.major : '')));

  const describe = (b: BurndownBar) =>
    `${b.major} ${b.minor}: ${b.pending} pending, ${b.started} started, ${done(b)} done`;
  const summary = $derived(
    `${result.title}: ${result.bars.length} bars. Latest: ${describe(result.bars[result.bars.length - 1])}.`,
  );
</script>

<h4 class="title">{result.title}</h4>
{#if empty}
  <p class="dim">No matches.</p>
{:else}
  <div class="scroll">
    <svg viewBox="0 0 {width} {height}" role="img" aria-label={summary} style="min-width: {Math.min(width, 640)}px">
      {#each result.y_labels as v, _i (_i)}
        {@const y = base - h(v)}
        <line class="grid" x1={LEFT} x2={width - 4} y1={y} y2={y} />
        <text class="axis" x={LEFT - 8} y={y + 4} text-anchor="end">{v}</text>
      {/each}
      <line class="baseline" x1={LEFT} x2={width - 4} y1={base} y2={base} />

      {#each result.bars as b, i (b.epoch)}
        {@const p = h(b.pending)}
        {@const s = h(b.started)}
        {@const d = h(done(b))}
        <g>
          <title>{describe(b)}</title>
          <rect class="pending" x={x(i)} y={base - p} width={BAR} height={p} />
          <rect class="started" x={x(i)} y={base - p - s} width={BAR} height={s} />
          <rect class="done" x={x(i)} y={base - p - s - d} width={BAR} height={d} />
          <text class="minor" x={x(i) + BAR / 2} y={base + 16} text-anchor="middle">{b.minor}</text>
          {#if majors[i]}
            <text class="major" x={x(i)} y={base + 33}>{majors[i]}</text>
          {/if}
        </g>
      {/each}
    </svg>
  </div>

  <p class="legend dim" aria-label="Legend">
    <span class="key done">Done</span><span class="key started">Started</span><span class="key pending">Pending</span>
  </p>
  <dl class="rates">
    <dt>Net fix rate</dt>
    <dd>{result.net_fix_rate != null ? `${result.net_fix_rate.toFixed(1)}/d` : '-'}</dd>
    {#if result.completion}
      <dt>Estimated completion</dt>
      <dd>
        {formatMoment(result.completion.epoch, undefined, dateFmt)} <span class="dim">({result.completion.vague})</span>
      </dd>
    {:else if result.no_convergence}
      <dt>Estimated completion</dt>
      <dd>No convergence</dd>
    {/if}
  </dl>
{/if}

<style>
  .title {
    margin: 4px 0 8px;
    font-size: 15px;
  }
  .scroll {
    overflow-x: auto;
  }
  svg {
    display: block;
    width: 100%;
    height: auto;
    max-width: 1100px;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
    stroke-dasharray: 3 4;
  }
  .baseline {
    stroke: var(--dim);
    stroke-width: 1;
  }
  .axis,
  .minor,
  .major {
    fill: var(--dim);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .major {
    font-weight: 600;
    fill: var(--text);
  }
  .pending {
    fill: color-mix(in srgb, var(--err) 80%, transparent);
  }
  .started {
    fill: color-mix(in srgb, var(--warn) 85%, transparent);
  }
  .done {
    fill: color-mix(in srgb, var(--ok) 75%, transparent);
  }
  rect {
    shape-rendering: crispEdges;
  }
  g:hover rect {
    filter: brightness(1.12);
  }
  .legend {
    display: flex;
    gap: 16px;
    font-size: 12.5px;
    margin: 6px 0;
  }
  .key {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .key::before {
    content: '';
    width: 14px;
    height: 14px;
    border-radius: 3px;
  }
  .key.done::before {
    background: color-mix(in srgb, var(--ok) 75%, transparent);
  }
  .key.started::before {
    background: color-mix(in srgb, var(--warn) 85%, transparent);
  }
  .key.pending::before {
    background: color-mix(in srgb, var(--err) 80%, transparent);
  }
  .rates {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 3px 18px;
    margin: 8px 0;
    font-size: 13.5px;
  }
  .rates dt {
    color: var(--dim);
  }
  .rates dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
</style>

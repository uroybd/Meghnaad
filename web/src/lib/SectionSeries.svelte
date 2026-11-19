<script lang="ts">
  import { isStacked, moveSeries, SPAN_PRESETS, type Section, type Series, type Span } from './boards';
  import { Plus } from './icons';
  import SectionItem from './SectionItem.svelte';

  /** A chart's series: each its own line or set of bars, of the tasks its filter picks. */
  let {
    section,
    set,
    dates,
  }: {
    section: Section;
    set: (patch: Partial<Section>) => void;
    /** The date attributes a span can run between. */
    dates: string[];
  } = $props();

  const MAX_SERIES = 12;
  const series = $derived(section.series ?? []);
  /** Stacked by a field has one series at most: its values are the series. */
  const seriesFull = $derived(series.length >= (section.stackBy ? 1 : MAX_SERIES));
  const setSeries = (list: Series[]) => set({ series: list.length ? list : undefined });
  const patchSeries = (i: number, patch: Partial<Series>) =>
    setSeries(series.map((x, j) => (j === i ? { ...x, ...patch } : x)));
  /** Which of the states of `task burndown` a span is, `custom` for any other, and `period` for none. */
  const spanKind = (x: Series): string =>
    !x.span ? 'period' : (SPAN_PRESETS.find((p) => JSON.stringify(p.span) === JSON.stringify(x.span))?.id ?? 'custom');
  function pickSpan(i: number, kind: string) {
    const span: Span | undefined =
      kind === 'period'
        ? undefined
        : kind === 'custom'
          ? { from: 'entry' }
          : SPAN_PRESETS.find((p) => p.id === kind)?.span;
    patchSeries(i, { span: span && { ...span } });
  }
  const patchSpan = (i: number, patch: Partial<Span>) => {
    const x = series[i];
    if (!x?.span) return;
    const next: Span = { ...x.span, ...patch };
    // An empty choice takes the date out.
    for (const k of ['to', 'unless'] as const) if (!next[k]) delete next[k];
    patchSeries(i, { span: next });
  };
  const addSeries = () => setSeries([...series, { label: `Series ${series.length + 1}` }]);
</script>

<fieldset class="wide series">
  <legend>Series</legend>
  <p class="dim hint">
    Each series is its own line or set of bars, of the tasks its filter picks (added to the filters above). With none,
    the chart is the one measure above. When they are stacked, the first is at the bottom and the next on it: the arrows
    change the order. In a heatmap with rows a series is a colour and a grid of its own, or, stacked, they are added
    into one; with no rows a series is a row, and stacked they are one row.
  </p>
  {#each series as x, i (i)}
    <SectionItem
      what="series"
      {i}
      count={series.length}
      item={x}
      filterId="series-filter-{section.id}-{i}"
      placeholder="Which tasks: outcome:failed"
      earlier="Move earlier: lower in a stack"
      later="Move later: higher in a stack"
      onpatch={(patch) => patchSeries(i, patch)}
      onmove={(by) => setSeries(moveSeries(series, i, by))}
      onremove={() => setSeries(series.filter((_, j) => j !== i))}
    >
      {#if section.by?.time}
        <div class="span-row">
          <label>
            Counts
            <select value={spanKind(x)} onchange={(e) => pickSpan(i, e.currentTarget.value)}>
              <option value="period">What happened in each period</option>
              {#each SPAN_PRESETS as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
              <option value="custom">Open between two dates…</option>
            </select>
          </label>
          {#if x.span && spanKind(x) === 'custom'}
            <label>
              From
              <select value={x.span.from} onchange={(e) => patchSpan(i, { from: e.currentTarget.value })}>
                {#each dates as d (d)}<option value={d}>{d}</option>{/each}
              </select>
            </label>
            <label>
              Until
              <select value={x.span.to ?? ''} onchange={(e) => patchSpan(i, { to: e.currentTarget.value })}>
                <option value="">never</option>
                {#each dates as d (d)}<option value={d}>{d}</option>{/each}
              </select>
            </label>
            <label>
              Unless
              <select value={x.span.unless ?? ''} onchange={(e) => patchSpan(i, { unless: e.currentTarget.value })}>
                <option value="">nothing</option>
                {#each dates as d (d)}<option value={d}>{d} has passed</option>{/each}
              </select>
            </label>
          {/if}
        </div>
      {/if}
    </SectionItem>
  {/each}
  <div class="row-series">
    <button onclick={addSeries} disabled={seriesFull}><Plus size={14} /> Add series</button>
    {#if series.length > 1 || section.stacked || section.stackBy}
      <label class="check">
        <input
          type="checkbox"
          checked={isStacked(section)}
          onchange={(e) =>
            set({
              // Stacked by a field is stacked unless it says not.
              stacked: section.stackBy
                ? e.currentTarget.checked
                  ? undefined
                  : false
                : e.currentTarget.checked || undefined,
            })}
        />
        Stacked
      </label>
    {/if}
  </div>
</fieldset>

<style>
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--dim);
  }
  label input,
  label select {
    font-size: 14px;
    color: var(--text);
  }
  .wide {
    grid-column: 1 / -1;
  }
  .series {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px 10px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .series legend {
    padding: 0 4px;
    font-size: 12px;
    color: var(--dim);
  }
  .hint {
    margin: 0;
  }
  .row-series {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .span-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    align-items: end;
  }
  .span-row select {
    max-width: 260px;
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
</style>

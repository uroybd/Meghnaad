<script lang="ts">
  import ColorPicker from './ColorPicker.svelte';
  import FilterInput from './FilterInput.svelte';
  import {
    FNS,
    SECTION_TYPES,
    SPAN_PRESETS,
    STEPS,
    isChartType,
    isRoundType,
    largestGroups,
    moveSeries,
    ROW_PARTS,
    rangeLabel,
    zoneNow,
    type DateRange,
    type Fn,
    type RowPart,
    type Rows,
    type Section,
    type SectionType,
    type Series,
    type Span,
  } from './boards';
  import { ArrowDown, ArrowUp, Plus, Trash2 } from './icons';
  import { store } from './store.svelte';

  /** The settings of one board section. It edits a copy and reports it; saving is the board's. */
  let {
    section,
    onchange,
    ranges = [],
  }: { section: Section; onchange: (next: Section) => void; ranges?: DateRange[] } = $props();

  const set = (patch: Partial<Section>) => onchange({ ...section, ...patch });

  // What can be measured and grouped by: the attributes Taskwarrior has, and the user's UDAs by type.
  const udas = $derived(Object.values(store.config?.config.udas ?? {}));
  const names = (types: string[]) => udas.filter((u) => types.includes(u.type)).map((u) => u.name);
  const MEASURABLE = $derived(['urgency', ...names(['numeric', 'duration'])]);
  const DATES = $derived([
    'due',
    'entry',
    'end',
    'start',
    'wait',
    'scheduled',
    'until',
    'modified',
    ...names(['date']),
  ]);
  const GROUPABLE = $derived(['project', 'tags', 'priority', 'status', ...names(['string', 'numeric'])]);

  const grouping = $derived(section.by?.time ? 'time' : 'field');
  const chart = $derived(isChartType(section.type));
  /** Several series are for charts that draw them side by side; a pie has one, its slices being the values. */
  const manySeries = $derived(chart && !isRoundType(section.type));

  function pickType(type: SectionType) {
    const patch: Partial<Section> = { type };
    if (isChartType(type) && !section.by) {
      patch.by =
        type === 'line' || type === 'area' || type === 'heatmap'
          ? { time: { field: 'entry', step: 'week' } }
          : { field: 'project' };
    }
    // A pie has one series, and none stacks. Only a heatmap has rows: it starts with the weekday of the date it is cut by.
    if (isRoundType(type)) Object.assign(patch, { series: undefined, stacked: undefined });
    if (type === 'heatmap') {
      if (!section.rows) patch.rows = { time: { field: section.by?.time?.field ?? 'entry', part: 'weekday' } };
    } else patch.rows = undefined;
    if (type === 'table' && !section.report) patch.report = 'next';
    set(patch);
  }

  function pickFn(fn: Fn) {
    set({ measure: fn === 'count' ? { fn } : { fn, field: section.measure.field ?? MEASURABLE[0] } });
  }

  function pickGrouping(kind: 'field' | 'time') {
    set({ by: kind === 'time' ? { time: { field: 'entry', step: 'week' } } : { field: 'project' } });
  }

  /** What a heatmap's rows are, as the choice it is in the list: none, a part of a date, or the value of an attribute. */
  const rowsKind = $derived(section.rows?.time ? section.rows.time.part : section.rows ? 'field' : '');
  const setRows = (rows: Rows | undefined) => set({ rows });
  function pickRows(v: string) {
    if (v === '') setRows(undefined);
    else if (v === 'field') setRows({ field: 'project' });
    else
      setRows({ time: { field: section.rows?.time?.field ?? section.by?.time?.field ?? 'entry', part: v as RowPart } });
  }

  const MAX_SERIES = 12;
  const series = $derived(section.series ?? []);
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

  /** The grouping by a date with some of it changed; what is not changed (the range of the axis) stays. */
  const setTime = (patch: Partial<NonNullable<NonNullable<Section['by']>['time']>>) =>
    set({ by: { time: { field: 'entry', step: 'week', ...section.by?.time, ...patch } } });
  /** What the axis covers, as the choice it is in the list: none, a range of the board, or dates of its own. */
  const overKind = $derived.by(() => {
    const r = section.by?.time?.range;
    return r === undefined ? '' : typeof r === 'string' ? `range:${r}` : 'own';
  });
  function pickOver(v: string) {
    const time = { field: 'entry', step: 'week', ...section.by?.time };
    delete time.range;
    if (v === '') set({ by: { time } });
    else if (v === 'own') set({ by: { time: { ...time, range: { start: 'sow', end: 'eow' } } } });
    else set({ by: { time: { ...time, range: v.slice('range:'.length) } } });
  }
  const setOwn = (patch: Partial<{ start: string; end: string }>) => {
    const r = section.by?.time?.range;
    if (typeof r === 'object') setTime({ range: { ...r, ...patch } });
  };

  const num = (v: string) => (v.trim() === '' ? undefined : Math.max(0, Math.floor(Number(v))) || undefined);
  /** What is typed for Keep: a number, or the count of a date range as text (the engine says if it is not one the board has). */
  const keep = (v: string) => (/^\s*\d*\s*$/.test(v) ? num(v) : v.trim());
  /** The step each range's largest group is now, from the dates the board has (or has been given on the page). */
  const groups = $derived(largestGroups(ranges, zoneNow(store.config?.config.settings)));
  /** `Largest group (currently week)`; with several ranges, whose. */
  const groupLabel = (name: string) =>
    `Largest group${ranges.length > 1 ? ` of ${rangeLabel(name)}` : ''} (currently ${groups[name] ?? 'not a date range yet'})`;
  const uid = $props.id();
</script>

<div class="form">
  <label class="wide">
    Title
    <input value={section.title} oninput={(e) => set({ title: e.currentTarget.value })} />
  </label>

  <label>
    Shows
    <select value={section.type} onchange={(e) => pickType(e.currentTarget.value as SectionType)}>
      {#each SECTION_TYPES as t (t.id)}<option value={t.id}>{t.label}</option>{/each}
    </select>
  </label>

  {#if section.type === 'table'}
    <label>
      Report
      <select value={section.report ?? 'next'} onchange={(e) => set({ report: e.currentTarget.value })}>
        {#each store.offeredReports as r (r.name)}<option value={r.name}>{r.name}</option>{/each}
      </select>
    </label>
  {:else}
    <label>
      Measure
      <select value={section.measure.fn} onchange={(e) => pickFn(e.currentTarget.value as Fn)}>
        {#each FNS as f (f.id)}<option value={f.id}>{f.label}</option>{/each}
      </select>
    </label>
    {#if section.measure.fn !== 'count'}
      <label>
        Field
        <input
          list="{uid}-measurable"
          value={section.measure.field ?? ''}
          oninput={(e) => set({ measure: { fn: section.measure.fn, field: e.currentTarget.value.trim() } })}
        />
        <datalist id="{uid}-measurable"
          >{#each MEASURABLE as n (n)}<option value={n}></option>{/each}</datalist
        >
      </label>
    {/if}
  {/if}

  {#if chart}
    <label>
      Group by
      <select value={grouping} onchange={(e) => pickGrouping(e.currentTarget.value as 'field' | 'time')}>
        <option value="field">The value of</option>
        <option value="time">A date, by</option>
      </select>
    </label>
    {#if section.by?.time}
      <label>
        Date
        <select value={section.by.time.field} onchange={(e) => setTime({ field: e.currentTarget.value })}>
          {#each DATES as d (d)}<option value={d}>{d}</option>{/each}
        </select>
      </label>
      <label>
        Per
        <select value={section.by.time.step} onchange={(e) => setTime({ step: e.currentTarget.value })}>
          {#each STEPS as s (s)}<option value={s}>{s}</option>{/each}
          {#each ranges as r (r.name)}
            <option value="${r.name}.lg_group.name">{groupLabel(r.name)}</option>
          {/each}
        </select>
      </label>
      <label>
        Over
        <select value={overKind} onchange={(e) => pickOver(e.currentTarget.value)}>
          <option value="">As far as the tasks go</option>
          {#each ranges as r (r.name)}
            <option value="range:{r.name}">{rangeLabel(r.name)} (the board’s date range)</option>
          {/each}
          <option value="own">Dates of its own…</option>
        </select>
      </label>
      {#if typeof section.by.time.range === 'object'}
        <label>
          From
          <input
            class="mono"
            placeholder="sow, 2026-10-01"
            spellcheck="false"
            autocomplete="off"
            value={section.by.time.range.start}
            oninput={(e) => setOwn({ start: e.currentTarget.value })}
          />
        </label>
        <label>
          To
          <input
            class="mono"
            placeholder="eow, 2026-10-31"
            spellcheck="false"
            autocomplete="off"
            value={section.by.time.range.end}
            oninput={(e) => setOwn({ end: e.currentTarget.value })}
          />
        </label>
      {/if}
    {:else}
      <label>
        Attribute
        <input
          list="{uid}-groupable"
          value={section.by?.field ?? ''}
          oninput={(e) => set({ by: { field: e.currentTarget.value.trim() } })}
        />
        <datalist id="{uid}-groupable"
          >{#each GROUPABLE as n (n)}<option value={n}></option>{/each}</datalist
        >
      </label>
    {/if}
    {#if section.type === 'heatmap'}
      <label>
        Rows
        <select value={rowsKind} onchange={(e) => pickRows(e.currentTarget.value)}>
          <option value="">A row for each series</option>
          {#each ROW_PARTS as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
          <option value="field">The value of</option>
        </select>
      </label>
      {#if section.rows?.time}
        <label>
          Date of the rows
          <select
            value={section.rows.time.field}
            onchange={(e) => setRows({ time: { ...section.rows!.time!, field: e.currentTarget.value } })}
          >
            {#each DATES as d (d)}<option value={d}>{d}</option>{/each}
          </select>
        </label>
      {:else if section.rows}
        <label>
          Attribute of the rows
          <input
            list="{uid}-groupable"
            value={section.rows.field ?? ''}
            oninput={(e) => setRows({ field: e.currentTarget.value.trim() })}
          />
        </label>
      {/if}
    {/if}
    <label>
      Order
      <select value={section.sort ?? ''} onchange={(e) => set({ sort: e.currentTarget.value || undefined })}>
        <option value="">{section.by?.time ? 'In time order' : 'Largest first'}</option>
        <option value="value-">Largest first</option>
        <option value="value+">Smallest first</option>
        <option value="key+">By name, A to Z</option>
        <option value="key-">By name, Z to A</option>
      </select>
    </label>
    <label>
      Keep
      <input
        class="mono"
        placeholder="all, 12 or $range.day_count"
        title="A number, or how many minutes, hours, days, weeks, months, quarters or years a date range of the board covers: $range.minute_count, .hour_count, .day_count, .week_count, .month_count, .quarter_count, .year_count"
        spellcheck="false"
        autocomplete="off"
        value={section.limit ?? ''}
        oninput={(e) => set({ limit: keep(e.currentTarget.value) })}
      />
    </label>
  {/if}

  {#if manySeries}
    <fieldset class="wide series">
      <legend>Series</legend>
      <p class="dim hint">
        Each series is its own line or set of bars, of the tasks its filter picks (added to the filters above). With
        none, the chart is the one measure above. When they are stacked, the first is at the bottom and the next on it:
        the arrows change the order. In a heatmap with rows a series is a colour and a grid of its own, or, stacked,
        they are added into one.
      </p>
      {#each series as x, i (i)}
        <div class="series-item">
          <div class="row-series">
            <input
              class="label"
              aria-label="Label of series {i + 1}"
              placeholder="Label"
              value={x.label}
              oninput={(e) => patchSeries(i, { label: e.currentTarget.value })}
            />
            <div class="grow">
              <FilterInput
                bind:value={() => x.filter ?? '', (v) => patchSeries(i, { filter: v || undefined })}
                id="series-filter-{section.id}-{i}"
                placeholder="Which tasks: outcome:failed"
              />
            </div>
          </div>
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
                    {#each DATES as d (d)}<option value={d}>{d}</option>{/each}
                  </select>
                </label>
                <label>
                  Until
                  <select value={x.span.to ?? ''} onchange={(e) => patchSpan(i, { to: e.currentTarget.value })}>
                    <option value="">never</option>
                    {#each DATES as d (d)}<option value={d}>{d}</option>{/each}
                  </select>
                </label>
                <label>
                  Unless
                  <select value={x.span.unless ?? ''} onchange={(e) => patchSpan(i, { unless: e.currentTarget.value })}>
                    <option value="">nothing</option>
                    {#each DATES as d (d)}<option value={d}>{d} has passed</option>{/each}
                  </select>
                </label>
              {/if}
            </div>
          {/if}
          <ColorPicker
            value={x.color}
            position={i}
            label="Colour of series {i + 1}"
            onchange={(color) => patchSeries(i, { color })}
          />
          <div class="row-series">
            <button
              aria-label="Move series {i + 1} up"
              title="Move earlier: lower in a stack"
              disabled={i === 0}
              onclick={() => setSeries(moveSeries(series, i, -1))}><ArrowUp size={14} /></button
            >
            <button
              aria-label="Move series {i + 1} down"
              title="Move later: higher in a stack"
              disabled={i === series.length - 1}
              onclick={() => setSeries(moveSeries(series, i, 1))}><ArrowDown size={14} /></button
            >
            <button
              class="danger"
              aria-label="Remove series {i + 1}"
              onclick={() => setSeries(series.filter((_, j) => j !== i))}><Trash2 size={14} /> Remove series</button
            >
          </div>
        </div>
      {/each}
      <div class="row-series">
        <button onclick={addSeries} disabled={series.length >= MAX_SERIES}><Plus size={14} /> Add series</button>
        {#if (series.length > 1 || section.stacked) && (section.type !== 'heatmap' || section.rows)}
          <label class="check">
            <input
              type="checkbox"
              checked={!!section.stacked}
              onchange={(e) => set({ stacked: e.currentTarget.checked || undefined })}
            />
            Stacked
          </label>
        {/if}
      </div>
    </fieldset>
  {/if}

  <div class="wide filter">
    <span aria-hidden="true">Filter</span>
    <FilterInput
      bind:value={() => section.filter ?? '', (v) => set({ filter: v || undefined })}
      id="section-filter-{section.id}"
      placeholder="Which tasks: project:Home +work  (added to the board's filter)"
    />
    <label class="check">
      <input
        type="checkbox"
        checked={!!section.ignoreBoardFilter}
        onchange={(e) => set({ ignoreBoardFilter: e.currentTarget.checked || undefined })}
      />
      Ignore the board's filter
    </label>
  </div>
</div>

<style>
  .form {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 10px 12px;
    align-items: end;
  }
  label,
  .filter {
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
  .row-series .label {
    width: 130px;
    flex: none;
    font-size: 14px;
    color: var(--text);
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
  .series-item {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--line);
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
</style>

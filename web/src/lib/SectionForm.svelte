<script lang="ts">
  import FilterInput from './FilterInput.svelte';
  import {
    FNS,
    SECTION_TYPES,
    STEPS,
    isChartType,
    isRoundType,
    isStackableType,
    largestGroups,
    ROW_PARTS,
    controlLabel,
    zoneNow,
    dateRanges,
    type Control,
    type Fn,
    type RowPart,
    type Rows,
    type Section,
    type SectionType,
    type StackBy,
  } from './boards';
  import SectionGroups from './SectionGroups.svelte';
  import SectionSeries from './SectionSeries.svelte';
  import { store } from './store.svelte';

  /** The settings of one board section. It edits a copy and reports it; saving is the board's. */
  let {
    section,
    onchange,
    controls = [],
  }: { section: Section; onchange: (next: Section) => void; controls?: Control[] } = $props();
  /** The board's date ranges, which a chart's axis, Per and Keep can use. */
  const ranges = $derived(dateRanges(controls));

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

  const grouping = $derived(section.by?.time ? 'time' : section.by?.groups ? 'groups' : 'field');
  const chart = $derived(isChartType(section.type));
  /** Several series are for charts that draw them side by side; a pie has one, its slices being the values. */
  const manySeries = $derived(chart && !isRoundType(section.type));

  function pickType(type: SectionType) {
    const patch: Partial<Section> = { type };
    // A heatmap's columns are a date or a value, never groups the board writes.
    if (isChartType(type) && (!section.by || (type === 'heatmap' && section.by.groups))) {
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
    // Only a bar, line or area chart is stacked by a field.
    if (!isStackableType(type)) patch.stackBy = undefined;
    set(patch);
  }

  /** Stacked by a field, or (an empty field) not. */
  function setStack(patch: Partial<StackBy>) {
    const next = { ...(section.stackBy ?? { field: '' }), ...patch };
    set({ stackBy: next.field ? next : undefined });
  }

  function pickFn(fn: Fn) {
    set({ measure: fn === 'count' ? { fn } : { fn, field: section.measure.field ?? MEASURABLE[0] } });
  }

  function pickGrouping(kind: 'field' | 'time' | 'groups') {
    set({
      by:
        kind === 'time'
          ? { time: { field: 'entry', step: 'week' } }
          : kind === 'groups'
            ? { groups: [{ label: 'Group 1' }] }
            : { field: 'project' },
    });
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

  /** The series, whose count Stack by needs (several series and Stack by are one or the other). */
  const series = $derived(section.series ?? []);

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
  /** Groups written with commas between them; none is undefined, which is not written. */
  const groupList = (v: string) => {
    const got = [...new Set(v.split(',').map((g) => g.trim()))].filter(Boolean);
    return got.length ? got : undefined;
  };
  /** The step each range's largest group is now, from the dates the board has (or has been given on the page). */
  const groups = $derived(largestGroups(controls, zoneNow(store.config?.config.settings)));
  /** `Largest group (currently week)`; with several ranges, whose. */
  const groupLabel = (r: Control) =>
    `Largest group${ranges.length > 1 ? ` of ${controlLabel(r)}` : ''} (currently ${groups[r.name] ?? 'not a date range yet'})`;
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
      <select value={grouping} onchange={(e) => pickGrouping(e.currentTarget.value as 'field' | 'time' | 'groups')}>
        <option value="field">The value of</option>
        <option value="time">A date, by</option>
        {#if section.type !== 'heatmap'}<option value="groups">Groups you write</option>{/if}
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
            <option value="${r.name}.lg_group.name">{groupLabel(r)}</option>
          {/each}
        </select>
      </label>
      <label>
        Over
        <select value={overKind} onchange={(e) => pickOver(e.currentTarget.value)}>
          <option value="">As far as the tasks go</option>
          {#each ranges as r (r.name)}
            <option value="range:{r.name}">{controlLabel(r)} (the board’s date range)</option>
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
    {:else if !section.by?.groups}
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
    {#if isStackableType(section.type)}
      <label>
        Stack by
        <input
          list="{uid}-groupable"
          placeholder="nothing; or project"
          title="A series of each value of this attribute, stacked; it takes the place of several series"
          value={section.stackBy?.field ?? ''}
          disabled={series.length > 1}
          oninput={(e) => setStack({ field: e.currentTarget.value.trim() })}
        />
      </label>
      {#if section.stackBy}
        <label>
          Stack only
          <input
            class="mono"
            placeholder="all; or home, work"
            title="The values stacked, separated by commas; none stacks them all"
            spellcheck="false"
            autocomplete="off"
            value={section.stackBy.only?.join(', ') ?? ''}
            onchange={(e) => setStack({ only: groupList(e.currentTarget.value) })}
          />
        </label>
        <label>
          Stack leaving out
          <input
            class="mono"
            placeholder="none; or (none)"
            title="The values left out of the stack, separated by commas"
            spellcheck="false"
            autocomplete="off"
            value={section.stackBy.except?.join(', ') ?? ''}
            onchange={(e) => setStack({ except: groupList(e.currentTarget.value) })}
          />
        </label>
      {/if}
    {/if}
    <label>
      Order
      <select value={section.sort ?? ''} onchange={(e) => set({ sort: e.currentTarget.value || undefined })}>
        <option value=""
          >{section.by?.time ? 'In time order' : section.by?.groups ? 'In their order' : 'Largest first'}</option
        >
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
    {#if section.by && !section.by.time}
      <label>
        Show only
        <input
          class="mono"
          placeholder="all; or feature, bugfix"
          title="The groups to show, separated by commas; none shows them all"
          spellcheck="false"
          autocomplete="off"
          value={section.only?.join(', ') ?? ''}
          onchange={(e) => set({ only: groupList(e.currentTarget.value) })}
        />
      </label>
      <label>
        Leave out
        <input
          class="mono"
          placeholder="none; or jira, work"
          title="The groups to leave out, separated by commas"
          spellcheck="false"
          autocomplete="off"
          value={section.except?.join(', ') ?? ''}
          onchange={(e) => set({ except: groupList(e.currentTarget.value) })}
        />
      </label>
    {/if}
  {/if}

  {#if chart && section.by?.groups}
    <SectionGroups {section} {set} />
  {/if}

  {#if manySeries}
    <SectionSeries {section} {set} dates={DATES} />
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
  .check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
</style>

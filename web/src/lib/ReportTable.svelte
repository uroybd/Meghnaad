<script lang="ts">
  import { untrack } from 'svelte';
  import { formatFor } from './dateformat';
  import { formatTable } from './format';
  import { ArrowDown, ArrowUp, Download, Rows3 } from './icons';
  import LazyView from './LazyView.svelte';
  import { lazyRows, MORE_ROWS } from './lazyRows.svelte';
  import { DIALOGS } from './pages';
  import { exportArgs } from './exportScope';
  import ReportRows from './ReportRows.svelte';
  import { groupHeads } from './groups';
  import { baseColumn, groupColumn, parseSort, SORTABLE, sortState } from './sortSpec';
  import { selection } from './selection.svelte';
  import { store, type Entry } from './store.svelte';
  import type { ReportResult, Row } from './types';

  let {
    result,
    entry = null,
    onedit,
    onsort,
    ongroup,
    ontag,
    activeTags = [],
    selectable = false,
  }: {
    result: ReportResult;
    entry?: Entry | null;
    onedit?: (row: Row) => void;
    /** Provided for the focused report only: header clicks sort it through the command line. */
    onsort?: (column: string, shift: boolean) => void;
    /** Provided for the focused report only: choose the column the table is grouped by (null: none). */
    ongroup?: (column: string | null) => void;
    /** Provided for the focused report only: a tag chip toggles that tag in the report's filter. Without it a chip opens the tag's entry on the Tags page. */
    ontag?: (tag: string) => void;
    /** The tags the focused report's filter already requires, shown as pressed chips. */
    activeTags?: string[];
    /** Provided for the focused report only: a checkbox on each row, to select several for the bulk actions. */
    selectable?: boolean;
  } = $props();

  // What the Export button exports: the tasks this table shows (its filter, report and context), when the command
  // line that drew it can be told.
  const exportLine = $derived(
    entry ? exportArgs(entry.input, entry.scope && store.contextChoice(entry.scope), store.config?.config) : null,
  );
  let exporting = $state(false);

  const udas = $derived(store.config?.config.udas ?? {});
  const settings = $derived(store.config?.config.settings);
  const reportFormat = $derived(
    (store.config?.config.reports[result.report] as { dateformat?: string | null } | undefined)?.dateformat,
  );
  const ctx = $derived({
    now: store.now,
    udas,
    journal: store.config?.journal ?? null,
    dates: { report: formatFor('report', settings, reportFormat), annotation: formatFor('annotation', settings) },
    recurIndicator: settings?.['recurrence.indicator'] || undefined,
    indicators: {
      active: settings?.['active.indicator'],
      tag: settings?.['tag.indicator'],
      dependency: settings?.['dependency.indicator'],
    },
    transformations: store.rules,
    idFallback: store.appearance.idFallback,
  });
  // What every cell shows, worked out by the engine for the whole table at once.
  const table = $derived(formatTable(result.columns, result.rows, ctx));
  // The columns shown: without `print.empty.columns` (Taskwarrior's default) one with nothing in it is left out, as the real
  // `task` does. A table with no rows keeps its columns, so it still reads as a table.
  const shown = $derived(
    result.columns
      .map((col, ci) => ({ col, ci }))
      .filter(({ ci }) => result.print_empty_columns || !result.rows.length || !table.empty[ci]),
  );
  const keys = $derived(parseSort(result.sort));
  // A header above each group, when the sort has a `/`: what its tasks have in common.
  const heads = $derived(groupHeads(result, ctx));
  const grouped = $derived(groupColumn(keys));

  // A table wider than the window scrolls sideways; its Actions column then stays at the right edge, so the buttons
  // (and the ⋯ menu) are never out of reach. Only then: a table that fits is laid out as it always was. If it is wider
  // still with the buttons in it, they fold into the ⋯ menu, so the column is one small button and the table has the room.
  let wrap = $state<HTMLElement>();
  let pinned = $state(false);
  let folded = $state(false);
  // How wide the column is with its buttons out; once they are folded, that is what it would grow back to.
  let full = 0;
  $effect(() => {
    const el = wrap;
    if (!el || typeof ResizeObserver === 'undefined') return;
    const check = () => {
      const column = el.querySelector<HTMLElement>('td.actions')?.offsetWidth ?? 0;
      const over = el.scrollWidth > el.clientWidth + 1;
      if (!folded) {
        full = column;
        if (over && column > 0 && matchMedia('(min-width: 761px)').matches) folded = true;
      } else if (full > 0 && el.scrollWidth + (full - column) <= el.clientWidth + 1) {
        folded = false;
      }
      pinned = el.scrollWidth > el.clientWidth + 1;
    };
    const watch = new ResizeObserver(() => untrack(check));
    watch.observe(el);
    if (el.firstElementChild) watch.observe(el.firstElementChild); // the table: its width follows what is in it
    untrack(check);
    return () => watch.disconnect();
  });

  const sortable = (name: string) => !!onsort && (SORTABLE.has(baseColumn(name)) || baseColumn(name) in udas);

  // Phones show cards instead of a table, so there are no headers to click: a sort picker
  // stands in for them (same command underneath).
  const sortColumns = $derived(shown.map((s) => s.col).filter((c) => sortable(c.name)));
  const primary = $derived(keys[0] ?? null);

  // The top of a long report is drawn first, and more as the end of it nears the window (`lazyRows`).
  let tail = $state<HTMLElement>();
  const lazy = lazyRows(
    () => result.rows,
    () => `${result.report}\0${result.sort ?? ''}`,
    () => tail,
  );

  // The select-all box: ticked when every row is, half when some are.
  const allUuids = $derived(result.rows.map((r) => r.uuid));
  const ticked = $derived(allUuids.filter((u) => selection.has(u)).length);
  let allBox = $state<HTMLInputElement | undefined>();
  $effect(() => {
    if (allBox) allBox.indeterminate = ticked > 0 && ticked < allUuids.length;
  });
  const ariaSort = (name: string) => {
    const s = sortState(keys, name);
    return s ? (s.desc ? 'descending' : 'ascending') : 'none';
  };
</script>

{#if result.description}
  <p class="dim desc">{result.description}</p>
{/if}

{#if result.rows.length === 0}
  <p class="dim empty">No tasks match.</p>
{:else}
  {#if onsort && sortColumns.length}
    <div class="sortbar row">
      <label class="dim" for="rt-sort">Sort</label>
      <select
        id="rt-sort"
        value={primary?.column ?? ''}
        onchange={(e) => e.currentTarget.value && onsort(e.currentTarget.value, false)}
      >
        {#if !primary}<option value="">report default</option>{/if}
        {#each sortColumns as c (c.spec)}<option value={baseColumn(c.name)}>{c.label}</option>{/each}
      </select>
      <button
        class="dir"
        disabled={!primary}
        aria-label={primary
          ? `Sorted ${primary.desc ? 'descending' : 'ascending'}; change direction`
          : 'Sort direction'}
        onclick={() => primary && onsort(primary.column, false)}
      >
        {#if primary?.desc}<ArrowDown size={16} />{:else}<ArrowUp size={16} />{/if}
      </button>
      {#if ongroup}
        <label class="dim" for="rt-group">Group</label>
        <select id="rt-group" value={grouped ?? ''} onchange={(e) => ongroup(e.currentTarget.value || null)}>
          <option value="">none</option>
          {#each sortColumns as c (c.spec)}<option value={baseColumn(c.name)}>{c.label}</option>{/each}
        </select>
      {/if}
    </div>
  {/if}
  <div class="wrap" class:pinned bind:this={wrap}>
    <table>
      <thead>
        <tr>
          {#if selectable}
            <th class="sel">
              <input
                type="checkbox"
                bind:this={allBox}
                checked={allUuids.length > 0 && ticked === allUuids.length}
                aria-label="Select all {allUuids.length} tasks"
                onchange={() => selection.toggleAll(allUuids)}
              />
            </th>
          {/if}
          {#each shown as { col } (col.spec)}
            {@const st = sortState(keys, col.name)}
            <th class={col.kind} aria-sort={sortable(col.name) ? ariaSort(col.name) : undefined}>
              {#if sortable(col.name)}
                <button
                  class="sort ghost"
                  class:on={!!st}
                  title="Sort by {col.label}. Click again to reverse, a third time to reset; Shift-click adds a tie-breaker."
                  onclick={(e) => onsort?.(col.name, e.shiftKey)}
                >
                  {col.label}
                  {#if st}
                    <span class="arrow" aria-hidden="true">
                      {#if st.desc}<ArrowDown size={12} />{:else}<ArrowUp size={12} />{/if}{st.rank ?? ''}
                    </span>
                  {/if}
                </button>
                {#if ongroup}
                  <button
                    class="grp ghost"
                    class:on={st?.group}
                    aria-pressed={!!st?.group}
                    title={st?.group ? `Stop grouping by ${col.label}` : `Group by ${col.label}`}
                    aria-label={st?.group ? `Stop grouping by ${col.label}` : `Group by ${col.label}`}
                    onclick={() => ongroup(baseColumn(col.name))}><Rows3 size={12} /></button
                  >
                {/if}
              {:else}
                {col.label}
                {#if st}
                  <span class="arrow dim" aria-hidden="true">
                    {#if st.desc}<ArrowDown size={12} />{:else}<ArrowUp size={12} />{/if}{st.rank ?? ''}
                  </span>
                {/if}
              {/if}
              {#if st?.group && !ongroup}
                <span class="grpmark" title="The table is grouped by {col.label}"><Rows3 size={12} /></span>
              {/if}
            </th>
          {/each}
          <th class="actions"><span class="sr-only">Actions</span></th>
        </tr>
      </thead>
      <ReportRows
        {result}
        rows={lazy.rows}
        {shown}
        cells={table.cells}
        {heads}
        {entry}
        {selectable}
        {activeTags}
        {ontag}
        {onedit}
        {folded}
      />
    </table>
  </div>
  <p class="dim count" bind:this={tail}>
    {result.rows.length === result.matched
      ? `${result.matched} ${result.matched === 1 ? 'task' : 'tasks'}`
      : `showing ${result.rows.length} of ${result.matched}`}
    {#if lazy.drawn < lazy.total}
      · {lazy.drawn} shown so far
      <button type="button" class="ghost more" onclick={lazy.more}
        >Show {Math.min(MORE_ROWS, lazy.total - lazy.drawn)} more</button
      >
      <button type="button" class="ghost more" onclick={lazy.all}>Show all</button>
    {/if}
    {#if exportLine}
      <button
        type="button"
        class="ghost export"
        onclick={() => (exporting = true)}
        title="Export these tasks as JSON, CSV or TSV"
        aria-label="Export these tasks"><Download size={13} /> Export</button
      >
    {/if}
  </p>
  {#if exporting && exportLine}
    <LazyView
      of={DIALOGS.export}
      props={{ args: exportLine, columns: result.columns, onclose: () => (exporting = false) }}
    />
  {/if}
{/if}

<style>
  /* Positioned, so the visually hidden header label stays inside it instead of widening the page. */
  .wrap {
    overflow-x: auto;
    position: relative;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  /* The headers; the rows are `ReportRows`, their cells `ReportCell`, a row's buttons `RowActions`. */
  th {
    text-align: left;
    padding: 9px 18px 10px 0;
    vertical-align: top;
    white-space: nowrap;
    color: var(--dim);
    font-weight: 500;
    font-size: 13px;
    border-bottom: 1px solid var(--line);
  }
  th .sort {
    padding: 0 4px;
    margin-left: -4px;
    color: inherit;
    font-size: inherit;
    font-weight: inherit;
    border-radius: 4px;
  }
  th .sort:hover {
    color: var(--text);
    background: var(--panel-2);
  }
  th .sort.on {
    color: var(--accent);
    font-weight: 700;
  }
  /* The button that groups the table by this column, and the mark on the column that it is grouped by. */
  th .grp {
    padding: 0 3px;
    line-height: 0;
    color: var(--dim);
    opacity: 0;
    transition: opacity 0.12s;
  }
  th:hover .grp,
  th .grp:focus-visible,
  th .grp.on {
    opacity: 1;
  }
  th .grp.on,
  .grpmark {
    color: var(--accent);
  }
  .grpmark {
    display: inline-flex;
    vertical-align: middle;
    margin-left: 2px;
  }
  .arrow {
    display: inline-flex;
    align-items: center;
    font-size: 10px;
    margin-left: 2px;
    vertical-align: middle;
  }
  th.description {
    white-space: normal;
    min-width: 16em;
  }
  @media (min-width: 761px) {
    .pinned th.actions {
      position: sticky;
      right: 0;
      z-index: 2;
      background: var(--bg);
    }
  }
  th.sel {
    width: 1%;
    padding-right: 6px;
  }
  th.sel input {
    cursor: pointer;
  }
  /* Room for the selection bar (and the hover tint) so the first column never touches it. */
  @media (min-width: 761px) {
    th:first-child {
      padding-left: 12px;
    }
  }
  .actions {
    text-align: right;
    white-space: nowrap;
  }
  .desc,
  .empty,
  .count {
    margin: 8px 0;
  }
  .more {
    margin-left: 6px;
    text-decoration: underline;
  }
  .export {
    margin-left: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--dim);
    opacity: 0.8;
  }
  .export:hover,
  .export:focus-visible {
    opacity: 1;
    color: var(--text);
  }
  .sortbar {
    display: none;
    margin: 0 0 8px;
  }
  .sortbar select {
    flex: 1;
    min-width: 0;
  }
  .sortbar .dir + label {
    margin-left: 6px;
  }

  /* Phones: each task is a card (`ReportRows`), so there are no headers; the sort bar stands in for them. */
  @media (max-width: 760px) {
    .sortbar {
      display: flex;
    }
    .desc {
      display: none;
    }
    .wrap {
      overflow: visible;
    }
    table {
      display: block;
    }
    thead {
      display: none;
    }
    th.sel {
      display: none;
    }
  }
</style>

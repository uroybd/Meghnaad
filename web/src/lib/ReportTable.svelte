<script lang="ts">
  import { formatFor } from './dateformat';
  import { css, look, rowVars } from './colors';
  import { cell, rowClass } from './format';
  import { ArrowDown, ArrowUp, Check, Pencil, Play, Repeat, Rows3, Square, Trash2 } from './icons';
  import ProjectPath from './ProjectPath.svelte';
  import TagChip from './TagChip.svelte';
  import DepLink from './DepLink.svelte';
  import UuidTip from './UuidTip.svelte';
  import { describeRecur } from './recurrence';
  import { groupHeads } from './groups';
  import { scheme } from './scheme.svelte';
  import { baseColumn, groupColumn, parseSort, SORTABLE, sortState } from './sortSpec';
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
  } = $props();

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
  });
  const keys = $derived(parseSort(result.sort));
  // A header above each group, when the sort has a `/`: what its tasks have in common.
  const heads = $derived(groupHeads(result, ctx));
  const grouped = $derived(groupColumn(keys));
  let pendingDelete = $state<string | null>(null);

  const sortable = (name: string) => !!onsort && (SORTABLE.has(baseColumn(name)) || baseColumn(name) in udas);

  function act(row: Row, ...rest: string[]) {
    store.act(entry, [row.uuid, ...rest]);
  }

  // The second click is the answer to Taskwarrior's "Delete task N?" (the `confirmation` setting);
  // with it off, the first click deletes.
  function del(row: Row) {
    if (!store.confirmation || pendingDelete === row.uuid) {
      pendingDelete = null;
      store.act(entry, [row.uuid, 'delete'], { approved: [row.uuid] });
    } else {
      pendingDelete = row.uuid;
      setTimeout(() => pendingDelete === row.uuid && (pendingDelete = null), 3000);
    }
  }

  // A click anywhere on the row opens the detail view, except on controls and selected text.
  function open(e: MouseEvent | KeyboardEvent, row: Row) {
    const t = e.target as HTMLElement;
    if (t.closest('button, a, input, select, textarea')) return;
    if (e instanceof MouseEvent && window.getSelection()?.toString()) return;
    store.openDetail(row.uuid, entry);
  }

  // Phones show cards instead of a table, so there are no headers to click: a sort picker
  // stands in for them (same command underneath).
  const sortColumns = $derived(result.columns.filter((c) => sortable(c.name)));
  const primary = $derived(keys[0] ?? null);

  const isOpen = (row: Row) => row.status === 'pending';
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
  <div class="wrap">
    <table>
      <thead>
        <tr>
          {#each result.columns as col (col.spec)}
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
      <tbody>
        {#each result.rows as row, i (row.uuid)}
          {#if heads[i]}
            <tr class="grouphead"><th colspan={result.columns.length + 1} scope="colgroup">{heads[i]}</th></tr>
          {/if}
          {@const l = look(row.style, scheme.dark)}
          <tr
            class={rowClass(row)}
            class:ruled={!!l}
            style={rowVars(l)}
            class:gap={result.breaks[i] && heads.length === 0}
            class:selected={store.detail?.uuid === row.uuid}
            tabindex="0"
            aria-label="Open details: {row.description}"
            onclick={(e) => open(e, row)}
            onkeydown={(e) => e.key === 'Enter' && e.target === e.currentTarget && open(e, row)}
          >
            {#each result.columns as col (col.spec)}
              {@const c = cell(col, row, ctx)}
              <td
                class="{col.kind} {c.cls ?? ''}"
                class:blank={!c.text && !c.lines?.length}
                data-label={col.label}
                style={col.name === 'priority' && row.priority
                  ? css(look(store.config?.colors?.[`uda.priority.${row.priority}`], scheme.dark))
                  : undefined}
              >
                {#if c.uuid}
                  <UuidTip text={c.text} uuid={c.uuid} />
                {:else if c.links}
                  {#each c.links as u (u)}<DepLink uuid={u} />{/each}
                {:else if c.segments}
                  <ProjectPath segments={c.segments} />
                {:else if c.chips}
                  {#each c.chips as t (t)}<TagChip tag={t} active={activeTags.includes(t)} onclick={ontag} />{/each}
                {:else}
                  {c.text}
                {/if}
                {#if col.name === 'description' && row.recur}
                  <span
                    class="repeat"
                    title={row.status === 'recurring'
                      ? `Recurring task: repeats ${describeRecur(row.recur)}`
                      : `Repeats ${describeRecur(row.recur)}`}
                  >
                    <Repeat size={12} /><span class="sr-only"
                      >{row.status === 'recurring' ? 'recurring' : 'repeats'} {describeRecur(row.recur)}</span
                    >
                  </span>
                {/if}
                {#if col.name === 'description' && row.orphans.length}
                  <span
                    class="chip orphan"
                    title="Has properties your taskrc doesn't define: {row.orphans.join(', ')} (read-only)"
                  >
                    +{row.orphans.length}
                  </span>
                {/if}
                {#each c.lines ?? [] as line, _i (_i)}
                  <div class="note dim">{line}</div>
                {/each}
              </td>
            {/each}
            <td class="actions">
              {#if isOpen(row)}
                <button class="ghost" title="Done" aria-label="Done" onclick={() => act(row, 'done')}
                  ><Check size={16} /></button
                >
                {#if row.start != null}
                  <button class="ghost" title="Stop" aria-label="Stop" onclick={() => act(row, 'stop')}
                    ><Square size={14} fill="currentColor" /></button
                  >
                {:else}
                  <button class="ghost" title="Start" aria-label="Start" onclick={() => act(row, 'start')}
                    ><Play size={14} /></button
                  >
                {/if}
              {/if}
              {#if onedit}
                <button class="ghost" title="Edit" aria-label="Edit" onclick={() => onedit(row)}
                  ><Pencil size={14} /></button
                >
              {/if}
              {#if row.status !== 'deleted'}
                <button
                  class="ghost danger"
                  title={pendingDelete === row.uuid ? 'Click again to delete' : 'Delete'}
                  aria-label="Delete"
                  onclick={() => del(row)}
                >
                  {#if pendingDelete === row.uuid}sure?{:else}<Trash2 size={14} />{/if}
                </button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <p class="dim count">
    {result.rows.length === result.matched
      ? `${result.matched} ${result.matched === 1 ? 'task' : 'tasks'}`
      : `showing ${result.rows.length} of ${result.matched}`}
  </p>
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
  th,
  td {
    text-align: left;
    padding: 9px 18px 9px 0;
    vertical-align: top;
    white-space: nowrap;
  }
  tbody td {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  th {
    color: var(--dim);
    font-weight: 500;
    font-size: 13px;
    padding-bottom: 10px;
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
  /* The header above a group of rows. */
  tr.grouphead,
  tr.grouphead:hover {
    cursor: default;
    background: transparent;
  }
  tr.grouphead th {
    padding: 14px 0 4px;
    border-bottom: 1px solid var(--accent);
    color: var(--accent);
    font-size: 13px;
    font-weight: 700;
    text-align: left;
    white-space: normal;
  }
  .arrow {
    display: inline-flex;
    align-items: center;
    font-size: 10px;
    margin-left: 2px;
    vertical-align: middle;
  }
  td.description,
  th.description {
    white-space: normal;
    min-width: 16em;
  }
  td.id,
  td.number {
    font-variant-numeric: tabular-nums;
  }
  td.priority {
    font-weight: 600;
  }
  td.pri-h,
  .pri-h {
    color: var(--pri-h);
  }
  td.pri-m,
  .pri-m {
    color: var(--pri-m);
  }
  td.pri-l,
  .pri-l {
    color: var(--pri-l);
  }
  td.id:not(.dim) {
    font-weight: 700;
  }
  td.dim {
    color: var(--dim);
  }
  tbody tr {
    cursor: pointer;
  }
  tr.gap td {
    border-top: 14px solid transparent;
  }
  /* Finished tasks are shown by colour, not struck through: green for completed, red for deleted. */
  tr.done td {
    color: var(--dim);
  }
  tr.completed td {
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }
  tr.deleted td {
    background: color-mix(in srgb, var(--err) 9%, transparent);
  }
  tr.waiting td {
    color: var(--dim);
  }
  /* The colour rules (color.active, color.overdue, ...) lay their colour on the whole row. */
  tr.ruled td {
    color: var(--row-fg, inherit);
    background: var(--row-bg, transparent);
    font-weight: var(--row-weight, inherit);
    text-decoration: var(--row-deco, none);
  }
  tbody tr:hover {
    background: var(--panel-2);
  }
  tr.selected {
    background: var(--panel-2);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  /* Room for the selection bar (and the hover tint) so the first column never touches it. */
  @media (min-width: 761px) {
    th:first-child,
    td:first-child {
      padding-left: 12px;
    }
  }
  tr:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .note {
    font-size: 13px;
    margin-left: 1em;
    white-space: pre-wrap;
  }
  td.multiline {
    white-space: pre-line;
  }
  .actions {
    text-align: right;
    white-space: nowrap;
  }
  .actions button {
    padding: 3px 6px;
    line-height: 0;
  }
  .actions button:has(:not(svg)) {
    line-height: inherit;
  }
  .orphan {
    margin-left: 6px;
    color: var(--dim);
  }
  .repeat {
    display: inline-flex;
    margin-left: 6px;
    color: var(--dim);
    vertical-align: -1px;
  }
  .desc,
  .empty,
  .count {
    margin: 8px 0;
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

  /* Phones: each task is a card. The first line is the description; the other columns follow as
     small "label value" pairs, and the buttons get their own row. */
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
    table,
    tbody {
      display: block;
    }
    thead {
      display: none;
    }
    tbody tr {
      position: relative;
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      gap: 2px 14px;
      padding: 10px 12px 6px;
      margin-bottom: 8px;
      border: 1px solid var(--line);
      border-radius: var(--radius);
      background: var(--panel);
    }
    tr.gap {
      margin-top: 18px;
    }
    tr.grouphead {
      display: block;
      margin: 14px 0 6px;
      padding: 0;
      border: 0;
      background: transparent;
    }
    tr.grouphead th {
      display: block;
      padding: 0 2px 4px;
    }
    tr.gap td {
      border-top: 0;
    }
    td {
      display: inline-flex;
      flex-wrap: wrap;
      gap: 4px;
      padding: 0;
      font-size: 13px;
      white-space: normal;
      color: var(--dim);
    }
    td::before {
      content: attr(data-label);
      font-size: 12px;
      text-transform: uppercase;
      letter-spacing: 0.04em;
      opacity: 0.8;
      align-self: center;
    }
    td.blank {
      display: none;
    }
    td.description {
      order: -1;
      flex-basis: 100%;
      font-size: 15px;
      color: var(--text);
      padding-right: 2.4em;
      min-width: 0;
    }
    td.description::before,
    td.actions::before {
      display: none;
    }
    td.id {
      position: absolute;
      top: 10px;
      right: 12px;
      font-size: 12px;
    }
    td.id::before {
      content: '#';
      opacity: 0.6;
    }
    td.priority.pri-h {
      color: var(--pri-h);
    }
    tr.done td.description,
    tr.done td {
      color: var(--dim);
    }
    /* A card is one box: tint the box, not each piece of it. */
    tr.completed,
    tr.deleted {
      background: color-mix(in srgb, var(--ok) 9%, var(--panel));
    }
    tr.deleted {
      background: color-mix(in srgb, var(--err) 9%, var(--panel));
    }
    tr.completed td,
    tr.deleted td,
    tr.ruled td {
      background: transparent;
    }
    /* A card is one box: the rule's background is the card's. */
    tr.ruled {
      background: var(--row-bg, var(--panel));
    }
    td.actions {
      order: 99;
      flex-basis: 100%;
      display: flex;
      justify-content: flex-end;
      gap: 4px;
      margin-top: 6px;
      padding-top: 4px;
      border-top: 1px solid var(--line);
    }
    td.actions button {
      min-width: 44px;
      min-height: 40px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .note {
      margin-left: 0;
      flex-basis: 100%;
    }
  }
</style>

<script lang="ts">
  import { cell, rowClass } from './format';
  import { ArrowDown, ArrowUp, Check, Pencil, Play, Square, Trash2 } from './icons';
  import { baseColumn, parseSort, SORTABLE, sortState } from './sortSpec';
  import { store, type Entry } from './store.svelte';
  import type { ReportResult, Row } from './types';

  let {
    result,
    entry = null,
    onedit,
    onsort,
  }: {
    result: ReportResult;
    entry?: Entry | null;
    onedit?: (row: Row) => void;
    /** Provided for the focused report only: header clicks sort it through the command line. */
    onsort?: (column: string, shift: boolean) => void;
  } = $props();

  const udas = $derived(store.config?.config.udas ?? {});
  const ctx = $derived({ now: store.now, udas, journal: store.config?.journal ?? null });
  const keys = $derived(parseSort(result.sort));
  let pendingDelete = $state<string | null>(null);

  const sortable = (name: string) => !!onsort && (SORTABLE.has(baseColumn(name)) || baseColumn(name) in udas);

  function act(row: Row, ...rest: string[]) {
    store.act(entry, [row.uuid, ...rest]);
  }

  function del(row: Row) {
    if (pendingDelete === row.uuid) {
      pendingDelete = null;
      act(row, 'delete');
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
              {:else}
                {col.label}
                {#if st}
                  <span class="arrow dim" aria-hidden="true">
                    {#if st.desc}<ArrowDown size={12} />{:else}<ArrowUp size={12} />{/if}{st.rank ?? ''}
                  </span>
                {/if}
              {/if}
            </th>
          {/each}
          <th class="actions"><span class="sr-only">Actions</span></th>
        </tr>
      </thead>
      <tbody>
        {#each result.rows as row, i (row.uuid)}
          <tr
            class={rowClass(row)}
            class:gap={result.breaks[i]}
            class:selected={store.detail?.uuid === row.uuid}
            tabindex="0"
            aria-label="Open details: {row.description}"
            onclick={(e) => open(e, row)}
            onkeydown={(e) => (e.key === 'Enter' && e.target === e.currentTarget) && open(e, row)}
          >
            {#each result.columns as col (col.spec)}
              {@const c = cell(col, row, ctx)}
              <td class="{col.kind} {c.cls ?? ''}">
                {c.text}
                {#if col.name === 'description' && row.orphans.length}
                  <span class="chip orphan" title="Has properties your taskrc doesn't define: {row.orphans.join(', ')} (read-only)">
                    +{row.orphans.length}
                  </span>
                {/if}
                {#each c.lines ?? [] as line}
                  <div class="note dim">{line}</div>
                {/each}
              </td>
            {/each}
            <td class="actions">
              {#if isOpen(row)}
                <button class="ghost" title="Done" aria-label="Done" onclick={() => act(row, 'done')}><Check size={16} /></button>
                {#if row.start != null}
                  <button class="ghost" title="Stop" aria-label="Stop" onclick={() => act(row, 'stop')}><Square size={14} fill="currentColor" /></button>
                {:else}
                  <button class="ghost" title="Start" aria-label="Start" onclick={() => act(row, 'start')}><Play size={14} /></button>
                {/if}
              {/if}
              {#if onedit}
                <button class="ghost" title="Edit" aria-label="Edit" onclick={() => onedit(row)}><Pencil size={14} /></button>
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
  .wrap { overflow-x: auto; }
  table { border-collapse: collapse; width: 100%; }
  th, td { text-align: left; padding: 4px 10px 4px 0; vertical-align: top; white-space: nowrap; }
  th { color: var(--dim); font-weight: 500; font-size: 12px; border-bottom: 1px solid var(--line); }
  th .sort { padding: 0 4px; margin-left: -4px; color: inherit; font-size: inherit; font-weight: inherit; border-radius: 4px; }
  th .sort:hover { color: var(--text); background: var(--panel-2); }
  th .sort.on { color: var(--accent); font-weight: 700; }
  .arrow { display: inline-flex; align-items: center; font-size: 10px; margin-left: 2px; vertical-align: middle; }
  td.description, th.description { white-space: normal; min-width: 14em; }
  td.id, td.number { font-variant-numeric: tabular-nums; }
  td.priority { font-weight: 600; }
  td.pri-h, .pri-h { color: var(--pri-h); }
  td.pri-m, .pri-m { color: var(--pri-m); }
  td.pri-l, .pri-l { color: var(--pri-l); }
  td.overdue { color: var(--err); font-weight: 600; }
  td.dim { color: var(--dim); }
  tbody tr { cursor: pointer; }
  tr.gap td { border-top: 14px solid transparent; }
  tr.done td { color: var(--dim); text-decoration: line-through; }
  tr.done td.actions { text-decoration: none; }
  tr.waiting td, tr.blocked td { color: var(--dim); }
  tr.active td.description { font-weight: 600; }
  tbody tr:hover { background: var(--panel-2); }
  tr.selected { background: var(--panel-2); box-shadow: inset 3px 0 0 var(--accent); }
  tr:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .note { font-size: 12px; margin-left: 1em; }
  .actions { text-align: right; white-space: nowrap; }
  .actions button { padding: 3px 6px; line-height: 0; }
  .actions button:has(:not(svg)) { line-height: inherit; }
  .orphan { margin-left: 6px; color: var(--dim); }
  .desc, .empty, .count { margin: 4px 0; }
</style>

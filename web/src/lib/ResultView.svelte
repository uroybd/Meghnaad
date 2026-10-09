<script lang="ts">
  import BurndownView from './BurndownView.svelte';
  import CalendarView from './CalendarView.svelte';
  import ConfirmView from './ConfirmView.svelte';
  import ReportTable from './ReportTable.svelte';
  import SummaryView from './SummaryView.svelte';
  import TaskInfo from './TaskInfo.svelte';
  import type { Entry } from './store.svelte';
  import type { Row } from './types';

  let {
    entry,
    onedit,
    onsort,
    ontag,
    activeTags,
  }: {
    entry: Entry;
    onedit?: (row: Row, from: Entry) => void;
    onsort?: (column: string, shift: boolean) => void;
    ontag?: (tag: string) => void;
    activeTags?: string[];
  } = $props();
  const r = $derived(entry.result);
</script>

{#if entry.failure}
  <p class="err" role="alert">{entry.failure}</p>
{:else if !r}
  <p class="dim">Running…</p>
{:else if r.kind === 'report'}
  <ReportTable result={r} {entry} onedit={onedit && ((row) => onedit(row, entry))} {onsort} {ontag} {activeTags} />
{:else if r.kind === 'info'}
  {#each r.tasks as t (t.uuid)}<TaskInfo task={t} onedit={onedit && ((row) => onedit(row, entry))} />{/each}
{:else if r.kind === 'summary'}
  <SummaryView result={r} />
{:else if r.kind === 'burndown'}
  <BurndownView result={r} />
{:else if r.kind === 'calendar'}
  <CalendarView result={r} {entry} />
{:else if r.kind === 'table'}
  {#if r.title}<p class="dim">{r.title}</p>{/if}
  {#if r.rows.length === 0}
    <p class="dim">Nothing to show.</p>
  {:else}
    <div class="scroll">
      <table>
        <thead
          ><tr
            >{#each r.headers as h, _i (_i)}<th>{h}</th>{/each}</tr
          ></thead
        >
        <tbody
          >{#each r.rows as cells, i (i)}<tr class:mod={r.highlight?.includes(i)}
              >{#each cells as c, _i (_i)}<td>{c}</td>{/each}</tr
            >{/each}</tbody
        >
      </table>
    </div>
  {/if}
  {#if r.footer?.length}<p class="dim footer">
      {#each r.footer as line, _i (_i)}<span>{line}</span>
      {/each}
    </p>{/if}
{:else if r.kind === 'text'}
  <pre class="mono">{r.lines.join('\n')}</pre>
{:else if r.kind === 'json'}
  <pre class="mono">{JSON.stringify(r.value, null, 2)}</pre>
{:else if r.kind === 'changed'}
  <p class="ok">{r.message}</p>
  <ul class="changed">
    {#each r.tasks as t, _i (_i)}<li>{t.id ?? t.uuid.slice(0, 8)} {t.description}</li>{/each}
  </ul>
{:else if r.kind === 'confirm'}
  {#key r}<ConfirmView {entry} result={r} />{/key}
{:else if r.kind === 'error'}
  <p class="err" role="alert">{r.message}</p>
{/if}
{#if entry.feedback?.length}
  <ul class="said" aria-label="Hooks">
    {#each entry.feedback as l, _i (_i)}<li class:warn={l.kind === 'warn'}>{l.text}</li>{/each}
  </ul>
{/if}

<style>
  .footer {
    display: flex;
    flex-wrap: wrap;
    gap: 0 0.7em;
    margin: 6px 0;
  }
  pre {
    margin: 4px 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .scroll {
    overflow-x: auto;
  }
  table {
    border-collapse: collapse;
  }
  /* `pre`: the indentation of sub-projects in `projects` is made of spaces. */
  th,
  td {
    text-align: left;
    padding: 2px 16px 2px 0;
    white-space: pre;
  }
  th {
    color: var(--dim);
    font-weight: 500;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
  }
  /* A setting changed from its default (`show`). */
  tr.mod td {
    color: var(--accent);
    font-weight: 600;
  }
  .said {
    list-style: none;
    margin: 4px 0;
    padding: 0;
    color: var(--dim);
    font-size: 13px;
  }
  .said .warn {
    color: var(--warn);
  }
  .changed {
    margin: 0;
    padding-left: 18px;
    color: var(--dim);
  }
  p {
    margin: 4px 0;
  }
</style>

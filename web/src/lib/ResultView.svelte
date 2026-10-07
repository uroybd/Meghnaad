<script lang="ts">
  import ReportTable from './ReportTable.svelte';
  import TaskInfo from './TaskInfo.svelte';
  import { store, type Entry } from './store.svelte';
  import type { Row } from './types';

  let {
    entry,
    onedit,
    onsort,
  }: { entry: Entry; onedit?: (row: Row, from: Entry) => void; onsort?: (column: string, shift: boolean) => void } = $props();
  const r = $derived(entry.result);
</script>

{#if entry.failure}
  <p class="err" role="alert">{entry.failure}</p>
{:else if !r}
  <p class="dim">Running…</p>
{:else if r.kind === 'report'}
  <ReportTable result={r} {entry} onedit={onedit && ((row) => onedit(row, entry))} {onsort} />
{:else if r.kind === 'info'}
  {#each r.tasks as t (t.uuid)}<TaskInfo task={t} onedit={onedit && ((row) => onedit(row, entry))} />{/each}
{:else if r.kind === 'table'}
  {#if r.title}<p class="dim">{r.title}</p>{/if}
  {#if r.rows.length === 0}
    <p class="dim">Nothing to show.</p>
  {:else}
    <table>
      <thead><tr>{#each r.headers as h}<th>{h}</th>{/each}</tr></thead>
      <tbody>{#each r.rows as cells}<tr>{#each cells as c}<td>{c}</td>{/each}</tr>{/each}</tbody>
    </table>
  {/if}
{:else if r.kind === 'text'}
  <pre class="mono">{r.lines.join('\n')}</pre>
{:else if r.kind === 'json'}
  <pre class="mono">{JSON.stringify(r.value, null, 2)}</pre>
{:else if r.kind === 'changed'}
  <p class="ok">{r.message}</p>
  <ul class="changed">
    {#each r.tasks as t}<li>{t.id ?? t.uuid.slice(0, 8)} {t.description}</li>{/each}
  </ul>
{:else if r.kind === 'confirm'}
  <div class="confirm" role="alert">
    <span>{r.message}</span>
    <button class="primary" onclick={() => store.confirm(entry, true)}>Yes</button>
    <button onclick={() => store.confirm(entry, false)}>No</button>
  </div>
{:else if r.kind === 'error'}
  <p class="err" role="alert">{r.message}</p>
{/if}

<style>
  pre { margin: 4px 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  table { border-collapse: collapse; }
  th, td { text-align: left; padding: 2px 16px 2px 0; }
  th { color: var(--dim); font-weight: 500; font-size: 12px; border-bottom: 1px solid var(--line); }
  .confirm { display: flex; gap: 8px; align-items: center; padding: 6px 0; }
  .changed { margin: 0; padding-left: 18px; color: var(--dim); }
  p { margin: 4px 0; }
</style>

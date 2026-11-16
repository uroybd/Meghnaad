<script lang="ts">
  import { untrack } from 'svelte';
  import { runCli } from './api';
  import { saveAs } from './download';
  import {
    exportFields,
    jsonArrayOf,
    renderExport,
    tableFields,
    type ExportChoice,
    type Fields,
    type Format,
  } from './exportFile';
  import { Download } from './icons';
  import { store } from './store.svelte';
  import type { Column } from './types';

  /** The Export button's dialog: what a table shows, as a file shaped as the person likes. */
  let {
    args,
    columns,
    onclose,
  }: {
    /** The command line that exports the table's tasks (`exportArgs`). */
    args: string[];
    /** The table's columns, for "only the table's fields". */
    columns: Column[];
    onclose: () => void;
  } = $props();

  const KEY = 'tw-web-export';
  // `json.array` as the taskrc has it, which the list toggle starts at.
  const jsonArray = $derived(jsonArrayOf(store.config?.config));

  /** The last choices, kept in this browser; the JSON shape always starts as the taskrc says. */
  function remembered(): Pick<ExportChoice, 'format' | 'fields' | 'picked'> {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<ExportChoice>;
      return {
        format: v.format === 'csv' || v.format === 'tsv' ? v.format : 'json',
        fields: v.fields === 'table' || v.fields === 'custom' ? v.fields : 'all',
        picked: Array.isArray(v.picked) ? v.picked.filter((x) => typeof x === 'string') : [],
      };
    } catch {
      return { format: 'json', fields: 'all', picked: [] };
    }
  }
  const start = remembered();
  let format = $state<Format>(start.format);
  /** Starts as the taskrc's `json.array`; the person may change it for this file. */
  let array = $derived(jsonArray);
  let fields = $state<Fields>(start.fields);
  let picked = $state<string[]>(start.picked);

  let exported = $state<{ name: string; text: string; count: number } | null>(null);
  let error = $state<string | null>(null);
  const names = $derived(columns.map((c) => c.name));
  const available = $derived(exported ? exportFields(exported.text) : []);
  const tasks = $derived(exported?.count ?? 0);
  const choice = $derived<ExportChoice>({ format, array, fields, picked });
  /** How many fields the file will have. */
  const count = $derived(
    fields === 'all'
      ? available.length
      : fields === 'table'
        ? tableFields(names).length
        : available.filter((f) => picked.includes(f)).length,
  );

  let dialog: HTMLDialogElement;
  $effect(() => {
    dialog.showModal();
  });
  async function load(line: string[]) {
    try {
      const r = (await runCli({ args: line })).result;
      if (r.kind === 'error') throw new Error(r.message);
      if (r.kind !== 'file') throw new Error('The tasks could not be read.');
      exported = { name: r.name, text: r.text, count: r.count };
      // Nothing picked yet: start from the table's fields.
      const have = exportFields(r.text);
      if (!picked.length) picked = tableFields(names).filter((f) => have.includes(f));
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
  $effect(() => {
    const line = args;
    untrack(() => void load(line));
  });
  function toggle(f: string, on: boolean) {
    picked = on ? [...picked, f] : picked.filter((x) => x !== f);
  }

  function download() {
    if (!exported) return;
    const file = renderExport(exported, choice, names, store.config?.config);
    try {
      localStorage.setItem(KEY, JSON.stringify({ format, fields, picked }));
    } catch {
      /* private mode: the choice lasts until the page is closed */
    }
    saveAs(file.name, file.text, file.mime);
    dialog.close();
  }
</script>

<dialog bind:this={dialog} {onclose} aria-label="Export">
  <h3>Export</h3>
  <p class="dim">
    {#if !exported && !error}Reading the tasks…{:else if exported}{tasks}
      {tasks === 1 ? 'task' : 'tasks'}, as the table shows them.{/if}
    <code class="line">{args.join(' ')}</code>
  </p>
  {#if error}<p class="err" role="alert">{error}</p>{/if}

  <fieldset>
    <legend>Format</legend>
    <label><input type="radio" bind:group={format} value="json" /> JSON</label>
    <label><input type="radio" bind:group={format} value="csv" /> CSV</label>
    <label><input type="radio" bind:group={format} value="tsv" /> TSV</label>
    {#if format === 'json'}
      <label class="sub">
        <input type="checkbox" bind:checked={array} />
        As a list (<code>json.array</code>{jsonArray ? ', on in your taskrc' : ', off in your taskrc'})
      </label>
    {/if}
  </fieldset>

  <fieldset>
    <legend>Fields</legend>
    <label><input type="radio" bind:group={fields} value="all" /> All fields</label>
    <label><input type="radio" bind:group={fields} value="table" /> Only the table's columns</label>
    <label><input type="radio" bind:group={fields} value="custom" /> Choose</label>
    {#if fields === 'custom'}
      <div class="picks" role="group" aria-label="Fields to include">
        {#each available as f (f)}
          <label class="mono">
            <input type="checkbox" checked={picked.includes(f)} onchange={(e) => toggle(f, e.currentTarget.checked)} />
            {f}
          </label>
        {/each}
      </div>
    {/if}
  </fieldset>

  <footer class="row">
    <span class="dim small">{exported ? `${count} ${count === 1 ? 'field' : 'fields'}` : ''}</span>
    <span class="grow"></span>
    <button type="button" onclick={() => dialog.close()}>Cancel</button>
    <button class="primary" onclick={download} disabled={!exported || count === 0}>
      <Download size={14} /> Download
    </button>
  </footer>
</dialog>

<style>
  dialog {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    padding: 22px 26px;
    width: min(520px, 96vw);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  h3 {
    margin: 0 0 4px;
  }
  .line {
    display: block;
    margin-top: 6px;
    overflow-wrap: anywhere;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    font-size: 12.5px;
  }
  fieldset {
    border: 0;
    margin: 12px 0 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  legend {
    font-size: 12px;
    color: var(--dim);
    margin-bottom: 4px;
    padding: 0;
  }
  label {
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .sub {
    flex-basis: 100%;
  }
  .picks {
    flex-basis: 100%;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 4px 12px;
    max-height: 220px;
    overflow: auto;
    font-size: 13px;
  }
  .small {
    font-size: 12px;
  }
  footer {
    margin-top: 18px;
    gap: 8px;
  }
  footer .primary {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
</style>

<script lang="ts">
  import { ApiError, MAX_IMPORT_BYTES, importTasks } from './api';
  import { Upload } from './icons';
  import { store } from './store.svelte';
  import type { ImportReport } from './types';

  /**
   * `import`: pick a file of tasks (what `export` writes). The file is read here and sent as it is; the server
   * reads it, checks all of it, and says what importing would do. Nothing is written until you confirm, and
   * then all of it is or none.
   */
  let text = $state('');
  let name = $state('');
  let report = $state<ImportReport | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const changes = $derived(report ? report.added + report.modified : 0);
  const words = (n: number) => `${n} ${n === 1 ? 'task' : 'tasks'}`;
  const fail = (e: unknown) => (error = e instanceof ApiError || e instanceof Error ? e.message : String(e));

  async function choose(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    report = error = null;
    if (!file) return;
    if (file.size > MAX_IMPORT_BYTES) {
      error = `That file is ${(file.size / 1024 / 1024).toFixed(1)} MB; the most one import takes is ${MAX_IMPORT_BYTES / 1024 / 1024} MB. Split it and import the parts.`;
      return;
    }
    busy = true;
    try {
      name = file.name;
      text = await file.text();
      report = await importTasks(text, false);
    } catch (err) {
      fail(err);
    } finally {
      busy = false;
    }
  }

  async function apply() {
    busy = true;
    error = null;
    try {
      report = await importTasks(text, true);
      text = '';
      store.notify(`Imported ${words(report.added + report.modified)}.`);
      if (store.live) void store.refresh(store.live);
    } catch (err) {
      fail(err);
    } finally {
      busy = false;
    }
  }
</script>

<div class="import">
  <p>
    <label class="pick">
      <Upload size={15} /> Choose a file…
      <input type="file" accept=".json,.txt,application/json,text/plain" onchange={choose} disabled={busy} />
    </label>
    <span class="dim">Tasks as <code>export</code> writes them. {name && !report ? name : ''}</span>
  </p>
  {#if busy}<p class="dim" role="status">Working…</p>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if report}
    <p class={report.applied ? 'ok' : ''}>
      {report.applied ? 'Imported' : `${name}:`}
      {report.added} new, {report.modified} changed, {report.skipped} unchanged.
      {#if !report.applied && changes > 0}
        <button class="primary" onclick={apply} disabled={busy}>Import {words(changes)}</button>
      {:else if !report.applied}
        Nothing to import.
      {/if}
    </p>
    {#each report.warnings as w, _i (_i)}<p class="warn">{w}</p>{/each}
    {#each report.feedback as l, _i (_i)}<p class:warn={l.kind === 'warn'}>{l.text}</p>{/each}
    {#if report.lines.length}
      <details open={report.lines.length <= 12}>
        <summary>{words(report.lines.length + report.more)}</summary>
        <table>
          <tbody>
            {#each report.lines as l (l.uuid)}
              <tr class={l.action}>
                <td>{l.action === 'add' ? 'new' : l.action === 'mod' ? 'changed' : 'same'}</td>
                <td class="mono">{l.uuid.slice(0, 8)}</td>
                <td>{l.description}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if report.more}<p class="dim">…and {report.more} more.</p>{/if}
      </details>
    {/if}
    {#if !report.applied && changes > 0}
      <p class="dim">Nothing is written until you import, and then all of it is, or none.</p>
    {/if}
  {/if}
</div>

<style>
  .import p {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 4px 0;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border: 1px solid var(--border, currentColor);
    border-radius: 6px;
    cursor: pointer;
  }
  .pick:focus-within {
    outline: 2px solid var(--accent, currentColor);
  }
  .pick input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
  }
  summary {
    color: var(--dim);
    font-size: 13px;
    cursor: pointer;
  }
  table {
    border-collapse: collapse;
    font-size: 13px;
  }
  td {
    padding: 1px 12px 1px 0;
  }
  .skip {
    color: var(--dim);
  }
  .warn {
    color: var(--warn, inherit);
    font-size: 13px;
  }
</style>

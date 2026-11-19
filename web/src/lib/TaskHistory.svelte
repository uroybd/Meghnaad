<script lang="ts">
  import { runCli } from './api';
  import { describeChange } from './history';
  import type { HistoryEntry, Row } from './types';

  let {
    task,
    stamp,
  }: {
    task: Row;
    /** A moment as the task's info writes it (`dateformat.info`). */
    stamp: (epoch: number) => string;
  } = $props();

  // The history is not part of the task: it is fetched when the section is opened, and again when the task changes.
  let open = $state(false);
  let history = $state<HistoryEntry[] | null>(null);
  let error = $state<string | null>(null);
  let asked = 0;
  $effect(() => {
    void task.uuid;
    void task.modified;
    if (!open) return;
    const mine = ++asked;
    error = null;
    runCli({ args: ['_history', task.uuid] })
      .then(({ result }) => {
        if (mine !== asked) return;
        if (result.kind === 'json') history = result.value as HistoryEntry[];
        else error = result.kind === 'error' ? result.message : 'The history could not be read.';
      })
      .catch((e) => {
        if (mine === asked) error = e instanceof Error ? e.message : String(e);
      });
  });
  // Another task: forget the last one's history, so it is never shown against the wrong task.
  $effect(() => {
    void task.uuid;
    history = null;
  });
</script>

<!-- Closed until asked for, and fetched then (`_history`): the log grows with every edit, and is rarely what you opened the task for. -->
<details class="hist" bind:open>
  <summary title="What changed and when, from the task's operation log (journal.info).">
    History{#if history}
      <span class="dim">· {history.length} {history.length === 1 ? 'moment' : 'moments'}</span>{/if}
  </summary>
  {#if error}
    <p class="err" role="alert">{error}</p>
  {:else if history === null}
    <p class="dim">Loading…</p>
  {:else if !history.length}
    <p class="dim">
      Nothing is recorded: <code>journal.info</code> is off, or the changes came before this app last started.
    </p>
  {:else}
    <div class="scroll">
      <table class="history" data-testid="history">
        <thead><tr><th>Date</th><th>Modification</th></tr></thead>
        <tbody>
          {#each history as e, _i (_i)}
            <tr>
              <td class="mono">{stamp(e.at)}</td>
              <td
                >{#each e.changes as c, _i (_i)}<div class="pre">{describeChange(c, stamp)}</div>{/each}</td
              >
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</details>

<style>
  .scroll {
    overflow-x: auto;
  }
  .pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .hist .err {
    color: var(--err);
  }
  .hist {
    margin: 16px 0 6px;
  }
  .hist summary {
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
  }
  .hist summary .dim {
    font-weight: 400;
    font-size: 13px;
  }
  .hist[open] summary {
    margin-bottom: 6px;
  }
  .history {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    margin: 2px 0 6px;
  }
  .history th,
  .history td {
    text-align: left;
    padding: 3px 12px 3px 0;
    vertical-align: top;
  }
  .history td:first-child {
    white-space: nowrap;
    color: var(--dim);
  }
  .history th {
    color: var(--dim);
    font-weight: 500;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
  }
  .history tbody td {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
</style>

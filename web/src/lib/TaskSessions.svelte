<script lang="ts">
  import { formatSeconds } from './dates';
  import { Timer } from './icons';
  import type { Session } from './types';

  let {
    sessions,
    total,
    stamp,
  }: {
    /** The times the task was started and stopped (`end` null while it runs), from the journal's notes. */
    sessions: Session[];
    /** The time tracked in all, in seconds. */
    total: number;
    /** A moment as the task's info writes it (`dateformat.info`). */
    stamp: (epoch: number) => string;
  } = $props();
</script>

<h4 class="sess"><Timer size={13} /> Time sessions</h4>
<div class="scroll">
  <table class="sessions" data-testid="sessions">
    <thead>
      <tr><th>#</th><th>Started</th><th>Stopped</th><th class="num">Duration</th></tr>
    </thead>
    <tbody>
      {#each sessions as s, i (i)}
        <tr class:running={s.end == null}>
          <td class="dim">{i + 1}</td>
          <td class="mono">{stamp(s.start)}</td>
          <td class="mono">
            {#if s.end != null}{stamp(s.end)}{:else}<span class="chip live"><Timer size={11} /> running</span>{/if}
          </td>
          <td class="num mono">{formatSeconds(s.seconds)}</td>
        </tr>
        {#if s.description}
          <tr class="said"><td></td><td colspan="3">{s.description}</td></tr>
        {/if}
      {/each}
    </tbody>
    <tfoot>
      <tr><td colspan="3">Total</td><td class="num mono" data-testid="sessions-total">{formatSeconds(total)}</td></tr>
    </tfoot>
  </table>
</div>

<style>
  h4 {
    margin: 16px 0 6px;
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  /* The notes written during a session, under its row, a line each. */
  .sessions .said td {
    white-space: pre-line;
    color: var(--dim);
    padding-top: 0;
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--ok);
    border-color: var(--ok);
  }
  .scroll {
    overflow-x: auto;
  }
  .sessions {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    margin: 2px 0 6px;
  }
  .sessions th,
  .sessions td {
    text-align: left;
    padding: 2px 10px 2px 0;
    white-space: nowrap;
  }
  .sessions th {
    color: var(--dim);
    font-weight: 500;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
  }
  .sessions .num {
    text-align: right;
    padding-right: 0;
    font-variant-numeric: tabular-nums;
  }
  .sessions tfoot td {
    border-top: 1px solid var(--line);
    font-weight: 600;
    padding-top: 3px;
  }
  .sessions tr.running td {
    color: var(--ok);
  }
</style>

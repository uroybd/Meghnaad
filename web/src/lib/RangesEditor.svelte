<script lang="ts">
  import type { DateRange } from './boards';
  import DateField from './DateField.svelte';
  import { Plus, Trash2 } from './icons';

  /**
   * The date ranges a board offers, as it is edited: a name and a start and an end, each a date as Taskwarrior reads one
   * (`sow`, `eod`, `2026-10-01`, `eow-2d`). A filter on the board uses them as `$name.start` and `$name.end`. What the
   * engine thinks of them (a name used twice, a date it cannot read) is said with the board's other problems.
   */
  let { ranges = $bindable() }: { ranges: DateRange[] } = $props();

  function add() {
    const taken = new Set(ranges.map((r) => r.name));
    let n = 1;
    while (taken.has(n === 1 ? 'daterange' : `daterange${n}`)) n++;
    ranges = [...ranges, { name: n === 1 ? 'daterange' : `daterange${n}`, start: 'sow', end: 'eow' }];
  }
  const set = (i: number, patch: Partial<DateRange>) =>
    (ranges = ranges.map((r, j) => (j === i ? { ...r, ...patch } : r)));
</script>

<fieldset class="ranges">
  <legend>Date ranges</legend>
  {#each ranges as r, i (i)}
    <div class="range">
      <label>
        Name
        <input
          class="mono"
          value={r.name}
          oninput={(e) => set(i, { name: e.currentTarget.value.trim().toLowerCase() })}
          spellcheck="false"
          autocomplete="off"
          aria-label="Name of date range {i + 1}"
        />
      </label>
      <div class="field">
        <span>Start</span>
        <DateField
          bind:value={() => r.start, (v) => set(i, { start: v })}
          label="Start of {r.name || 'date range ' + (i + 1)}"
          placeholder="sow"
        />
      </div>
      <div class="field">
        <span>End</span>
        <DateField
          bind:value={() => r.end, (v) => set(i, { end: v })}
          label="End of {r.name || 'date range ' + (i + 1)}"
          placeholder="eow"
        />
      </div>
      <button
        type="button"
        class="ghost"
        aria-label="Remove {r.name || 'date range ' + (i + 1)}"
        title="Remove"
        onclick={() => (ranges = ranges.filter((_, j) => j !== i))}><Trash2 size={15} /></button
      >
    </div>
  {/each}
  <div class="foot">
    <button type="button" onclick={add}><Plus size={14} /> Date range</button>
    <span class="dim small">
      Use <code>$name.start</code> and <code>$name.end</code> in any filter on the board, such as
      <code>due.after:$daterange.start</code>.
    </span>
  </div>
</fieldset>

<style>
  .ranges {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px 10px;
    margin: 0;
    display: grid;
    gap: 8px;
  }
  legend {
    font-size: 12px;
    color: var(--dim);
    padding: 0 4px;
  }
  .range {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto auto;
    gap: 8px;
    align-items: end;
  }
  .range label,
  .range .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--dim);
  }
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    padding: 1px 5px;
    border-radius: 4px;
  }
  @media (max-width: 560px) {
    .range {
      grid-template-columns: 1fr 1fr auto;
    }
    .range label:first-child {
      grid-column: 1 / -1;
    }
  }
</style>

<script lang="ts">
  import { rangeLabel, resolveRanges, viewed, zoneNow, type Board, type ResolvedRange } from './boards';
  import DateField from './DateField.svelte';
  import { formatFor } from './dateformat';
  import { formatMoment } from './dates';
  import { ChevronDown, ChevronRight, RotateCcw } from './icons';
  import FilterInput from './FilterInput.svelte';
  import { store } from './store.svelte';

  /**
   * A filter to look at one board another way: collapsed by default, and what is typed here takes the place of the board's own
   * filter (its default, saved with the board) for as long as the page is open. It is not saved. A context goes in it as it does
   * in the board's own: `rc.context=this_week`. The board's date ranges are beside it, always in view, and work the same way.
   */
  let { board }: { board: Board } = $props();

  let open = $state(false);
  const own = $derived(board.filter ?? '');
  const typed = $derived(store.boardFilters[board.id]);
  const changed = $derived(typed !== undefined);

  function set(value: string) {
    const next = { ...store.boardFilters };
    // Typing the board's own filter back is no change.
    if (value.trim() === own.trim()) delete next[board.id];
    else next[board.id] = value;
    store.boardFilters = next;
  }

  // ---- The date ranges ---------------------------------------------------------------------------------------------
  const typedRanges = $derived(store.boardRanges[board.id] ?? {});
  const ranges = $derived(viewed(board, undefined, typedRanges).ranges ?? []);
  const rangesChanged = $derived(Object.keys(typedRanges).length > 0);
  const fmt = $derived(formatFor('info', store.config?.config.settings));
  const resolved = $derived<ResolvedRange[]>(resolveRanges(ranges, zoneNow(store.config?.config.settings)));

  /** Keep these dates for the range (or, with none, the board's own). */
  function keepRange(name: string, dates: { start: string; end: string } | null) {
    const next = { ...store.boardRanges };
    const mine = { ...typedRanges };
    if (dates) mine[name] = dates;
    else delete mine[name];
    if (Object.keys(mine).length) next[board.id] = mine;
    else delete next[board.id];
    store.boardRanges = next;
  }
  function setRange(name: string, which: 'start' | 'end', value: string) {
    const saved = board.ranges?.find((r) => r.name === name);
    if (!saved) return;
    const was = typedRanges[name] ?? saved;
    const now = { start: was.start, end: was.end };
    now[which] = value;
    // Typing the board's own dates back is no change.
    const same = now.start.trim() === saved.start.trim() && now.end.trim() === saved.end.trim();
    keepRange(name, same ? null : now);
  }
  /** `Oct 5 – Oct 11` as the user's dateformat writes each end; or why the range is not one. */
  function shown(r: ResolvedRange | undefined): string {
    if (!r) return '';
    if (r.error) return r.error;
    return `${formatMoment(r.start ?? 0, undefined, fmt)} – ${formatMoment(r.end ?? 0, undefined, fmt)}`;
  }
</script>

<section class="filter" aria-label="Filter for this board">
  <div class="row">
    <button
      type="button"
      class="ghost toggle"
      aria-expanded={open}
      aria-controls="board-view-filter"
      onclick={() => (open = !open)}
    >
      {#if open}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if} Filter
      {#if changed}<span class="changed">changed</span>{/if}
    </button>
    {#each ranges as r, i (r.name)}
      <div class="range" role="group" aria-label="Date range {rangeLabel(r.name)}">
        <span class="name" title="Use it in a filter as ${r.name}.start and ${r.name}.end">{rangeLabel(r.name)}</span>
        <DateField
          bind:value={() => r.start, (v) => setRange(r.name, 'start', v)}
          label="Start of {rangeLabel(r.name)}"
          invalid={!!resolved[i]?.error}
        />
        <span class="dim">–</span>
        <DateField
          bind:value={() => r.end, (v) => setRange(r.name, 'end', v)}
          label="End of {rangeLabel(r.name)}"
          invalid={!!resolved[i]?.error}
        />
        <button
          type="button"
          class="ghost"
          aria-label="Use the board's own dates for {rangeLabel(r.name)}"
          title="Use the board's own dates"
          disabled={!typedRanges[r.name]}
          onclick={() => keepRange(r.name, null)}><RotateCcw size={15} /></button
        >
      </div>
    {/each}
  </div>
  {#if ranges.length}
    <ul class="dim resolved" aria-label="What the date ranges are now">
      {#each ranges as r, i (r.name)}
        <li class:err={!!resolved[i]?.error}><code>${r.name}</code> {shown(resolved[i])}</li>
      {/each}
      {#if rangesChanged}<li>Dates typed here replace the board's own while you look at it; nothing is saved.</li>{/if}
    </ul>
  {/if}
  {#if open}
    <div id="board-view-filter" class="body">
      <div class="row">
        <div class="grow">
          <FilterInput
            bind:value={() => typed ?? own, set}
            id="board-view-filter-input"
            placeholder="Which tasks: project:Home +work  ·  rc.context=this_week"
          />
        </div>
        <button
          type="button"
          class="ghost"
          aria-label="Use the board's own filter"
          title="Use the board's own filter"
          disabled={!changed}
          onclick={() => set(own)}><RotateCcw size={15} /></button
        >
      </div>
      <p class="dim hint">
        {#if own}The board's own filter is <code>{own}</code>.{:else}The board has no filter of its own.{/if}
        What you type here replaces it while you look at this board; nothing is saved. A card that ignores the board's filter
        keeps its own.
      </p>
    </div>
  {/if}
</section>

<style>
  .filter {
    margin: 4px 0 10px;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
  }
  .row {
    flex-wrap: wrap;
  }
  .range {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .name {
    font-size: 13px;
    font-weight: 600;
  }
  .resolved {
    list-style: none;
    margin: 2px 0 0;
    padding: 0;
    font-size: 12.5px;
  }
  .err {
    color: var(--err);
  }
  .changed {
    margin-left: 6px;
    padding: 0 7px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-text);
    font-size: 11px;
  }
  .body {
    margin-top: 6px;
    display: grid;
    gap: 6px;
  }
  .hint {
    margin: 0;
    font-size: 12.5px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    padding: 1px 5px;
    border-radius: 4px;
    overflow-wrap: anywhere;
  }
</style>

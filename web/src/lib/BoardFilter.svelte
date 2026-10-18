<script lang="ts">
  import type { Board } from './boards';
  import { ChevronDown, ChevronRight, RotateCcw } from './icons';
  import FilterInput from './FilterInput.svelte';
  import { store } from './store.svelte';

  /**
   * A filter to look at one board another way: collapsed by default, and what is typed here takes the place of the board's own
   * filter (its default, saved with the board) for as long as the page is open. It is not saved. A context goes in it as it does
   * in the board's own: `rc.context=this_week`.
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
  </div>
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

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { splitWords } from './cmdline';
  import FilterChips from './FilterChips.svelte';
  import FilterInput from './FilterInput.svelte';
  import ResultView from './ResultView.svelte';
  import { store, type Entry } from './store.svelte';

  /**
   * A page in the style of the Tasks page: a filter box (focused when the page opens) and, under it,
   * what the same command prints in the console, drawn by the same component.
   */
  let {
    which,
    filter = $bindable(''),
    command,
    label,
    placeholder,
    controls,
  }: {
    which: 'summary' | 'calendar' | 'burndown';
    filter: string;
    /** The command the filter is put in front of, such as `summary` or `burndown.weekly`. */
    command: string;
    label: string;
    placeholder?: string;
    /** Extra buttons or selects beside the filter box. */
    controls?: Snippet<[Entry | null]>;
  } = $props();

  const entry = $derived(store.panels[which]);

  // Ask again when the filter or the command changes, and when a task changes anywhere. Only typing
  // waits (a moment); opening the page and clicking a button do not.
  let lastFilter: string | null = null;
  $effect(() => {
    const f = filter;
    const cmd = command;
    void store.rev;
    const typing = lastFilter !== null && f !== lastFilter;
    const t = setTimeout(
      () => {
        lastFilter = f;
        void store.runPanel(which, { args: [...splitWords(f), cmd] });
      },
      typing ? 250 : 0,
    );
    return () => clearTimeout(t);
  });
</script>

<section class="bar" aria-label={label}>
  <div class="row top">
    {#if controls}{@render controls(entry)}{/if}
    <FilterInput bind:value={filter} id="{which}-filter" {placeholder} autofocus />
  </div>
  <FilterChips bind:value={filter} />
</section>
{#if entry}
  <div class:stale={entry.loading && entry.result}>
    <ResultView {entry} />
  </div>
{/if}

<style>
  .bar {
    display: grid;
    gap: 10px;
    margin-bottom: 18px;
  }
  .top {
    flex-wrap: wrap;
  }
  .stale {
    opacity: 0.6;
    transition: opacity 0.15s;
  }
</style>

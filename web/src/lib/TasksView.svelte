<script lang="ts">
  import FilterBar from './FilterBar.svelte';
  import ResultView from './ResultView.svelte';
  import { clickSort, parseSort, serializeSort, withSortOverride } from './sortSpec';
  import { store } from './store.svelte';

  // A header click rewrites `rc.report.<name>.sort:…` in the filter, exactly what you'd type in the
  // console, so the table, the filter chips and the "last command" line stay in step.
  function sortBy(column: string, shift: boolean) {
    const r = store.live?.result;
    if (r?.kind !== 'report') return;
    const base = column.split('.')[0];
    const res = clickSort(parseSort(r.sort), base, shift);
    const def = store.reports.find((x) => x.name === store.report)?.sort ?? null;
    store.filter = withSortOverride(store.filter, store.report, res.reset ? null : serializeSort(res.keys), def);
  }
</script>

<FilterBar />
{#if store.live}
  <div class:stale={store.live.loading && store.live.result}>
    <ResultView entry={store.live} onedit={(row, from) => (store.editing = { row, from })} onsort={sortBy} />
  </div>
{/if}

<style>
  .stale { opacity: 0.6; transition: opacity 0.15s; }
</style>

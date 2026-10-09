<script lang="ts">
  import FilterBar from './FilterBar.svelte';
  import ResultView from './ResultView.svelte';
  import { clearGroups, clickSort, parseSort, serializeSort, toggleGroup, withSortOverride } from './sortSpec';
  import { store } from './store.svelte';
  import { activeTags, toggleTag } from './tagfilter';

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

  // Grouping is the `/` on a sort key (`project+/`). Choosing a column turns it on or off; no column ends it.
  function groupBy(column: string | null) {
    const r = store.live?.result;
    if (r?.kind !== 'report') return;
    const def = store.reports.find((x) => x.name === store.report)?.sort ?? null;
    const keys = column ? toggleGroup(parseSort(r.sort), column, parseSort(def)) : clearGroups(parseSort(r.sort));
    store.filter = withSortOverride(store.filter, store.report, serializeSort(keys), def);
  }

  // A tag chip in this table toggles that tag in the report's filter, as typing `+tag` would.
  const toggle = (tag: string) => (store.filter = toggleTag(store.filter, tag));
  const active = $derived(activeTags(store.filter));
</script>

<FilterBar />
{#if store.live}
  <div class:stale={store.live.loading && store.live.result}>
    <ResultView
      entry={store.live}
      onedit={(row, from) => (store.editing = { row, from })}
      onsort={sortBy}
      ongroup={groupBy}
      ontag={toggle}
      activeTags={active}
    />
  </div>
{/if}

<style>
  .stale {
    opacity: 0.6;
    transition: opacity 0.15s;
  }
</style>

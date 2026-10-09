<script lang="ts">
  import { reportArgs, shellQuote, splitWords } from './cmdline';
  import FilterChips from './FilterChips.svelte';
  import FilterInput from './FilterInput.svelte';
  import { store } from './store.svelte';

  const meta = $derived(store.reports.find((r) => r.name === store.report));
  const args = $derived(reportArgs(store.filter, store.report));
  const words = $derived(splitWords(store.filter));
  const cfg = $derived(store.config?.config);
  const context = $derived(
    cfg?.active_context ? { name: cfg.active_context, read: cfg.contexts[cfg.active_context]?.read ?? null } : null,
  );

  // Run the report whenever the report or filter changes. Only typing is debounced: opening the
  // page or picking another report should not wait for a keystroke that isn't coming. The console
  // may already have loaded exactly this (it hands its result over), so skip duplicates.
  let ranFilter: string | null = null;
  $effect(() => {
    const a = args;
    const filter = store.filter;
    if (JSON.stringify(a) === store.liveKey) {
      ranFilter = filter;
      return;
    }
    const typing = ranFilter !== null && filter !== ranFilter;
    const t = setTimeout(() => {
      ranFilter = filter;
      void store.runLive({ args: a });
    }, typing ? 250 : 0);
    return () => clearTimeout(t);
  });

  function pickReport(name: string) {
    // Sort overrides belong to one report; drop the ones that no longer apply.
    store.filter = words
      .filter((w) => !/^rc\.report\./.test(w) || w.startsWith(`rc.report.${name}.`))
      .map(shellQuote)
      .join(' ');
    store.report = name;
  }
</script>

<section class="bar" aria-label="Report and filters">
  <div class="row top">
    <label class="sr-only" for="fb-report">Report</label>
    <select id="fb-report" value={store.report} onchange={(e) => pickReport(e.currentTarget.value)} title={meta?.description ?? ''}>
      {#each store.reports as r (r.name)}
        <option value={r.name}>{r.name}{r.description ? ` — ${r.description}` : ''}</option>
      {/each}
      {#if store.reports.length === 0}<option value={store.report}>{store.report}</option>{/if}
    </select>
    <FilterInput bind:value={store.filter} id="fb-filter" />
  </div>

  <FilterChips bind:value={store.filter} />

  {#if meta?.filter || meta?.sort || context}
    <p class="implicit dim">
      {#if meta?.filter}report filter <code>{meta.filter}</code>{/if}
      {#if meta?.sort} · default sort <code>{meta.sort}</code>{/if}
      {#if context} · context <code>{context.name}{context.read ? ` (${context.read})` : ''}</code>{/if}
      <span class="tail">— applied automatically; your filters are added to these. Click a column header to sort.</span>
    </p>
  {/if}
</section>

<style>
  .bar { display: grid; gap: 10px; margin-bottom: 18px; }
  .top select { max-width: 17em; }
  @media (max-width: 760px) {
    .top { flex-wrap: wrap; }
    .top select { max-width: 100%; width: 100%; }
    .implicit .tail { display: none; }
  }
  .implicit { margin: 0; font-size: 13px; }
  .implicit code { background: var(--panel-2); border-radius: 4px; padding: 1px 5px; }
</style>

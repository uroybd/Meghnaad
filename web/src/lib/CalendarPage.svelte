<script lang="ts">
  import { withMonths, stepWords } from './calendarQuery';
  import { ChevronLeft, ChevronRight } from './icons';
  import PanelPage from './PanelPage.svelte';
  import { store, type Entry } from './store.svelte';

  /** The months now on screen, to page from. */
  function shown(entry: Entry | null): { first: { year: number; month: number }; count: number } | null {
    const r = entry?.result;
    if (r?.kind !== 'calendar' || r.months.length === 0) return null;
    return { first: { year: r.months[0].year, month: r.months[0].month }, count: r.months.length };
  }

  const go = (months: string[]) => (store.calendarFilter = withMonths(store.calendarFilter, months));
</script>

<PanelPage
  which="calendar"
  bind:filter={store.calendarFilter}
  command="calendar"
  label="Calendar months and filters"
  placeholder="Months and filters: march 2027 · y · due · project:Work +next  (Tab completes)"
>
  {#snippet controls(entry)}
    {@const at = shown(entry)}
    <div class="pager" role="group" aria-label="Months">
      <button class="ghost" aria-label="Earlier months" title="Earlier months" disabled={!at} onclick={() => at && go(stepWords(at.first, at.count, -1))}><ChevronLeft size={16} /></button>
      <button onclick={() => go([])} title="From this month">Today</button>
      <button class="ghost" aria-label="Later months" title="Later months" disabled={!at} onclick={() => at && go(stepWords(at.first, at.count, 1))}><ChevronRight size={16} /></button>
      <button onclick={() => go(['y'])} title="A year of months, from this one">Year</button>
      <button onclick={() => go(['due'])} title="From the oldest due date">First due</button>
    </div>
  {/snippet}
</PanelPage>

<style>
  .pager { display: inline-flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .pager button { white-space: nowrap; }
</style>

<script lang="ts">
  import PanelPage from './PanelPage.svelte';
  import { store } from './store.svelte';

  const periods = ['daily', 'weekly', 'monthly', 'annual'] as const;
</script>

<PanelPage
  which="burndown"
  bind:filter={store.burndownFilter}
  command="burndown.{store.burndownPeriod}"
  label="Burndown period and filters"
  placeholder="Add filters: project:Home +work  (Tab completes)"
>
  {#snippet controls()}
    <label class="sr-only" for="bd-period">Period</label>
    <select id="bd-period" bind:value={store.burndownPeriod}>
      {#each periods as p}<option value={p}>{p[0].toUpperCase() + p.slice(1)}</option>{/each}
    </select>
  {/snippet}
</PanelPage>

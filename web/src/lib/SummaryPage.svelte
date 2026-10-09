<script lang="ts">
  import PanelPage from './PanelPage.svelte';
  import { store } from './store.svelte';
</script>

<PanelPage
  which="summary"
  bind:filter={store.summaryFilter}
  command="summary"
  label="Summary filters"
  placeholder="Add filters: project:Home +work status:pending  (Tab completes)"
/>

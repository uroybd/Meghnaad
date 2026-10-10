<script lang="ts">
  import { shortUuid } from './format';
  import { ChevronDown, ChevronRight } from './icons';
  import { fetchTask } from './lookup';
  import { taskPath } from './route';
  import { store } from './store.svelte';
  import TaskCard from './TaskCard.svelte';
  import type { Row } from './types';

  /**
   * Another task in the detail pane, as a line you can tap: its name as far as the page knows it, and on a tap a
   * card of it, fetched only then if the page does not have it already, with a button that opens it. `row` is the
   * task when the caller already holds it (the tasks waiting on this one come with their data).
   */
  let { uuid, row = null, onopen }: { uuid: string; row?: Row | null; onopen?: (uuid: string) => void } = $props();

  let shown = $state(false);
  let fetched = $state<Row | null>(null);
  let missing = $state(false);

  const card = $derived(row ?? fetched);
  const name = $derived(card?.description ?? store.tasks.find((t) => t.uuid === uuid)?.description);

  async function toggle() {
    shown = !shown;
    if (!shown || card) return;
    const got = await fetchTask(uuid);
    fetched = got;
    missing = !got;
  }

  const open = () => (onopen ? onopen(uuid) : store.openDetail(uuid, null));
</script>

<span class="item">
  <button type="button" class="ghost line" aria-expanded={shown} title="Show this task" onclick={toggle}>
    {#if shown}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
    <span class="name">{name ?? 'Task'}</span>
    <span class="dim mono">{shortUuid(uuid)}</span>
  </button>
  {#if shown}
    <span class="card" data-testid="depitem-card">
      {#if card}
        <TaskCard row={card} />
      {:else if missing}
        <span class="dim">This task is not here any more.</span>
      {:else}
        <span class="dim">Loading…</span>
      {/if}
      <span class="foot">
        <button type="button" class="open" onclick={open}>Open task</button>
        <a class="dim mono" href={taskPath(uuid)} onclick={(e) => (e.preventDefault(), open())}>{taskPath(uuid)}</a>
      </span>
    </span>
  {/if}
</span>

<style>
  .item {
    display: block;
  }
  .line {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    padding: 0 4px;
    margin-left: -4px;
    text-align: left;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 2px 0 8px 14px;
    padding: 8px 10px;
    font-weight: 400;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 4px;
    font-size: 12px;
  }
  .foot a {
    color: inherit;
  }
</style>

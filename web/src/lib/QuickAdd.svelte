<script lang="ts">
  import { Plus, SlidersHorizontal } from './icons';
  import { store } from './store.svelte';

  // One line for the common case; "more fields" opens the full form with this text carried over.
  let description = $state('');
  let busy = $state(false);

  async function add(e: Event) {
    e.preventDefault();
    if (!description.trim()) return;
    busy = true;
    // `description:` keeps words like "project:foo" in the text from being read as fields.
    const res = await store.act(store.view === 'tasks' ? store.live : null, [
      'add',
      `description:${description.trim()}`,
    ]);
    busy = false;
    if (res && res.result.kind !== 'error') description = '';
  }

  function more() {
    store.adding = { description: description.trim(), n: (store.adding?.n ?? 0) + 1 };
    description = '';
  }
</script>

<form onsubmit={add} aria-label="Add a task">
  <div class="row">
    <input class="grow" bind:value={description} placeholder="Add a task…" aria-label="Description" />
    <button class="primary add" disabled={busy || !description.trim()} aria-label="Add task" title="Add task"
      ><Plus size={16} /></button
    >
  </div>
  <button type="button" class="ghost more" onclick={more}><SlidersHorizontal size={13} /> more fields…</button>
</form>

<style>
  form {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
  .row input {
    min-width: 0;
    width: 100%;
  }
  .more {
    justify-self: start;
    color: var(--dim);
    padding: 2px 0;
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .add {
    line-height: 0;
    padding: 5px 8px;
  }
</style>

<script lang="ts">
  import { formatFor } from './dateformat';
  import { formatMoment } from './dates';
  import { Check } from './icons';
  import { store } from './store.svelte';
  import TagChip from './TagChip.svelte';
  import type { Row } from './types';

  /** One pending task as a line in a tree or list: tick it done, open it, see its tags and due date. */
  let { row, depth }: { row: Row; depth: number } = $props();

  const fmt = $derived(formatFor('report', store.config?.config.settings));
  const pending = $derived(row.status === 'pending');
  const late = $derived(pending && row.due != null && row.due < store.now);
</script>

<li
  class="task"
  class:finished={!pending}
  class:completed={row.status === 'completed'}
  class:deleted={row.status === 'deleted'}
  style="--depth: {depth}"
>
  {#if pending}
    <button
      class="ghost tick"
      aria-label="Mark done: {row.description}"
      title="Mark done"
      onclick={() => void store.act(null, [row.uuid, 'done'])}><Check size={15} /></button
    >
  {:else}
    <span class="tick-gap" title={row.status}></span>
  {/if}
  <button class="ghost desc" onclick={() => store.openDetail(row.uuid)}>
    {#if row.priority}<span class="pri pri-{row.priority}">{row.priority}</span>{/if}
    <span class="text">{row.description}</span>
  </button>
  {#each row.tags as t (t)}<TagChip tag={t} />{/each}
  {#if row.due != null}<span class="due" class:late>{formatMoment(row.due, undefined, fmt)}</span>{/if}
</li>

<style>
  .task {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0 3px calc(var(--depth) * 20px + 4px);
    min-width: 0;
  }
  /* A finished or deleted task (the filter let it in): same alignment, nothing to tick, dimmed. */
  .tick-gap {
    width: 28px;
    flex: none;
  }
  /* Finished tasks are shown by colour, not struck through: green for completed, red for deleted. */
  .finished .text {
    color: var(--dim);
  }
  .task.completed {
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }
  .task.deleted {
    background: color-mix(in srgb, var(--err) 9%, transparent);
  }
  .tick {
    line-height: 0;
    color: var(--dim);
  }
  .tick:hover {
    color: var(--ok);
  }
  .desc {
    display: flex;
    align-items: baseline;
    gap: 6px;
    text-align: left;
    min-width: 0;
    flex: 0 1 auto;
  }
  .text {
    overflow-wrap: anywhere;
  }
  .pri {
    font-size: 11px;
    font-weight: 700;
  }
  .pri-H {
    color: var(--pri-h);
  }
  .pri-M {
    color: var(--pri-m);
  }
  .pri-L {
    color: var(--pri-l);
  }
  .due {
    color: var(--dim);
    font-size: 13px;
    white-space: nowrap;
  }
  .due.late {
    color: var(--err);
  }
  @media (max-width: 760px) {
    .task {
      flex-wrap: wrap;
    }
  }
</style>

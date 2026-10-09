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
  const late = $derived(row.due != null && row.due < store.now);
</script>

<li class="task" style="--depth: {depth}">
  <button
    class="ghost tick"
    aria-label="Mark done: {row.description}"
    title="Mark done"
    onclick={() => void store.act(null, [row.uuid, 'done'])}><Check size={15} /></button
  >
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

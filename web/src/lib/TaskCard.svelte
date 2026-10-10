<script lang="ts">
  import { formatMoment } from './dates';
  import StatusPill, { type Kind } from './StatusPill.svelte';
  import type { Row } from './types';

  /** What to know of another task at a glance: its number and state, what it is, and where it sits. */
  let { row }: { row: Row } = $props();

  const kind = $derived<Kind>(
    row.status === 'pending' && row.virtual_tags?.includes('WAITING') ? 'waiting' : (row.status as Kind),
  );
</script>

<span class="head">
  {#if row.id != null}<span class="dim mono">#{row.id}</span>{/if}
  <StatusPill {kind} />
</span>
<strong class="desc">{row.description}</strong>
<span class="meta dim">
  {#if row.project}{row.project}{/if}
  {#if row.priority}· priority {row.priority}{/if}
  {#if row.due != null}· due {formatMoment(row.due, undefined, undefined)}{/if}
  {#if row.tags.length}· +{row.tags.join(' +')}{/if}
</span>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .desc {
    overflow-wrap: anywhere;
  }
  .meta {
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
</style>

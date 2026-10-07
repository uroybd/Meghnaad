<script lang="ts">
  import type { Completer } from './completer.svelte';

  // Shown above (console) or below (filter bar) its input.
  let { completer, placement = 'above' }: { completer: Completer; placement?: 'above' | 'below' } = $props();
  const menu = $derived(completer.menu);
</script>

{#if menu}
  <ul class="menu {placement} mono" role="listbox" aria-label="Completions">
    {#each menu.options as o, i (o.value)}
      <li role="option" aria-selected={i === menu.sel} class:sel={i === menu.sel}>
        <span class="v">{o.value}</span>
        {#if o.hint}<span class="h dim">{o.hint}</span>{/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .menu {
    position: absolute; left: 0; z-index: 30; margin: 0; padding: 4px; list-style: none;
    min-width: 16em; max-width: min(36em, 94vw); max-height: 15em; overflow: auto;
    background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius);
    box-shadow: 0 6px 22px rgb(0 0 0 / 0.18); font-size: 13px;
  }
  .menu.above { bottom: calc(100% + 4px); }
  .menu.below { top: calc(100% + 4px); }
  li { display: flex; gap: 14px; justify-content: space-between; padding: 1px 8px; border-radius: 4px; white-space: nowrap; }
  li.sel { background: var(--accent); color: var(--accent-text); }
  li.sel .h { color: inherit; opacity: 0.85; }
  .h { overflow: hidden; text-overflow: ellipsis; }
</style>

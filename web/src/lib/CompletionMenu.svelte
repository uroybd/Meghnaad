<script lang="ts">
  import type { Completer } from './completer.svelte';

  // Shown above (console) or below (filter bar) its input.
  let {
    completer,
    placement = 'above',
    onpick,
  }: {
    completer: Completer;
    placement?: 'above' | 'below';
    /** Tapped an option (touch has no Tab key): the parent applies it to its input. */
    onpick?: (index: number) => void;
  } = $props();
  const menu = $derived(completer.menu);
</script>

{#if menu}
  <ul class="menu {placement} mono" role="listbox" aria-label="Completions">
    {#each menu.options as o, i (o.value)}
      <!-- Act on click, not pointerdown: on touch the menu would vanish mid-tap and the click would land
           on whatever is underneath. mousedown's default is the focus change that would blur the input
           (and close the menu) before the click, so it is cancelled. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- Keyboard users drive the menu from the input (Tab, arrows, Enter), not by focusing options. -->
      <li
        role="option"
        aria-selected={i === menu.sel}
        class:sel={i === menu.sel}
        onmousedown={(e) => e.preventDefault()}
        onclick={() => onpick?.(i)}
      >
        <span class="v">{o.value}</span>
        {#if o.hint}<span class="h dim">{o.hint}</span>{/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .menu {
    position: absolute;
    left: 0;
    z-index: 30;
    margin: 0;
    padding: 4px;
    list-style: none;
    min-width: 16em;
    max-width: min(36em, 94vw);
    max-height: 15em;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 6px 22px rgb(0 0 0 / 0.18);
    font-size: 13px;
  }
  .menu.above {
    bottom: calc(100% + 4px);
  }
  .menu.below {
    top: calc(100% + 4px);
  }
  li {
    cursor: pointer;
    display: flex;
    gap: 14px;
    justify-content: space-between;
    padding: 1px 8px;
    border-radius: 4px;
    white-space: nowrap;
  }
  li.sel {
    background: var(--accent);
    color: var(--accent-text);
  }
  li.sel .h {
    color: inherit;
    opacity: 0.85;
  }
  .h {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  @media (pointer: coarse) {
    li {
      padding: 9px 8px;
    }
  }
</style>

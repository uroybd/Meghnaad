<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import { ChevronDown } from './icons';

  /**
   * A page in the top bar that holds several of something (boards, kanbans): its button opens a menu of them, so one
   * can be opened directly, with the page's own list last.
   */
  let {
    label,
    on,
    items,
    current,
    all,
    onpick,
    children,
  }: {
    /** The page's name, also the button's accessible name (the bar's preloading reads it). */
    label: string;
    /** This page is shown. */
    on: boolean;
    items: { id: string; name: string }[];
    /** The one shown now, if any. */
    current: string | null;
    /** What the last entry, the page's list, is called ("All boards"). */
    all: string;
    /** An entry chosen; null for the list. */
    onpick: (id: string | null) => void;
    /** The button's icon and label. */
    children: Snippet;
  } = $props();

  let open = $state(false);
  let button: HTMLButtonElement | undefined = $state();
  let menu: HTMLElement | undefined = $state();
  let place = $state({ left: 0, top: 0 });

  const entries = () => [...(menu?.querySelectorAll<HTMLElement>('[role=menuitem]') ?? [])];

  async function show() {
    if (!button) return;
    const r = button.getBoundingClientRect();
    place = { left: Math.max(8, Math.min(r.left, window.innerWidth - 248)), top: r.bottom + 4 };
    open = true;
    await tick();
    (entries().find((e) => e.getAttribute('aria-current') === 'true') ?? entries()[0])?.focus();
  }

  function close(refocus = false) {
    open = false;
    if (refocus) button?.focus();
  }

  function pick(id: string | null) {
    close();
    onpick(id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) return;
    const all = entries();
    const at = all.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      all[(at + (e.key === 'ArrowDown' ? 1 : -1) + all.length) % all.length]?.focus();
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      all[e.key === 'Home' ? 0 : all.length - 1]?.focus();
    } else if (e.key === 'Tab') {
      close();
    }
  }

  function away(e: Event) {
    if (open && !menu?.contains(e.target as Node) && !button?.contains(e.target as Node)) close();
  }
</script>

<svelte:window onpointerdown={away} onresize={() => open && close()} {onkeydown} />

<button
  bind:this={button}
  class:on
  aria-label={label}
  aria-haspopup="menu"
  aria-expanded={open}
  onclick={() => (open ? close() : void show())}
  >{@render children()}<span class="chev"><ChevronDown size={13} /></span></button
>
{#if open}
  <div
    bind:this={menu}
    class="menu"
    role="menu"
    aria-label={label}
    style:left="{place.left}px"
    style:top="{place.top}px"
  >
    {#each items as it (it.id)}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        aria-current={on && current === it.id ? 'true' : undefined}
        onclick={() => pick(it.id)}>{it.name}</button
      >
    {/each}
    {#if items.length}<hr />{/if}
    <button
      type="button"
      role="menuitem"
      tabindex="-1"
      class="all"
      aria-current={on && current === null ? 'true' : undefined}
      onclick={() => pick(null)}>{all}</button
    >
  </div>
{/if}

<style>
  .chev {
    display: inline-flex;
    margin-left: -2px;
    opacity: 0.6;
  }
  .menu {
    position: fixed;
    z-index: 40;
    min-width: 200px;
    max-width: 240px;
    max-height: min(420px, calc(100vh - 80px));
    overflow: auto;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.22);
    display: flex;
    flex-direction: column;
  }
  .menu button {
    display: block;
    width: 100%;
    padding: 7px 10px;
    border-color: transparent;
    background: transparent;
    text-align: left;
    line-height: 1.3;
    font-weight: normal;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--panel-2);
  }
  .menu button[aria-current='true'] {
    font-weight: 600;
    color: var(--accent);
  }
  .menu button.all {
    color: var(--dim);
  }
  .menu hr {
    margin: 4px 0;
    border: 0;
    border-top: 1px solid var(--line);
  }
</style>

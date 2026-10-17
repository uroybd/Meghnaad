<script lang="ts">
  import { tick, type Component } from 'svelte';
  import type { TaskCommand } from './appsettings';
  import CommandIcon from './CommandIcon.svelte';
  import { Ellipsis } from './icons';

  /** One of the app's own buttons, folded into the menu where there is no room for it. */
  export interface MenuAction {
    label: string;
    icon?: Component<{ size?: number }>;
    run: () => void;
    danger?: boolean;
    disabled?: boolean;
    /** Leave the menu open after it runs (a delete that asks to be clicked twice). */
    stay?: boolean;
  }

  /**
   * The task commands from settings.json under one "⋯" button, so the buttons for the everyday actions stay few.
   * Where a row or card has no room for its buttons, they come here too (`actions`), ahead of the commands.
   * It draws nothing when there are none of either.
   */
  let {
    commands = [],
    actions = [],
    onrun = () => {},
    label = 'More actions',
    disabled = false,
  }: {
    commands?: TaskCommand[];
    actions?: MenuAction[];
    onrun?: (c: TaskCommand) => void;
    label?: string;
    disabled?: boolean;
  } = $props();

  let open = $state(false);
  let button: HTMLButtonElement | undefined = $state();
  let menu: HTMLElement | undefined = $state();
  // Where the panel goes: fixed to the window, so a table cell can't clip it, and above the button when there is
  // more room there.
  let place = $state<{ right: number; top?: number; bottom?: number; max: number }>({ right: 8, top: 0, max: 320 });

  async function show() {
    if (!button) return;
    const r = button.getBoundingClientRect();
    const room = { below: window.innerHeight - r.bottom - 8, above: r.top - 8 };
    const up = room.below < 200 && room.above > room.below;
    place = {
      right: Math.max(8, window.innerWidth - r.right),
      ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
      max: Math.min(360, up ? room.above : room.below),
    };
    open = true;
    await tick();
    items()[0]?.focus();
  }

  const items = () => [...(menu?.querySelectorAll<HTMLElement>('[role=menuitem]:not(:disabled)') ?? [])];

  function close(refocus = false) {
    open = false;
    if (refocus) button?.focus();
  }

  function run(c: TaskCommand) {
    close();
    onrun(c);
  }

  function runAction(a: MenuAction) {
    if (!a.stay) close();
    a.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) return;
    const all = items();
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

  // The page, or anything on it that scrolls (a wide table), moving under the menu closes it; the menu's own list
  // scrolling does not.
  function scrolled(e: Event) {
    if (open && !menu?.contains(e.target as Node)) close();
  }

  // A click anywhere else closes it.
  function away(e: Event) {
    if (open && !menu?.contains(e.target as Node) && !button?.contains(e.target as Node)) close();
  }
</script>

<svelte:window onpointerdown={away} onscrollcapture={scrolled} onresize={() => open && close()} {onkeydown} />

{#if commands.length || actions.length}
  <span class="more">
    <button
      bind:this={button}
      type="button"
      class="ghost trigger"
      aria-haspopup="menu"
      aria-expanded={open}
      aria-label={label}
      title={label}
      {disabled}
      onclick={() => (open ? close() : void show())}><Ellipsis size={16} /></button
    >
    {#if open}
      <div
        bind:this={menu}
        class="menu"
        role="menu"
        aria-label={label}
        style:right="{place.right}px"
        style:top={place.top === undefined ? undefined : `${place.top}px`}
        style:bottom={place.bottom === undefined ? undefined : `${place.bottom}px`}
        style:max-height="{place.max}px"
      >
        {#each actions as a, i (i)}
          {@const Icon = a.icon}
          <button
            type="button"
            role="menuitem"
            tabindex="-1"
            class:danger={a.danger}
            disabled={a.disabled}
            onclick={() => runAction(a)}
          >
            {#if Icon}<Icon size={15} />{/if}
            <span class="name">{a.label}</span>
          </button>
        {/each}
        {#if actions.length && commands.length}<hr />{/if}
        {#each commands as c (c.name)}
          <button type="button" role="menuitem" tabindex="-1" title={c.command} onclick={() => run(c)}>
            <CommandIcon name={c.icon} size={15} />
            <span class="name">{c.name}</span>
            <span class="what dim mono">{c.command}</span>
          </button>
        {/each}
      </div>
    {/if}
  </span>
{/if}

<style>
  .more {
    display: inline-flex;
  }
  .trigger {
    line-height: 0;
    padding: 4px 6px;
  }
  .menu {
    position: fixed;
    z-index: 40;
    min-width: 200px;
    max-width: min(340px, calc(100vw - 16px));
    overflow: auto;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.22);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-color: transparent;
    background: transparent;
    text-align: left;
    line-height: 1.3;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--panel-2);
  }
  .menu button:disabled {
    opacity: 0.45;
  }
  .menu button.danger {
    color: var(--err);
  }
  .menu hr {
    margin: 4px 0;
    border: 0;
    border-top: 1px solid var(--line);
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
  }
  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    text-align: right;
  }
  @media (max-width: 760px) {
    .menu button {
      min-height: 44px;
    }
  }
</style>

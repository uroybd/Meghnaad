<script lang="ts">
  import { Check, Pencil, Play, Square, Trash2 } from './icons';
  import MoreMenu, { type MenuAction } from './MoreMenu.svelte';
  import { store, type Entry } from './store.svelte';
  import { commandSteps } from './taskCommands';
  import type { Row } from './types';

  let {
    row,
    entry,
    onedit,
    card,
    folded,
    pending = $bindable(),
  }: {
    row: Row;
    entry: Entry | null;
    onedit?: (row: Row) => void;
    /** The row is a card (a phone): one button, Done, and the rest in the ⋯ menu. */
    card: boolean;
    /** The table is too wide for the buttons: they are in the ⋯ menu. */
    folded: boolean;
    /** The task whose Delete was clicked once and waits for the second click, for the whole table (one at a time). */
    pending: string | null;
  } = $props();

  const isOpen = $derived(row.status === 'pending');

  function act(...rest: string[]) {
    store.act(entry, [row.uuid, ...rest]);
  }

  // The second click is the answer to Taskwarrior's "Delete task N?" (the `confirmation` setting);
  // with it off, the first click deletes.
  function del() {
    if (!store.confirmation || pending === row.uuid) {
      pending = null;
      store.act(entry, [row.uuid, 'delete'], { approved: [row.uuid] });
    } else {
      const uuid = row.uuid;
      pending = uuid;
      setTimeout(() => pending === uuid && (pending = null), 3000);
    }
  }

  // What a row's buttons are, in the menu they fold into (without Done, on a card, which has its own button).
  function menuActions(withDone = true): MenuAction[] {
    const list: MenuAction[] = [];
    if (isOpen) {
      if (withDone) list.push({ label: 'Done', icon: Check, run: () => act('done') });
      list.push(
        row.start != null
          ? { label: 'Stop', icon: Square, run: () => act('stop') }
          : { label: 'Start', icon: Play, run: () => act('start') },
      );
    }
    if (onedit) list.push({ label: 'Edit', icon: Pencil, run: () => onedit(row) });
    if (row.status !== 'deleted') {
      const asking = store.confirmation && pending !== row.uuid;
      list.push({
        label: pending === row.uuid ? 'Click again to delete' : 'Delete',
        icon: Trash2,
        danger: true,
        stay: asking,
        run: del,
      });
    }
    return list;
  }
</script>

{#if card}
  {#if isOpen}
    <button class="ghost" title="Done" aria-label="Done" onclick={() => act('done')}><Check size={16} /></button>
  {/if}
{:else if !folded}
  {#if isOpen}
    <button class="ghost" title="Done" aria-label="Done" onclick={() => act('done')}><Check size={16} /></button>
    {#if row.start != null}
      <button class="ghost" title="Stop" aria-label="Stop" onclick={() => act('stop')}
        ><Square size={14} fill="currentColor" /></button
      >
    {:else}
      <button class="ghost" title="Start" aria-label="Start" onclick={() => act('start')}><Play size={14} /></button>
    {/if}
  {/if}
  {#if onedit}
    <button class="ghost" title="Edit" aria-label="Edit" onclick={() => onedit(row)}><Pencil size={14} /></button>
  {/if}
  {#if row.status !== 'deleted'}
    <button
      class="ghost danger"
      title={pending === row.uuid ? 'Click again to delete' : 'Delete'}
      aria-label="Delete"
      onclick={del}
    >
      {#if pending === row.uuid}sure?{:else}<Trash2 size={14} />{/if}
    </button>
  {/if}
{/if}
<MoreMenu
  commands={store.menuCommands}
  actions={card ? menuActions(false) : folded ? menuActions() : []}
  onrun={(c) =>
    store.actSteps(
      entry,
      commandSteps(c).map((s) => ({ ...s, args: [row.uuid, ...s.words] })),
    )}
/>

<style>
  /* The cell is the table's (`td.actions`); these are its buttons. */
  :global(.actions) button {
    padding: 3px 6px;
    line-height: 0;
  }
  :global(.actions) button:has(:not(svg)) {
    line-height: inherit;
  }
  /* A card's buttons: quiet, with the tick in green. */
  @media (max-width: 760px) {
    :global(td.actions) button {
      min-width: 36px;
      min-height: 32px;
      border-color: transparent;
      background: transparent;
      color: var(--dim);
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    :global(td.actions) button[aria-label='Done'] {
      color: var(--ok);
    }
    :global(td.actions) button.danger {
      color: color-mix(in srgb, var(--err) 70%, var(--dim));
    }
  }
</style>

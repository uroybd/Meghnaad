<script lang="ts">
  import { untrack } from 'svelte';
  import { CalendarDays, Download, Ellipsis, Trash2 } from './icons';
  import { includeLine, ROOT } from './rcpaths';

  let {
    current,
    others,
    busy,
    canDownload,
    made,
    ondownload,
    onupdate,
    oninclude,
    ondelete,
  }: {
    /** The file shown. */
    current: string;
    /** The folder's other files, which it can be included in. */
    others: string[];
    busy: boolean;
    /** The file is read, so its saved text can be downloaded. */
    canDownload: boolean;
    /** The file's text when it is holidays this app made (and not edited), so they can be fetched again. */
    made: string | null;
    ondownload: () => void;
    onupdate: (text: string) => void;
    /** Put `include <this file>` in `target`'s box. */
    oninclude: (target: string) => void;
    ondelete: () => void;
  } = $props();

  let open = $state(false);
  let includeIn = $state(ROOT);
  let confirmDelete = $state(false);
  const isRoot = $derived(current === ROOT);

  // Another file shown: the menu closes, and a Delete clicked once is forgotten.
  $effect.pre(() => {
    void current;
    untrack(() => {
      open = false;
      confirmDelete = false;
    });
  });

  // The second click deletes.
  function remove() {
    if (isRoot) return;
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 4000);
      return;
    }
    confirmDelete = false;
    ondelete();
  }
</script>

<svelte:window
  onclick={(e) => {
    if (open && !(e.target as Element).closest?.('.more')) open = false;
  }}
/>

<div class="more">
  <button
    class="ghost icon"
    aria-label="More for this file"
    title="More for this file"
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={() => (open = !open)}><Ellipsis size={16} /></button
  >
  {#if open}
    <div class="menu" role="menu" aria-label="This file">
      <button role="menuitem" onclick={ondownload} disabled={!canDownload} title="Download the saved file"
        ><Download size={14} /> Download</button
      >
      {#if made}
        <button
          role="menuitem"
          onclick={() => {
            onupdate(made);
            open = false;
          }}
          title="Fetch these holidays again, for the current years"><CalendarDays size={14} /> Update holidays</button
        >
      {/if}
      {#if !isRoot && others.length}
        <div class="include">
          <label for="rc-include">Include this file in</label>
          <select id="rc-include" bind:value={includeIn}>
            {#each others as p (p)}<option value={p}>{p}</option>{/each}
          </select>
          <button class="ghost" onclick={() => oninclude(includeIn)} title={includeLine(includeIn, current)}
            >Include</button
          >
          <span class="dim small">adds <code>{includeLine(includeIn, current)}</code> to that file; Apply keeps it</span
          >
        </div>
      {/if}
      {#if !isRoot}
        <button role="menuitem" class="danger" onclick={remove} disabled={busy} title="Delete this file">
          <Trash2 size={14} />
          {confirmDelete ? 'Click again to delete' : 'Delete file'}
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .icon {
    line-height: 0;
  }
  .more {
    position: relative;
  }
  .menu {
    position: absolute;
    z-index: 20;
    right: 0;
    top: calc(100% + 4px);
    width: 280px;
    display: grid;
    gap: 2px;
    padding: 6px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
  .menu > button {
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    background: none;
    border: 0;
    padding: 7px 8px;
    border-radius: 5px;
  }
  .menu > button:hover {
    background: var(--panel-2);
  }
  .include {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    padding: 6px 8px;
    border-block: 1px solid var(--border);
  }
  .include select {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .danger {
    color: var(--err);
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
</style>

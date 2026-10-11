<script lang="ts">
  import { tick } from 'svelte';
  import { X } from './icons';
  import { review } from './review.svelte';
  import ReviewBar from './ReviewBar.svelte';
  import TaskInfo from './TaskInfo.svelte';

  // The review with buttons: the same session the console drives, with the keys of tasksh's menu.
  let dialog: HTMLDialogElement;
  let modifyInput: HTMLInputElement | undefined = $state();
  let args = $state('');

  $effect(() => {
    dialog.showModal();
  });
  $effect(() => {
    if (review.phase === 'modifying' && !review.busy) void tick().then(() => modifyInput?.focus());
  });

  // The keys of the menu (and y, n to a question), unless something is being typed. Enter is the focused button's.
  function keys(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (e.metaKey || e.ctrlKey || e.altKey || t?.tagName === 'INPUT' || t?.tagName === 'TEXTAREA') return;
    if (e.key.length !== 1 || e.key === ' ') return;
    e.preventDefault();
    void review.press(e.key);
  }

  function modify(e: Event) {
    e.preventDefault();
    const words = args;
    args = '';
    void review.modify(words);
  }
</script>

<!-- Closing the dialog itself (Escape) ends the review; there is no card left to show a summary in. -->
<dialog bind:this={dialog} onclose={() => review.close()} onkeydown={keys} aria-label="Review">
  <header class="head row">
    {#if review.phase === 'running' || review.phase === 'asking' || review.phase === 'modifying' || review.phase === 'editing'}
      <span class="count mono" data-testid="review-count">[{review.current + 1} of {review.total}]</span>
    {/if}
    <strong class="grow title">{review.row?.description ?? 'Review'}</strong>
    <button class="ghost" aria-label="Quit the review" title="Quit the review" onclick={() => review.quit()}
      ><X size={18} /></button
    >
  </header>

  {#if review.phase === 'done'}
    <p class="end" class:err={review.messageKind === 'err'} role="status">{review.message}</p>
    <div class="row buttons">
      <button class="primary" onclick={() => review.close()}>Close</button>
    </div>
  {:else}
    {#if review.row}
      <div class="info"><TaskInfo task={review.row} embedded /></div>
    {:else if review.phase !== 'setup'}
      <p class="dim" role="status">{review.message || 'Looking for tasks to review…'}</p>
    {/if}

    {#if review.phase === 'modifying'}
      <form class="modify row" onsubmit={modify}>
        <input
          bind:this={modifyInput}
          bind:value={args}
          class="grow mono"
          aria-label="Modification"
          placeholder="+tag -tag /teh/the/ project:X"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
        />
        <button class="primary" disabled={review.busy}>Modify</button>
        <button type="button" disabled={review.busy} onclick={() => review.modify('')}>Cancel</button>
      </form>
    {/if}

    {#if review.message && review.phase !== 'setup' && review.phase !== 'asking'}
      <p class="msg" class:err={review.messageKind === 'err'} role="status">{review.message}</p>
    {/if}

    <div class="foot"><ReviewBar inline focusFirst /></div>
  {/if}
</dialog>

<style>
  dialog {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    padding: 18px 22px;
    width: min(760px, 96vw);
    max-height: 94vh;
    overflow: auto;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  .head {
    gap: 10px;
    align-items: center;
    margin-bottom: 8px;
  }
  .count {
    background: var(--accent);
    color: #fff;
    border-radius: 4px;
    padding: 1px 7px;
    font-size: 13px;
    white-space: nowrap;
  }
  .title {
    font-size: 17px;
    overflow-wrap: anywhere;
  }
  .end {
    margin: 14px 0;
  }
  .msg {
    margin: 10px 0 0;
    color: var(--dim);
    font-size: 13.5px;
  }
  .err {
    color: var(--err);
  }
  .modify {
    gap: 8px;
    margin-top: 12px;
  }
  .buttons {
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 14px;
    align-items: center;
  }
  .foot {
    margin-top: 14px;
  }
  @media (max-width: 760px) {
    dialog {
      padding: 14px 12px;
    }
    .buttons button {
      min-height: 40px;
    }
  }
</style>

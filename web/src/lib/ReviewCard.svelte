<script lang="ts">
  import { tick } from 'svelte';
  import { Check, Pencil, SkipForward, SquarePen, Trash2, X } from './icons';
  import { review } from './review.svelte';
  import TaskInfo from './TaskInfo.svelte';
  import { menuKey } from './tasksh';

  // The review with buttons: the same session the console's prompt drives, with the keys of tasksh's menu.
  let dialog: HTMLDialogElement;
  let reviewedButton: HTMLButtonElement | undefined = $state();
  let modifyInput: HTMLInputElement | undefined = $state();
  let args = $state('');

  $effect(() => {
    dialog.showModal();
  });

  const choosing = $derived(review.phase === 'running' && !review.busy);

  // The button for the usual answer has the focus, so Enter marks the task reviewed, as it does in tasksh. It can only
  // take it while the choices are on (the card opens while the review is still starting, and a button that is off
  // can't be focused), so this runs whenever they come on, which is also each time another task is shown.
  $effect(() => {
    void review.row;
    if (choosing) void tick().then(() => reviewedButton?.focus());
  });
  $effect(() => {
    if (review.phase === 'modifying' && !review.busy) void tick().then(() => modifyInput?.focus());
  });

  // The letters of the menu, unless something is being typed.
  function keys(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (e.metaKey || e.ctrlKey || e.altKey || t?.tagName === 'INPUT' || t?.tagName === 'TEXTAREA') return;
    if (e.key.length !== 1 || e.key === ' ') return;
    const key = menuKey(e.key);
    if (key && choosing) {
      e.preventDefault();
      void review.act(key);
    }
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
    {#if review.phase === 'running' || review.phase === 'modifying' || review.phase === 'editing'}
      <span class="count mono" data-testid="review-count">[{review.current + 1} of {review.total}]</span>
    {/if}
    <strong class="grow title">{review.row?.description ?? 'Review'}</strong>
    <button class="ghost" aria-label="Quit the review" title="Quit the review" onclick={() => review.quit()}
      ><X size={18} /></button
    >
  </header>

  {#if review.phase === 'setup'}
    <p class="question" data-testid="review-question">{review.question}</p>
    <div class="row buttons">
      <button class="primary" disabled={review.busy} onclick={() => review.answerSetup(true)}>Yes</button>
      <button disabled={review.busy} onclick={() => review.answerSetup(false)}>No</button>
    </div>
  {:else if review.phase === 'done'}
    <p class="end" class:err={review.messageKind === 'err'} role="status">{review.message}</p>
    <div class="row buttons">
      <button class="primary" onclick={() => review.close()}>Close</button>
    </div>
  {:else if review.row}
    <div class="info"><TaskInfo task={review.row} embedded /></div>

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

    {#if review.message}
      <p class="msg" class:err={review.messageKind === 'err'} role="status">{review.message}</p>
    {/if}

    <footer class="row buttons">
      <button
        bind:this={reviewedButton}
        class="primary btn"
        disabled={!choosing}
        title="Mark as reviewed (Enter or r)"
        onclick={() => review.act('reviewed')}><Check size={15} /> Reviewed</button
      >
      <button
        class="btn"
        disabled={!choosing}
        title="Skip it: it is not counted as reviewed (s)"
        onclick={() => review.act('skip')}><SkipForward size={14} /> Skip</button
      >
      <button
        class="btn"
        disabled={!choosing}
        title="Edit it, then mark it reviewed (e)"
        onclick={() => review.act('edit')}><SquarePen size={14} /> Edit</button
      >
      <button
        class="btn"
        disabled={!choosing}
        title="Change it with modify arguments (m)"
        onclick={() => review.act('modify')}><Pencil size={14} /> Modify</button
      >
      <button class="btn" disabled={!choosing} title="Complete it (c)" onclick={() => review.act('complete')}
        ><Check size={14} /> Complete</button
      >
      <button class="btn danger" disabled={!choosing} title="Delete it (d)" onclick={() => review.act('delete')}
        ><Trash2 size={14} /> Delete</button
      >
      <span class="grow"></span>
      <button
        class="ghost"
        disabled={review.busy}
        title="Stop here: what is done stays done (q)"
        onclick={() => review.act('quit')}>Quit</button
      >
    </footer>
  {:else}
    <p class="dim" role="status">{review.message || 'Looking for tasks to review…'}</p>
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
  .question,
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
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
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

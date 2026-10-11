<script lang="ts">
  import { tick } from 'svelte';
  import { Check, PanelTop, SkipForward, SquarePen, Pencil, Trash2 } from './icons';
  import { review } from './review.svelte';
  import { REVIEW_MODIFY_PROMPT } from './tasksh';

  /**
   * What a review can be answered with, as buttons: the choices of tasksh's menu while it asks what to do with a task,
   * Yes and No while it asks a question, and a way out while a change is being typed. Each says its key, which does the
   * same. `inline`: the card has its own field for the change, so no bar is shown for it. `focusFirst`: the usual
   * answer has the focus, so Enter is a click on it (the card; the console takes Enter as a key instead).
   */
  let { inline = false, focusFirst = false }: { inline?: boolean; focusFirst?: boolean } = $props();

  let first: HTMLButtonElement | undefined = $state();
  const choosing = $derived(review.phase === 'running' && !review.busy);

  // It can only take the focus while it is on (a button that is off can't), so this runs whenever the choices come on,
  // which is also each time another task is shown.
  $effect(() => {
    void review.row;
    if (focusFirst && choosing) void tick().then(() => first?.focus());
  });

  const yes = () => (review.phase === 'setup' ? review.answerSetup(true) : review.answer(true));
  const no = () => (review.phase === 'setup' ? review.answerSetup(false) : review.answer(false));
</script>

{#if review.phase === 'setup' || review.phase === 'asking'}
  <div class="bar" role="group" aria-label="Question">
    <span class="q" data-testid="review-question">{review.question}</span>
    <span class="grow"></span>
    <button class="primary" bind:this={first} disabled={review.busy} onclick={yes}>Yes <kbd>y</kbd></button>
    <button disabled={review.busy} onclick={no}>No <kbd>n</kbd></button>
  </div>
{:else if review.phase === 'running'}
  <div class="bar" role="toolbar" aria-label="Review">
    <button
      bind:this={first}
      class="primary btn"
      disabled={!choosing}
      title="Mark as reviewed"
      onclick={() => review.act('reviewed')}><Check size={15} /> Reviewed <kbd>Enter</kbd></button
    >
    <button
      class="btn"
      disabled={!choosing}
      title="Skip it: it is not counted as reviewed"
      onclick={() => review.act('skip')}><SkipForward size={14} /> Skip <kbd>s</kbd></button
    >
    <button class="btn" disabled={!choosing} title="Edit it, then mark it reviewed" onclick={() => review.act('edit')}
      ><SquarePen size={14} /> Edit <kbd>e</kbd></button
    >
    <button
      class="btn"
      disabled={!choosing}
      title="Change it with modify arguments"
      onclick={() => review.act('modify')}><Pencil size={14} /> Modify <kbd>m</kbd></button
    >
    <button class="btn" disabled={!choosing} title="Complete it" onclick={() => review.act('complete')}
      ><Check size={14} /> Complete <kbd>c</kbd></button
    >
    <button class="btn danger" disabled={!choosing} title="Delete it" onclick={() => review.act('delete')}
      ><Trash2 size={14} /> Delete <kbd>d</kbd></button
    >
    <span class="grow"></span>
    {#if !inline}
      <button
        class="ghost btn"
        title="Carry on in the review card, with buttons and the task's details"
        onclick={() => review.toCard()}><PanelTop size={14} /> Open as card</button
      >
    {/if}
    <button
      class="ghost"
      disabled={review.busy}
      title="Stop here: what is done stays done"
      onclick={() => review.act('quit')}>Quit <kbd>q</kbd></button
    >
  </div>
{:else if review.phase === 'modifying' && !inline}
  <div class="bar" role="group" aria-label="Change">
    <span class="q">{REVIEW_MODIFY_PROMPT}</span>
    <span class="grow"></span>
    <button disabled={review.busy} onclick={() => review.modify('')}>Cancel <kbd>Esc</kbd></button>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
  }
  .q {
    font-size: 13.5px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  kbd {
    font-family: var(--mono);
    font-size: 11px;
    opacity: 0.65;
    border: 1px solid currentColor;
    border-radius: 4px;
    padding: 0 4px;
    line-height: 1.4;
  }
  @media (max-width: 760px) {
    .bar button {
      min-height: 40px;
    }
    kbd {
      display: none;
    }
  }
</style>

<script lang="ts">
  import { asker } from './ask.svelte';

  /** The page's own yes-or-no question (ask.svelte.ts). Escape or a click outside is no; Enter is yes. */
  let dialog: HTMLDialogElement | undefined = $state();
  let yesButton: HTMLButtonElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (asker.current && !dialog.open) {
      dialog.showModal();
      yesButton?.focus();
    } else if (!asker.current && dialog.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-label="Question"
  oncancel={(e) => {
    e.preventDefault();
    asker.answer(false);
  }}
  onclick={(e) => {
    if (e.target === dialog) asker.answer(false);
  }}
>
  {#if asker.current}
    <p>{asker.current.message}</p>
    <footer>
      <button onclick={() => asker.answer(false)}>{asker.current.no}</button>
      <button class="primary" bind:this={yesButton} onclick={() => asker.answer(true)}>{asker.current.yes}</button>
    </footer>
  {/if}
</dialog>

<style>
  dialog {
    width: min(440px, calc(100vw - 32px));
    padding: 18px 20px 16px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--panel);
    color: var(--text);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 35%);
  }
  p {
    margin: 0 0 16px;
    white-space: pre-wrap;
    line-height: 1.45;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>

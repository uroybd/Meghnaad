<script lang="ts">
  import type { Snippet } from 'svelte';
  import { X } from './icons';

  /**
   * A panel that slides in from the right (the whole width on a phone) over the page, for a form that belongs to one thing on
   * it. Escape, the close button and a click outside close it; focus goes back to where it was.
   */
  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  $effect(() => {
    if (dialog && !dialog.open) dialog.showModal();
  });
</script>

<dialog
  bind:this={dialog}
  aria-label={title}
  {onclose}
  onclick={(e) => {
    if (e.target === dialog) dialog?.close();
  }}
>
  <header>
    <h2>{title}</h2>
    <button class="ghost" aria-label="Close" title="Close" onclick={() => dialog?.close()}><X size={16} /></button>
  </header>
  <div class="body">{@render children()}</div>
</dialog>

<style>
  dialog {
    position: fixed;
    inset: 0 0 0 auto;
    margin: 0;
    padding: 0;
    width: min(480px, 100vw);
    max-width: 100vw;
    height: 100dvh;
    max-height: none;
    background: var(--panel);
    color: var(--text);
    border: 0;
    border-left: 1px solid var(--line);
    box-shadow: -8px 0 24px rgb(0 0 0 / 0.25);
  }
  dialog[open] {
    display: flex;
    flex-direction: column;
    animation: slide 0.18s ease-out;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }
  @keyframes slide {
    from {
      transform: translateX(100%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    dialog[open] {
      animation: none;
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 16px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 14px;
  }
</style>

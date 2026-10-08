<script lang="ts">
  import { Check, Copy } from './icons';
  import { store } from './store.svelte';

  /** Shows `text` (a task id); hovering or focusing it offers the full `uuid` with a Copy button. */
  let { text, uuid }: { text: string; uuid: string } = $props();

  let copied = $state(false);
  let tip: HTMLElement;

  // Fixed-positioned under the id so a scrolling container can't clip it, yet still inside this
  // element so the pointer can travel onto it to click Copy.
  function place(e: Event) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    tip.style.left = `${Math.max(8, Math.min(r.left, window.innerWidth - tip.offsetWidth - 8))}px`;
    tip.style.top = `${r.bottom - 2}px`;
  }

  async function copy(e: MouseEvent) {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(uuid);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      store.notify('Could not copy: select the id and copy it by hand.', 'err');
    }
  }
</script>

<span class="uuidtip" role="group" aria-label="Task id {text}" tabindex="-1" onmouseenter={place} onfocusin={place}>
  <span class="idtext">{text}</span>
  <span class="tip" bind:this={tip}>
    <code>{uuid}</code>
    <button type="button" class="copy" onclick={copy}>
      {#if copied}<Check size={13} /> Copied{:else}<Copy size={13} /> Copy{/if}
    </button>
  </span>
</span>

<style>
  .uuidtip { display: inline-block; cursor: default; outline: none; }
  .tip {
    display: none; position: fixed; z-index: 30; align-items: center; gap: 10px;
    padding: 6px 8px; font-weight: 400; white-space: nowrap; color: var(--text);
    background: var(--panel); border: 1px solid var(--line); border-radius: 6px;
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.18);
  }
  .uuidtip:hover .tip, .uuidtip:focus-within .tip { display: flex; }
  code { font-family: var(--mono); font-size: 12px; user-select: all; }
  .copy { display: inline-flex; align-items: center; gap: 4px; padding: 2px 8px; font-size: 12px; }
</style>

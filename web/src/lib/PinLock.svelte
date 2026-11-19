<script lang="ts">
  // The PIN screen of a trusted device (spec 0022): a blurred layer over the app, gone with the four keys of the PIN. A
  // modal dialog, so it sits above any drawer that is open and nothing under it can be clicked, typed into or reached
  // with Tab; the app runs on underneath. It is a visual block, not a lock: the full lock is Forgot the PIN?
  import { onMount } from 'svelte';
  import { lock, onPin, pinUnlock } from '../engine/session';
  import { PIN_LENGTH, pinKey } from '../engine/pin';

  let up = $state(false);
  let keys = $state<string[]>([]);
  let wrong = $state(false);
  let dialog: HTMLDialogElement | undefined = $state();

  onMount(() =>
    onPin((locked) => {
      up = locked;
      keys = [];
    }),
  );

  $effect(() => {
    if (!dialog) return;
    if (up && !dialog.open) dialog.showModal();
    if (!up && dialog.open) dialog.close();
  });

  async function press(e: KeyboardEvent) {
    if (!up) return;
    // A button of the screen (Forgot the PIN?) still takes Enter, Space and Tab.
    if ((e.target as Element | null)?.closest?.('button') && ['Enter', ' ', 'Tab'].includes(e.key)) return;
    if (e.key === 'Tab') return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Backspace') {
      keys = keys.slice(0, -1);
      return;
    }
    const k = pinKey(e);
    if (!k || keys.length >= PIN_LENGTH) return;
    wrong = false;
    keys = [...keys, k];
    if (keys.length < PIN_LENGTH) return;
    const tried = keys;
    if (!(await pinUnlock(tried))) {
      wrong = true;
      keys = [];
    }
  }
</script>

<svelte:window onkeydowncapture={press} />

<dialog
  bind:this={dialog}
  class="pin"
  aria-label="Locked: type the PIN"
  oncancel={(e) => e.preventDefault()}
  onclose={() => {
    // Closed from outside while still locked (a browser that closes a modal on its own): open it again.
    if (up) dialog?.showModal();
  }}
>
  <p class="title">Locked</p>
  <p class="dim">Type the PIN</p>
  <div class="dots" class:wrong aria-live="polite" aria-label="{keys.length} of {PIN_LENGTH} keys">
    {#each Array.from({ length: PIN_LENGTH }, (_, i) => i) as i (i)}<span class:on={i < keys.length}></span>{/each}
  </div>
  {#if wrong}<p class="dim" role="status">That isn't the PIN.</p>{/if}
  <button class="ghost forgot" onclick={lock} title="Lock fully: the encryption secret or a passkey opens it again"
    >Forgot the PIN?</button
  >
</dialog>

<style>
  .pin {
    border: 0;
    background: transparent;
    color: var(--fg, inherit);
    text-align: center;
    padding: 24px;
    outline: none;
  }
  .pin::backdrop {
    background: rgb(0 0 0 / 0.35);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
  }
  .title {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
  .dots {
    display: flex;
    justify-content: center;
    gap: 14px;
    margin: 18px 0 10px;
  }
  .dots span {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid currentColor;
  }
  .dots span.on {
    background: currentColor;
  }
  .wrong {
    animation: shake 0.3s;
  }
  @keyframes shake {
    25% {
      transform: translateX(-8px);
    }
    75% {
      transform: translateX(8px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .wrong {
      animation: none;
    }
  }
  .forgot {
    margin-top: 12px;
    font-size: 13px;
  }
</style>

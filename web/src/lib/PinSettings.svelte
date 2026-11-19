<script lang="ts">
  // The PIN of a trusted device, in Settings → General → This device (spec 0022): set by pressing four keys twice (no
  // button: it is saved as the second four match), changed, removed, and after how many idle minutes it comes up.
  import {
    clearPin,
    DEFAULT_PIN_MINUTES,
    hasPin,
    PIN_LENGTH,
    pinKey,
    pinMinutes,
    setPin,
    setPinMinutes,
  } from '../engine/pin';
  import { pinLock } from '../engine/session';

  let have = $state(hasPin());
  let minutes = $state(pinMinutes());
  /** Setting one: the first four, then the second. */
  let setting = $state(false);
  let first = $state<string[] | null>(null);
  let keys = $state<string[]>([]);
  let said = $state('');
  let box: HTMLElement | undefined = $state();

  function start() {
    setting = true;
    first = null;
    keys = [];
    said = '';
    queueMicrotask(() => box?.focus());
  }

  async function press(e: KeyboardEvent) {
    if (e.key === 'Tab') return;
    if (e.key === 'Escape') {
      setting = false;
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Backspace') {
      keys = keys.slice(0, -1);
      return;
    }
    const k = pinKey(e);
    if (!k) return;
    keys = [...keys, k];
    if (keys.length < PIN_LENGTH) return;
    if (!first) {
      first = keys;
      keys = [];
      return;
    }
    if (first.join(' ') !== keys.join(' ')) {
      said = 'The two differ. Press four keys again.';
      first = null;
      keys = [];
      return;
    }
    await setPin(keys);
    have = true;
    setting = false;
    said = `Set. The PIN screen comes up after ${minutes || 'no'} idle minute${minutes === 1 ? '' : 's'}.`;
  }
</script>

<div class="pin">
  <h4>PIN</h4>
  <p class="dim">
    A screen over the app after some idle minutes, gone with four keys: the arrows, letters and digits. It keeps eyes
    off your tasks; it doesn't protect them (anyone who can use this browser gets past it). Only this browser knows of
    it.
  </p>
  {#if setting}
    <div
      class="capture"
      bind:this={box}
      tabindex="0"
      role="textbox"
      aria-label={first ? 'Press the same four keys again' : 'Press four keys for the PIN'}
      onkeydown={press}
    >
      <span>{first ? 'Again, to confirm' : 'Press four keys'}</span>
      <span class="dots"
        >{#each Array.from({ length: PIN_LENGTH }, (_, i) => i) as i (i)}<span class:on={i < keys.length}
          ></span>{/each}</span
      >
    </div>
    <div class="row"><button class="ghost" onclick={() => (setting = false)}>Cancel</button></div>
  {:else}
    <div class="row">
      <button onclick={start}>{have ? 'Change the PIN' : 'Set a PIN'}</button>
      {#if have}
        <button onclick={pinLock}>Lock with PIN now</button>
        <button
          class="ghost"
          onclick={() => {
            clearPin();
            have = false;
            said = 'No PIN now.';
          }}>Remove PIN</button
        >
      {/if}
    </div>
  {/if}
  {#if have}
    <label class="field"
      ><span>PIN screen after this many idle minutes (0: only with Lock with PIN now or <code>:lock</code>)</span>
      <input
        type="number"
        min="0"
        step="1"
        bind:value={minutes}
        onchange={() => setPinMinutes(minutes)}
        placeholder={String(DEFAULT_PIN_MINUTES)}
      />
    </label>
  {/if}
  {#if said}<p class="dim" role="status">{said}</p>{/if}
</div>

<style>
  .pin {
    display: grid;
    gap: 8px;
  }
  h4 {
    margin: 6px 0 0;
    font-size: 14px;
  }
  p {
    margin: 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .capture {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 14px;
    border: 1px solid var(--accent);
    border-radius: 8px;
    max-width: 360px;
  }
  .dots {
    display: inline-flex;
    gap: 10px;
  }
  .dots span {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid currentColor;
  }
  .dots span.on {
    background: currentColor;
  }
  .field {
    display: grid;
    gap: 4px;
    max-width: 420px;
  }
  .field input {
    width: 90px;
  }
  code {
    font-family: var(--mono);
  }
</style>

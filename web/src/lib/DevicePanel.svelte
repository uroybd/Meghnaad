<script lang="ts">
  // The static flavour's device settings (in General): where it syncs, trusting it (spec 0018), a passkey to unlock it
  // (spec 0019), locking, and forgetting it. All of it is this browser's alone: nothing here is synced.
  import { onMount } from 'svelte';
  import { PasskeyCancelled } from '../engine/passkey';
  import {
    connectedTo,
    DEFAULT_IDLE_MINUTES,
    forget,
    idleMinutes,
    isTrusted,
    lock,
    passkeyKind,
    passkeyPossible,
    removePasskey,
    setIdleMinutes,
    setTrusted,
    setUpPasskey,
  } from '../engine/session';

  const where = connectedTo();
  let minutes = $state(idleMinutes());
  let trusted = $state(false);
  let passkey = $state<'prf' | 'signature' | undefined>();
  let possible = $state(false);
  let busy = $state(false);
  let said = $state('');

  async function refresh() {
    trusted = await isTrusted();
    passkey = await passkeyKind();
    possible = await passkeyPossible();
  }

  onMount(() => void refresh());

  async function doing(what: () => Promise<void>, done: string) {
    busy = true;
    said = '';
    try {
      await what();
      said = done;
    } catch (e) {
      if (!(e instanceof PasskeyCancelled)) said = e instanceof Error ? e.message : String(e);
    } finally {
      await refresh();
      busy = false;
    }
  }

  function toggleTrust(e: Event) {
    const on = (e.currentTarget as HTMLInputElement).checked;
    void doing(
      () => setTrusted(on),
      on
        ? 'This device stays unlocked now.'
        : passkey === 'signature'
          ? 'This device asks for the password again; its passkey was removed with the trust.'
          : 'This device asks for the password again.',
    );
  }

  async function forgetIt() {
    if (
      !confirm(
        'Forget this device? Its sealed token and its copy of the tasks are removed, and every tab locks. What was synced stays on the branch.',
      )
    )
      return;
    await forget();
  }
</script>

<div class="device">
  <h3>This device</h3>
  {#if where}
    <p class="dim">
      Syncs with <code>{where.repo}</code>, branch <code>{where.branch || 'taskchampion'}</code>{#if where.api}, at
        <code>{where.api}</code>{/if}. The token is kept here sealed with your encryption secret; the secret is never
      kept.
    </p>
  {/if}
  <label class="check"
    ><input type="checkbox" checked={trusted} disabled={busy} onchange={toggleTrust} /> Trust this device: stay unlocked
    here, without the password
    <span class="dim">Anyone who can use this browser can open your tasks. Only this browser knows of it.</span></label
  >
  <label class="field"
    >Lock after this many idle minutes (0: never)
    <input
      type="number"
      min="0"
      step="5"
      bind:value={minutes}
      onchange={() => setIdleMinutes(minutes)}
      placeholder={String(DEFAULT_IDLE_MINUTES)}
      disabled={trusted}
    />
    {#if trusted}<span>Off while this device is trusted.</span>{/if}</label
  >
  {#if passkey}
    <p class="dim">
      {passkey === 'prf'
        ? 'A passkey unlocks this device.'
        : 'A passkey unlocks this device after Lock now (checked by its signature, while the device is trusted).'}
      The passkey itself stays in your password manager: delete it there if you no longer want it.
    </p>
    <div class="row">
      <button disabled={busy} onclick={() => doing(removePasskey, 'The passkey no longer unlocks this device.')}
        >Stop unlocking with a passkey</button
      >
    </div>
  {:else if possible}
    <div class="row">
      <button
        disabled={busy}
        onclick={() => doing(setUpPasskey, 'Done: the unlock form offers your passkey from now on.')}
        >Unlock with a passkey on this device</button
      >
    </div>
  {/if}
  {#if said}<p class="dim" role="status">{said}</p>{/if}
  <div class="row">
    <button onclick={lock}>Lock now</button>
    <button class="danger" onclick={forgetIt}>Forget this device</button>
  </div>
</div>

<style>
  .device {
    margin-top: 24px;
    display: grid;
    gap: 10px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 5px;
    overflow-wrap: anywhere;
  }
  .check {
    display: block;
    font-size: 13px;
  }
  .check .dim {
    display: block;
    margin-left: 22px;
  }
  .field {
    display: grid;
    gap: 4px;
    font-size: 13px;
    color: var(--dim);
  }
  .field input {
    width: 8em;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>

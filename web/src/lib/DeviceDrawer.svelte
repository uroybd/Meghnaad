<script lang="ts">
  import { ask } from './ask.svelte';
  import { deviceAsk } from './deviceAsk.svelte';
  import { deviceIdProblem } from './deviceSettings';
  import Drawer from './Drawer.svelte';
  import { Trash2 } from './icons';
  import { store } from './store.svelte';

  // Naming this device on the first save of its own settings, or choosing which device's settings to use (spec 0027).
  const now = $derived(deviceAsk.now);
  let id = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);
  $effect(() => {
    if (now?.mode === 'name') id = now.id;
  });
  const problem = $derived(deviceIdProblem(id.trim()));

  function close() {
    if (deviceAsk.now?.mode === 'name') deviceAsk.now.resolve(null);
    deviceAsk.now = null;
    error = null;
  }

  function save(e: SubmitEvent) {
    e.preventDefault();
    if (problem || deviceAsk.now?.mode !== 'name') return;
    deviceAsk.now.resolve(id.trim());
    deviceAsk.now = null;
  }

  async function use(chosen: string) {
    busy = true;
    error = null;
    try {
      await store.useDevice(chosen);
      deviceAsk.now = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function remove(gone: string) {
    if (!(await ask(`Delete the settings of the device ${gone}? Any device using them goes back to everyone's.`)))
      return;
    busy = true;
    error = null;
    try {
      await store.deleteDevice(gone);
      if (deviceAsk.now?.mode === 'choose') {
        const ids = deviceAsk.now.ids.filter((x) => x !== gone);
        deviceAsk.now = ids.length ? { mode: 'choose', ids } : null;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if now?.mode === 'name'}
  <Drawer title="Name this device" onclose={close}>
    <form onsubmit={save}>
      <p class="dim">
        This device's own settings are kept under its ID, beside everyone's <code>settings.json</code>. Keep this one or
        type your own; another device given the same ID shares them.
      </p>
      <label class="field">
        <span>Device ID</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={id} autofocus spellcheck="false" autocomplete="off" autocapitalize="off" />
      </label>
      {#if problem && id.trim()}<p class="err" role="alert">{problem}</p>{/if}
      <div class="row">
        <button class="primary" type="submit" disabled={!!problem}>Save</button>
        <button type="button" class="ghost" onclick={close}>Cancel</button>
      </div>
    </form>
  </Drawer>
{:else if now?.mode === 'choose'}
  <Drawer title="This device's settings" onclose={close}>
    <p class="dim">Other devices have settings of their own. Use one of them here, or make new ones for this device.</p>
    <ul class="devices">
      {#each now.ids as d (d)}
        <li>
          <button class="pick" disabled={busy} onclick={() => use(d)}>Use <code>{d}</code></button>
          <button class="icon" disabled={busy} title="Delete {d}" aria-label="Delete {d}" onclick={() => remove(d)}
            ><Trash2 size={14} /></button
          >
        </li>
      {/each}
    </ul>
    {#if error}<p class="err" role="alert">{error}</p>{/if}
    <div class="row">
      <button class="primary" onclick={close}>New ones for this device</button>
    </div>
    <p class="dim">New ones are made, and named, when you first save something under This device.</p>
  </Drawer>
{/if}

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin: 10px 0;
    font-size: 12px;
    color: var(--dim);
  }
  .field input {
    font-size: 14px;
    font-family: var(--mono);
    color: var(--text);
  }
  .devices {
    list-style: none;
    margin: 12px 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .devices li {
    display: flex;
    gap: 6px;
  }
  .pick {
    flex: 1;
    text-align: left;
  }
</style>

<script lang="ts">
  import { STATIC } from '../engine/flavour';
  import DevicePanel from './DevicePanel.svelte';
  import GeneralPanel from './GeneralPanel.svelte';
  import KeysPanel from './KeysPanel.svelte';
  import { notifier } from './notifier.svelte';
  import { store } from './store.svelte';

  /**
   * Settings → This device (spec 0028): only what a device can have of its own, over everyone's `settings.json`: its ID,
   * its home page, its reminders and its keys, and its file as text. There is no other view of Settings per device.
   */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();
  let home = $state(false);
  let keys = $state(false);
  let agentsBusy = $state(false);
  let agentsError = $state<string | null>(null);
  async function toggleAgents(on: boolean) {
    agentsBusy = true;
    agentsError = null;
    try {
      await store.setAgents(on);
    } catch (e) {
      agentsError = e instanceof Error ? e.message : String(e);
    } finally {
      agentsBusy = false;
    }
  }
  $effect(() => {
    ondirty?.(home || keys);
  });

  const reminders = $derived(
    notifier.settings.enabled ? `On, ${notifier.settings.leadMinutes} minutes ahead` : 'Off on this device',
  );
</script>

<section aria-label="This device">
  <h2>This device</h2>
  <p class="dim">
    What this device has of its own: where it opens, its reminders and its keys. Everything else in Settings is the same
    on every device.
  </p>
  <GeneralPanel device ondirty={(v: boolean) => (home = v)}>
    <h3>Reminders</h3>
    <p class="dim">
      Always this device's own. {reminders}.
      <button class="small" onclick={() => (store.notifyOpen = true)}>Set reminders</button>
    </p>
    <h3>AI agents</h3>
    <label class="pick">
      <input
        type="checkbox"
        checked={store.agentsOn}
        disabled={agentsBusy}
        onchange={(e) => toggleAgents(e.currentTarget.checked)}
      />
      Let AI agents in this browser use the app's tools
    </label>
    <p class="dim">
      Off unless you turn it on, here and on each device. On, the app loads a WebMCP runtime so an agent in a tab where
      you are signed in can read your tasks and run commands, and a desktop agent can too through the MCP-B browser
      extension. Questions such as a delete still wait for you in the console.
    </p>
    {#if agentsError}<p class="err" role="alert">{agentsError}</p>{/if}
    <h3>Keys</h3>
    <KeysPanel device ondirty={(v: boolean) => (keys = v)} />
  </GeneralPanel>
  {#if STATIC}<DevicePanel />{/if}
</section>

<style>
  h3 {
    margin: 22px 0 6px;
    font-size: 15px;
  }
</style>

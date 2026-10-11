<script lang="ts">
  import { engine } from '../engine/client.svelte';
  import { plugin, type Connection } from '../engine/native';
  import ConnectForm from './ConnectForm.svelte';
  import { store } from './store.svelte';

  let connection = $state<Connection | null>(null);
  let busy = $state(false);

  async function load() {
    connection = await plugin.getConnection();
  }
  void load();

  /** The tasks of the bucket are in: the page reads them again. */
  async function connected() {
    await load();
    store.notify('Connected. Your tasks are syncing.', 'ok');
    await engine.sync();
  }

  async function syncNow() {
    busy = true;
    try {
      await engine.sync(true);
      store.notify('Synced.', 'ok');
    } catch {
      // The banner under the header says why.
    } finally {
      busy = false;
    }
  }

  async function disconnect() {
    if (!confirm('Stop syncing? The tasks stay on this phone, and the bucket keeps what it has.')) return;
    await plugin.clearConnection();
    await load();
    store.notify('Not syncing any more. Your tasks are on this phone only.', 'ok');
  }
</script>

<section>
  {#if connection === null}
    <p class="dim">Looking…</p>
  {:else if connection.configured}
    <p>
      Syncing with <code>{connection.bucket}</code> at <code>{connection.endpoint}</code> (access key
      <code>{connection.accessKey}</code>).
    </p>
    {#if engine.lastSync}<p class="dim">Last synced {new Date(engine.lastSync).toLocaleTimeString()}.</p>{/if}
    <div class="row">
      <button class="primary" onclick={syncNow} disabled={busy}>{busy ? 'Syncing…' : 'Sync now'}</button>
      <button class="ghost" onclick={disconnect}>Stop syncing</button>
    </div>
  {:else}
    <p>
      Syncing is off: your tasks are on this phone only. Connect an S3 bucket (the one your <code>task</code> CLI syncs with)
      to have them everywhere. What is on the phone is added to what the bucket has.
    </p>
    <ConnectForm onconnected={connected} />
  {/if}
</section>

<style>
  section {
    display: grid;
    gap: 12px;
    max-width: 36rem;
  }
  p {
    margin: 0;
  }
  .dim {
    color: var(--dim);
  }
  .row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 5px;
    overflow-wrap: anywhere;
  }
</style>

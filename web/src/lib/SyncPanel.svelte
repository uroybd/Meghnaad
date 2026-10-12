<script lang="ts">
  import { engine } from '../engine/client.svelte';
  import { plugin, type Connection, type SyncOptions } from '../engine/native';
  import ConnectForm from './ConnectForm.svelte';
  import { store } from './store.svelte';

  let connection = $state<Connection | null>(null);
  let busy = $state(false);
  let options = $state<SyncOptions>({ settings: false, hooks: false });

  async function load() {
    connection = await plugin.getConnection();
    if (connection.configured) options = await plugin.getSyncOptions();
  }

  /** Keep the settings and/or the hooks in step with the bucket, from the next sync (now) on. */
  async function choose(next: SyncOptions) {
    options = next;
    await plugin.setSyncOptions(next);
    if (next.settings || next.hooks) await syncNow();
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
    <p>Syncing is on: this phone's tasks go to your S3 bucket, and what other devices add comes here.</p>
    {#if engine.lastSync}<p class="dim">Last synced {new Date(engine.lastSync).toLocaleTimeString()}.</p>{/if}
    <fieldset>
      <legend>Keep in step with the bucket</legend>
      <label>
        <input
          type="checkbox"
          checked={options.settings}
          onchange={(e) => choose({ ...options, settings: e.currentTarget.checked })}
        />
        Settings: the taskrc subset, reports, commands and the rest
      </label>
      <label>
        <input
          type="checkbox"
          checked={options.hooks}
          onchange={(e) => choose({ ...options, hooks: e.currentTarget.checked })}
        />
        Hooks: the Lua script
      </label>
      <small>
        These are the files the web app keeps in the bucket, so an edit on either side reaches the other at the next
        sync; when both changed, the newer one wins and the other is kept to restore. The secrets for hooks are never
        synced.
      </small>
    </fieldset>
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
  fieldset {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 14px;
    display: grid;
    gap: 8px;
  }
  fieldset label {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    min-height: 32px;
  }
  fieldset small {
    color: var(--dim);
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 5px;
    overflow-wrap: anywhere;
  }
</style>

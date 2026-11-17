<script lang="ts">
  // The static flavour's device settings (in General): where it syncs, locking, and forgetting it.
  import { connectedTo, DEFAULT_IDLE_MINUTES, forget, idleMinutes, lock, setIdleMinutes } from '../engine/session';

  const where = connectedTo();
  let minutes = $state(idleMinutes());

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
  <label class="field"
    >Lock after this many idle minutes (0: never)
    <input
      type="number"
      min="0"
      step="5"
      bind:value={minutes}
      onchange={() => setIdleMinutes(minutes)}
      placeholder={String(DEFAULT_IDLE_MINUTES)}
    /></label
  >
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

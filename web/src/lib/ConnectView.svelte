<script lang="ts">
  import { plugin } from '../engine/native';
  import ConnectForm from './ConnectForm.svelte';

  let { onready }: { onready: () => void } = $props();

  let failed = $state('');

  /** Go on without a bucket: the tasks stay on this phone, and a connection can be made later in Settings. */
  async function skip() {
    failed = '';
    try {
      await plugin.useLocally();
      onready();
    } catch (err) {
      failed = err instanceof Error ? err.message : String(err);
    }
  }
</script>

<main class="guide">
  <img src="/logo-64.png" width="48" height="48" alt="" />
  <h1>Your tasks</h1>
  <p class="lead">
    Meghnaad keeps your tasks on this phone. To have them on your other devices too, connect the S3 bucket your
    <code>task</code> CLI syncs with (Cloudflare R2, AWS S3, MinIO, anything that speaks S3). The keys stay on this phone,
    in its secure storage.
  </p>

  <ConnectForm onconnected={onready} />

  <div class="skip">
    <p>Or start without syncing: everything works, and you can connect later in <strong>Settings → Sync</strong>.</p>
    {#if failed}<p class="problem" role="alert">{failed}</p>{/if}
    <button class="ghost" onclick={skip}>Use on this phone only</button>
  </div>
</main>

<style>
  .guide {
    max-width: 40rem;
    margin: 0 auto;
    padding: max(32px, env(safe-area-inset-top)) 20px 48px;
    min-height: 100%;
    overflow: auto;
  }
  img {
    border-radius: 10px;
    display: block;
  }
  h1 {
    margin: 12px 0 4px;
    font-size: 24px;
  }
  .lead {
    color: var(--dim);
    margin: 0 0 18px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 5px;
    overflow-wrap: anywhere;
  }
  .skip {
    margin-top: 28px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
    display: grid;
    gap: 10px;
  }
  .skip p {
    margin: 0;
    color: var(--dim);
  }
  .skip .problem {
    color: var(--err);
  }
  .skip button {
    min-height: 44px;
  }
</style>

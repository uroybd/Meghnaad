<script lang="ts">
  // The static flavour's front door: Connect (the first visit on a device) or Unlock (every later one). Nothing of the
  // app is loaded until it is passed. See src/engine/session.ts and vault.ts.
  import { onMount } from 'svelte';
  import { WrongPassword } from '../engine/pgp';
  import { askOtherTabs, connect, forget, unlockWithSecret } from '../engine/session';
  import { sealedAccess } from '../engine/vault';
  import { TriangleAlert } from './icons';

  let { onunlocked, defaults }: { onunlocked: () => void; defaults: { repo?: string; branch?: string; api?: string } } =
    $props();

  let mode = $state<'checking' | 'connect' | 'unlock'>('checking');
  let busy = $state(false);
  let error = $state('');
  let repo = $state('');
  let branch = $state('');
  let api = $state('');
  let token = $state('');
  let secret = $state('');

  onMount(async () => {
    repo = defaults.repo ?? '';
    branch = defaults.branch ?? '';
    api = defaults.api ?? '';
    if (!(await sealedAccess())) {
      mode = 'connect';
      return;
    }
    if (await askOtherTabs()) onunlocked();
    else mode = 'unlock';
  });

  async function doConnect(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await connect({ api, repo, branch, token, secret });
      token = '';
      secret = '';
      onunlocked();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function doUnlock(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await unlockWithSecret(secret);
      secret = '';
      onunlocked();
    } catch (err) {
      error = err instanceof WrongPassword ? err.message : `This device could not be unlocked: ${String(err)}`;
    } finally {
      busy = false;
    }
  }

  async function doForget() {
    if (
      !confirm(
        'Forget this device? Its sealed token and its copy of the tasks are removed; what was synced stays on the branch.',
      )
    )
      return;
    await forget();
  }
</script>

<main class="gate">
  <img src="logo-64.png" width="48" height="48" alt="" />
  {#if mode === 'connect'}
    <h1>Connect this device</h1>
    <p class="lead">
      Your tasks are on a branch of a git repository, the one your <code>task</code> CLI syncs to with
      <code>sync.git.*</code>. This page reaches it directly; no server of ours is involved.
    </p>
    <form onsubmit={doConnect}>
      <label>Repository <input bind:value={repo} placeholder="owner/name" required autocomplete="off" /></label>
      <label>Branch <input bind:value={branch} placeholder="taskchampion" autocomplete="off" /></label>
      <label
        >API address <input bind:value={api} placeholder="https://api.github.com" autocomplete="off" />
        <span class="dim">GitHub's if empty; for Gitea or Forgejo, <code>https://&lt;host&gt;/api/v1</code></span>
      </label>
      <label
        >Token <input type="password" bind:value={token} required autocomplete="off" />
        <span class="dim">For this one repository: Contents, read and write.</span>
      </label>
      <label
        >Encryption secret <input type="password" bind:value={secret} required autocomplete="current-password" />
        <span class="dim"
          >Your <code>sync.encryption_secret</code>. Typed on each visit; the token is kept sealed with it.</span
        >
      </label>
      <button class="primary" disabled={busy}>{busy ? 'Connecting…' : 'Connect'}</button>
    </form>
  {:else if mode === 'unlock'}
    <h1>Unlock</h1>
    <p class="lead">Type the encryption secret to open your tasks on this device.</p>
    <form onsubmit={doUnlock}>
      <label
        >Encryption secret <input type="password" bind:value={secret} required autocomplete="current-password" /></label
      >
      <div class="row">
        <button class="primary" disabled={busy}>{busy ? 'Unlocking…' : 'Unlock'}</button>
        <button type="button" class="ghost" onclick={doForget} disabled={busy}>Forget this device</button>
      </div>
    </form>
  {/if}
  {#if error}
    <p class="warn" role="alert"><TriangleAlert size={14} /> {error}</p>
  {/if}
</main>

<style>
  .gate {
    max-width: 30rem;
    margin: 0 auto;
    padding: max(32px, env(safe-area-inset-top)) 16px 48px;
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
  form {
    display: grid;
    gap: 12px;
  }
  label {
    display: grid;
    gap: 4px;
    font-weight: 600;
  }
  label .dim {
    font-weight: 400;
    font-size: 13px;
  }
  input {
    width: 100%;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 5px;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .warn {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--warn);
    margin-top: 14px;
  }
</style>

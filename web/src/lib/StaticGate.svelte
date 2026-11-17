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

  /**
   * Password managers keep the pair as a login: the token as the username, the encryption secret as the password, so
   * a new device fills both. The fields say so with `autocomplete`; where the browser has the Credential Management
   * API (Chrome, Edge) it is also asked to offer saving them, since this page never navigates after a form.
   */
  async function offerToSave(): Promise<void> {
    const make = (globalThis as { PasswordCredential?: new (data: { id: string; password: string }) => Credential })
      .PasswordCredential;
    if (!make || !navigator.credentials) return;
    await navigator.credentials.store(new make({ id: token, password: secret })).catch(() => undefined);
  }

  async function doConnect(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await connect({ api, repo, branch, token, secret });
      await offerToSave();
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
        >Token <input
          class="masked"
          name="username"
          bind:value={token}
          required
          autocomplete="username"
          autocapitalize="off"
          spellcheck="false"
        />
        <span class="dim"
          >For this one repository: Contents, read and write. A password manager keeps it as the username.</span
        >
      </label>
      <label
        >Encryption secret <input
          type="password"
          name="password"
          bind:value={secret}
          required
          autocomplete="current-password"
        />
        <span class="dim"
          >Your <code>sync.encryption_secret</code>, kept as the password. Typed on each visit; the token is kept sealed
          with it.</span
        >
      </label>
      <button class="primary" disabled={busy}>{busy ? 'Connecting…' : 'Connect'}</button>
    </form>
  {:else if mode === 'unlock'}
    <h1>Unlock</h1>
    <p class="lead">Type the encryption secret to open your tasks on this device.</p>
    <form onsubmit={doUnlock}>
      <!-- The saved login's username (the token), so a password manager offers it here; only the secret is used. -->
      <input class="for-password-manager" name="username" autocomplete="username" tabindex="-1" aria-hidden="true" />
      <label
        >Encryption secret <input
          type="password"
          name="password"
          bind:value={secret}
          required
          autocomplete="current-password"
        /></label
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
  /* Shown as dots, but a text field, so password managers take it for the username. */
  .masked {
    -webkit-text-security: disc;
  }
  .for-password-manager {
    display: none;
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

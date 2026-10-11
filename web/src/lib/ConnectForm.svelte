<script lang="ts">
  import { plugin } from '../engine/native';

  /** Called once the bucket has been tried with a first sync and works. */
  let { onconnected }: { onconnected: () => void } = $props();

  let endpoint = $state('');
  let bucket = $state('');
  let region = $state('auto');
  let accessKey = $state('');
  let secretKey = $state('');
  let encryptionSecret = $state('');
  let working = $state(false);
  let problem = $state('');

  const complete = $derived(
    [endpoint, bucket, accessKey, secretKey, encryptionSecret].every((v) => v.trim() !== '') && !working,
  );

  /** Keep the connection, and try it: a first sync fetches the tasks, and a wrong key or secret says so here. */
  async function connect(e: SubmitEvent) {
    e.preventDefault();
    working = true;
    problem = '';
    try {
      await plugin.setConnection({ endpoint, bucket, region, accessKey, secretKey, encryptionSecret });
      await plugin.send({ op: 'sync' });
      secretKey = encryptionSecret = '';
      onconnected();
    } catch (err) {
      await plugin.clearConnection().catch(() => {});
      problem = (err instanceof Error ? err.message : String(err)).replace(/^Server Error: /, '');
    } finally {
      working = false;
    }
  }
</script>

<form onsubmit={connect}>
  <label>
    S3 endpoint
    <input
      bind:value={endpoint}
      type="url"
      inputmode="url"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      placeholder="https://<account>.r2.cloudflarestorage.com"
    />
  </label>
  <label>
    Bucket
    <input bind:value={bucket} autocomplete="off" autocapitalize="off" spellcheck="false" />
  </label>
  <label>
    Region
    <input bind:value={region} autocomplete="off" autocapitalize="off" spellcheck="false" />
    <small>"auto" for R2; the bucket's region (such as eu-west-1) for AWS.</small>
  </label>
  <label>
    Access key ID
    <input bind:value={accessKey} autocomplete="off" autocapitalize="off" spellcheck="false" />
    <small>The client ID of the key: "Access Key ID" in R2 and AWS, "keyID" in Backblaze.</small>
  </label>
  <label>
    Secret access key
    <input bind:value={secretKey} type="password" autocomplete="off" />
    <small>The client secret: shown once when the key is made.</small>
  </label>
  <label>
    Encryption secret
    <input bind:value={encryptionSecret} type="password" autocomplete="off" />
    <small>The same value as <code>sync.encryption_secret</code> in your taskrc.</small>
  </label>

  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {/if}
  <button class="primary" type="submit" disabled={!complete}>{working ? 'Connecting…' : 'Connect'}</button>
</form>

<style>
  form {
    display: grid;
    gap: 14px;
  }
  label {
    display: grid;
    gap: 4px;
    font-size: 14px;
  }
  small {
    color: var(--dim);
  }
  input {
    font: inherit;
    padding: 10px 12px;
    min-height: 44px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 5px;
    overflow-wrap: anywhere;
  }
  .problem {
    color: var(--err);
    margin: 0;
    overflow-wrap: anywhere;
  }
  button {
    min-height: 44px;
  }
</style>

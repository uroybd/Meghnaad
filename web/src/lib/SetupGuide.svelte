<script lang="ts">
  import { getSetup, type SetupStatus } from './api';
  import { Check, Copy, TriangleAlert } from './icons';

  let { status, onready }: { status: SetupStatus; onready: () => void } = $props();

  let checking = $state(false);
  let copied = $state(false);
  let still = $state(false);

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* the text is selectable on the page, so nothing is lost */
    }
  }

  async function recheck() {
    checking = true;
    still = false;
    const s = await getSetup();
    checking = false;
    if (!s || s.configured) onready();
    else still = true;
  }
</script>

<main class="guide">
  <img src="/logo-64.png" width="48" height="48" alt="" />
  <h1>Almost there</h1>
  <p class="lead">
    Meghnaad is deployed, but it doesn't know yet who is allowed in, so it refuses every request. That is on purpose: until
    Cloudflare Access is connected, your tasks stay private. Two short steps finish it.
  </p>

  <ol>
    <li>
      <h2>Protect this address with Cloudflare Access</h2>
      <p>
        In the Cloudflare dashboard open <strong>Access controls &rarr; Applications &rarr; Add an application &rarr;
        Self-hosted</strong>.
      </p>
      <ul>
        <li>
          Destination: <code>{status.host}</code>
          <button class="ghost" onclick={() => copy(status.host)} aria-label="Copy the hostname">
            {#if copied}<Check size={14} />{:else}<Copy size={14} />{/if}
          </button>
        </li>
        <li>Policy: <em>Allow</em>, selector <em>Emails</em>, your own email address.</li>
        <li>Login method: <em>One-time PIN</em> works without any setup.</li>
      </ul>
    </li>
    <li>
      <h2>Tell the Worker about it</h2>
      <p>
        Open this Worker in the dashboard: <strong>Settings &rarr; Variables and Secrets</strong>, and add these as
        <em>Text</em> variables, then deploy:
      </p>
      <table>
        <tbody>
          <tr class:todo={status.missing.includes('TEAM_DOMAIN')}>
            <th scope="row"><code>TEAM_DOMAIN</code></th>
            <td>
              <code>https://&lt;your-team&gt;.cloudflareaccess.com</code>
              <span class="dim">(Access controls &rarr; Settings)</span>
            </td>
            <td>{#if status.missing.includes('TEAM_DOMAIN')}<span class="need">needed</span>{:else}<Check size={14} />{/if}</td>
          </tr>
          <tr class:todo={status.missing.includes('POLICY_AUD')}>
            <th scope="row"><code>POLICY_AUD</code></th>
            <td>
              the <em>Application Audience (AUD) tag</em> <span class="dim">(on the application's page)</span>
            </td>
            <td>{#if status.missing.includes('POLICY_AUD')}<span class="need">needed</span>{:else}<Check size={14} />{/if}</td>
          </tr>
        </tbody>
      </table>
      <p class="dim">
        From a terminal, <code>npm run setup</code> does this for you. The sync secret
        (<code>TC_ENCRYPTION_SECRET</code>) is separate and should already be set.
      </p>
    </li>
  </ol>

  <div class="row">
    <button class="primary" onclick={recheck} disabled={checking}>{checking ? 'Checking…' : 'I did it, check again'}</button>
    {#if still}
      <span class="warn"><TriangleAlert size={14} /> Still missing: {status.missing.join(', ') || 'unknown'}. A deploy may take a moment.</span>
    {/if}
  </div>
</main>

<style>
  .guide { max-width: 40rem; margin: 0 auto; padding: max(32px, env(safe-area-inset-top)) 20px 48px; min-height: 100%; overflow: auto; }
  img { border-radius: 10px; display: block; }
  h1 { margin: 12px 0 4px; font-size: 24px; }
  h2 { margin: 0 0 4px; font-size: 16px; }
  .lead { color: var(--dim); margin: 0 0 18px; }
  ol { padding-left: 1.2em; display: grid; gap: 18px; margin: 0 0 20px; }
  ol > li { padding-left: 4px; }
  ul { margin: 6px 0; padding-left: 1.2em; }
  p { margin: 4px 0; }
  code { font-family: var(--mono); background: var(--panel-2); border-radius: 4px; padding: 0 5px; overflow-wrap: anywhere; }
  table { border-collapse: collapse; margin: 8px 0; width: 100%; }
  th, td { text-align: left; padding: 6px 10px 6px 0; vertical-align: top; font-weight: 400; }
  tr { border-top: 1px solid var(--line); }
  tr.todo th code { background: color-mix(in srgb, var(--warn) 22%, var(--panel-2)); }
  .need { color: var(--warn); font-weight: 600; }
  .warn { display: inline-flex; align-items: center; gap: 5px; color: var(--warn); }
  .row { flex-wrap: wrap; }
  button.ghost { line-height: 0; vertical-align: middle; }
  th code { white-space: nowrap; }
  @media (max-width: 600px) {
    tr { display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; padding: 8px 0; }
    th { grid-column: 1; }
    td:nth-child(2) { grid-column: 1 / -1; grid-row: 2; }
    td:nth-child(3) { grid-column: 2; grid-row: 1; text-align: right; }
    th, td { padding: 0; }
  }
</style>

<script lang="ts">
  import { keyLabel } from './keymap';
  import { store } from './store.svelte';

  // Every key of the web app (`app.help`, `?` by default), with what it does from the keymap asset's `about`.
  let dialog: HTMLDialogElement;
  $effect(() => {
    dialog.showModal();
  });

  const close = () => (store.keysHelpOpen = false);
  const about = (action: string) => {
    const a = store.keymapDefaults?.about;
    const line = a && typeof a === 'object' ? (a as Record<string, unknown>)[action] : undefined;
    return typeof line === 'string' ? line : action;
  };
  const rows = $derived(Object.entries(store.keymap.keys));

  function settings() {
    dialog.close();
    store.openSettings('keys');
  }
</script>

<dialog bind:this={dialog} onclose={close} aria-label="Keys">
  <h3>Keys</h3>
  <table>
    <tbody>
      {#each rows as [action, keys] (action)}
        <tr>
          <td class="k">
            {#each keys as k, i (i)}<kbd>{keyLabel(k)}</kbd>{:else}<span class="dim">—</span>{/each}
          </td>
          <td>{about(action)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
  <p class="dim">Away from a text field. <button class="link" onclick={settings}>Change them in Settings</button></p>
  <div class="row"><button class="primary" onclick={() => dialog.close()}>Close</button></div>
</dialog>

<style>
  dialog {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    padding: 20px 24px;
    width: min(520px, 96vw);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  h3 {
    margin: 0 0 10px;
  }
  table {
    border-collapse: collapse;
    font-size: 13px;
  }
  td {
    padding: 3px 12px 3px 0;
    vertical-align: top;
  }
  .k {
    white-space: nowrap;
  }
  kbd {
    display: inline-block;
    font: 12px var(--mono, monospace);
    padding: 1px 6px;
    margin-right: 3px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--panel-2);
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: inherit;
    cursor: pointer;
  }
</style>

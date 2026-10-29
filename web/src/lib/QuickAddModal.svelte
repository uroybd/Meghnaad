<script lang="ts">
  import AddPreviewCard from './AddPreviewCard.svelte';
  import { Plus, SlidersHorizontal } from './icons';
  import { addArgs, previewAdd, seedOf } from './addPreview';
  import { store, type Entry } from './store.svelte';

  // One line for the common case, typed as in the console: the card below it is the task the line will make. "More fields"
  // opens the full form with that task already broken into its fields.
  let {
    line: initial = '',
    from = null,
    onclose,
  }: {
    line?: string;
    from?: Entry | null;
    onclose: () => void;
  } = $props();

  /* svelte-ignore state_referenced_locally */
  let line = $state(initial);
  let busy = $state(false);
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;

  const text = $derived(line.trim());
  const preview = $derived(text ? previewAdd(text, store.config?.config) : null);
  // The engine refuses a line with any part wrong, so adding waits until the card has none to report.
  const ready = $derived(!!preview?.description && preview.errors.length === 0);

  async function add(e: Event) {
    e.preventDefault();
    if (!ready || busy) return;
    busy = true;
    const mine = text;
    const { value: res, released } = await store.waitOrRelease(store.act(from, addArgs(mine)), onclose, 'Creating…');
    busy = false;
    // A slow add let the form go; if the engine then refuses it, the line comes back to be put right.
    if (released) {
      if (!res || res.result.kind === 'error') store.quickAdd(mine);
    } else if (res && res.result.kind !== 'error') onclose();
  }

  function more() {
    store.addWithForm(seedOf(preview, text, store.config?.config.udas));
  }

  $effect(() => {
    dialog.showModal();
    input.focus();
  });
</script>

<dialog bind:this={dialog} {onclose} aria-label="New task">
  <form onsubmit={add}>
    <h3>New task</h3>
    <input
      bind:this={input}
      bind:value={line}
      aria-label="Task"
      placeholder="Buy milk due:tomorrow +home"
      autocomplete="off"
      autocapitalize="sentences"
    />
    <p class="hint dim">
      Words are the description; <code>project:</code> <code>due:</code> <code>priority:</code> <code>+tag</code> set the
      rest.
    </p>

    <AddPreviewCard {preview} />
    <div class="problems" role="status">
      {#if preview}{#each preview.errors as e, i (i)}<div class="err">{e}</div>{/each}{/if}
    </div>

    <footer class="row">
      <button type="button" class="ghost more" onclick={more}><SlidersHorizontal size={14} /> More fields</button>
      <span class="grow"></span>
      <button type="button" onclick={() => dialog.close()}>Cancel</button>
      <button type="submit" class="primary" disabled={busy || !ready}><Plus size={14} /> Add task</button>
    </footer>
  </form>
</dialog>

<style>
  dialog {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    padding: 20px 22px;
    width: min(560px, 96vw);
    max-height: 94vh;
    overflow: auto;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  form {
    display: grid;
    gap: 10px;
  }
  h3 {
    margin: 0;
  }
  input {
    width: 100%;
    font-size: 16px;
    padding: 9px 10px;
  }
  .hint {
    margin: 0;
    font-size: 12px;
  }
  .problems {
    min-height: 0;
    font-size: 12px;
  }
  .more {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--dim);
  }
  footer button.primary {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  /* Phones: the box sits at the top, clear of the keyboard, and the buttons fill the width. */
  @media (max-width: 760px) {
    dialog {
      margin: max(8px, env(safe-area-inset-top)) auto auto;
      padding: 16px;
    }
    footer {
      flex-wrap: wrap;
    }
    footer .grow {
      display: none;
    }
    footer button {
      flex: 1 1 auto;
      white-space: nowrap;
    }
  }
</style>

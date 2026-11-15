<script lang="ts">
  import AddPreviewCard from './AddPreviewCard.svelte';
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
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

  // Tab completes as the console does for `add` (projects, tags, attributes, dates): the line is completed with `add `
  // in front, so the first word is read as part of the task and not as a command. Tab stays in the box; Shift-Tab
  // goes back through the options, or leaves the box when there are none to go through.
  const PREFIX = 'add ';
  const completer = new Completer();
  const caret = () => PREFIX.length + (input.selectionStart ?? line.length);
  function take(r: { line: string; caret: number } | null) {
    if (!r) return;
    line = r.line.slice(PREFIX.length);
    const at = Math.max(0, r.caret - PREFIX.length);
    queueMicrotask(() => input.setSelectionRange(at, at));
  }
  function onkeydown(e: KeyboardEvent) {
    const open = !!completer.menu;
    if (e.key === 'Tab' && (!e.shiftKey || open)) {
      e.preventDefault();
      take(completer.tab(PREFIX + line, caret(), store.vocab, e.shiftKey));
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && open) {
      e.preventDefault();
      take(completer.move(e.key === 'ArrowDown' ? 1 : -1));
    } else if ((e.key === 'Enter' || e.key === 'Escape') && open) {
      // The menu first: Enter keeps the option, Escape the line; a second one adds, or closes the form.
      e.preventDefault();
      completer.close();
    }
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
    <div class="field">
      <input
        bind:this={input}
        bind:value={line}
        {onkeydown}
        oninput={() => completer.suggest(PREFIX + line, caret(), store.vocab)}
        onblur={() => completer.close()}
        aria-label="Task"
        placeholder="Buy milk due:tomorrow +home  (Tab completes)"
        autocomplete="off"
        autocapitalize="sentences"
      />
      <CompletionMenu {completer} placement="below" onpick={(i) => take(completer.pick(i))} />
    </div>
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
  .field {
    position: relative;
  }
  .field input {
    width: 100%;
  }
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

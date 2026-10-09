<script lang="ts">
  import { getTaskrcText, putTaskrc, restoreTaskrc } from './api';
  import { RotateCcw, TriangleAlert } from './icons';
  import { applyTheme, colourOff, expandIncludes, fetchTheme, THEMES, withColour } from './themes';
  import { store } from './store.svelte';
  import type { TaskrcResponse } from './types';

  let dialog: HTMLDialogElement;
  let text = $state('');
  /** What is saved right now, to detect edits and to revert to. */
  let saved = $state('');
  let loading = $state(true);
  let busy = $state(false);
  let result = $state<TaskrcResponse | null>(null);
  let error = $state<string | null>(null);
  let confirmRestore = $state(false);

  $effect(() => {
    dialog.showModal();
  });

  // Start from what is saved, so an import edits it rather than replacing it with a blank page.
  async function load() {
    loading = true;
    try {
      saved = text = await getTaskrcText();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    void load();
  });

  async function onfile(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (f) {
      text = await f.text();
      result = null;
    }
  }

  // The server returns what it actually saved: show that, not what was pasted, so secrets that
  // were in a pasted file are gone from the page as well as from storage.
  function applied(r: TaskrcResponse) {
    result = r;
    saved = text = r.text;
  }

  // A theme: its colour lines replace the box's colour lines (Save keeps them). Taskwarrior's are static files.
  async function pickTheme(e: Event) {
    const el = e.currentTarget as HTMLSelectElement;
    const id = el.value;
    el.value = '';
    if (!id) return;
    error = null;
    try {
      text = applyTheme(text, id, id === 'meghnaad' ? '' : await fetchTheme(id));
      result = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  const coloursOff = $derived(colourOff(text));

  async function save() {
    busy = true;
    error = null;
    try {
      // `include dark-256.theme`, as a desktop taskrc has it, becomes that theme's lines.
      text = await expandIncludes(text, fetchTheme);
      applied(await putTaskrc(text));
      await store.settingsChanged();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function restore() {
    if (!confirmRestore) {
      confirmRestore = true;
      setTimeout(() => (confirmRestore = false), 4000);
      return;
    }
    confirmRestore = false;
    busy = true;
    error = null;
    try {
      applied(await restoreTaskrc());
      await store.settingsChanged();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  const dirty = $derived(text !== saved);
  const close = () => (store.settingsOpen = false);
</script>

<dialog bind:this={dialog} onclose={close} aria-label="taskrc settings">
  <h3>taskrc settings</h3>
  <p class="dim">
    UDAs, custom reports, contexts and journal settings live in your <code>~/.taskrc</code>, not in your synced data, so
    this app keeps a copy. <strong>They are saved with your tasks and stay until you change them.</strong>
    Edit below, or paste or choose your taskrc. Only those settings are kept:
    <strong>sync settings and anything that looks like a credential are blocked</strong>; they're never stored, logged
    or shown.
  </p>

  {#if store.config?.config_error}
    <p class="bad" role="alert">
      <TriangleAlert size={15} /> The saved settings couldn't be read ({store.config.config_error}). Tasks still work
      with the defaults. Importing below replaces them; the unreadable copy is kept aside.
    </p>
  {/if}

  <div class="colours">
    <label
      >Colour theme
      <select onchange={pickTheme} aria-label="Colour theme">
        <option value="">choose…</option>
        {#each THEMES as t (t.id)}<option value={t.id} title={t.note}>{t.label}</option>{/each}
      </select>
    </label>
    <label class="check"
      ><input
        type="checkbox"
        checked={!coloursOff}
        onchange={(e) => (text = withColour(text, e.currentTarget.checked))}
      />
      Colours</label
    >
    <span class="dim"
      >A theme replaces the colour lines below; Save to keep it. The Meghnaad theme is what applies when there are none.</span
    >
  </div>
  <label class="file">Choose a file <input type="file" onchange={onfile} /></label>
  <label for="rc-text" class="sr-only">Saved taskrc settings</label>
  <textarea
    id="rc-text"
    bind:value={text}
    rows="12"
    class="mono"
    disabled={loading}
    placeholder={loading ? 'Loading saved settings…' : 'Nothing saved yet. Paste your taskrc here.'}
    spellcheck="false"></textarea>

  {#if error}<p class="err" role="alert">{error}</p>{/if}

  {#if result}
    <div class="result" role="status">
      <p class="ok">
        Saved {result.udas} UDA{result.udas === 1 ? '' : 's'}, {result.reports} report{result.reports === 1 ? '' : 's'},
        {result.contexts} context{result.contexts === 1 ? '' : 's'}.
      </p>
      {#if result.blocked.length}
        <p>
          <strong>Blocked for safety ({result.blocked.length}):</strong>
          <span class="mono dim">{result.blocked.join(', ')}</span>
        </p>
      {/if}
      {#each result.warnings as w, _i (_i)}<p class="warn"><TriangleAlert size={14} /> {w}</p>{/each}
      {#if result.ignored.length}
        <details>
          <summary class="dim">{result.ignored.length} other settings not used</summary>
          <p class="mono dim">{result.ignored.join(', ')}</p>
        </details>
      {/if}
    </div>
  {/if}

  <footer class="row">
    {#if store.config?.has_previous}
      <button
        class="ghost btn"
        onclick={restore}
        disabled={busy}
        title="Go back to the settings from before the last save"
      >
        <RotateCcw size={14} />
        {confirmRestore ? 'Click again to restore' : 'Restore previous'}
      </button>
    {/if}
    {#if dirty}
      <button class="ghost" onclick={() => (text = saved)}>Revert edits</button>
    {/if}
    <span class="grow"></span>
    <button onclick={close}>Close</button>
    <button class="primary" onclick={save} disabled={busy || loading || !dirty}>Save</button>
  </footer>
</dialog>

<style>
  dialog {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    padding: 24px 28px;
    width: min(760px, 96vw);
    max-height: 94vh;
    overflow: auto;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  h3 {
    margin: 0 0 4px;
  }
  textarea {
    width: 100%;
    margin: 8px 0;
  }
  .colours {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    margin: 0 0 10px;
  }
  .colours select {
    margin-left: 6px;
  }
  .colours .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .colours .dim {
    flex-basis: 100%;
    font-size: 12.5px;
  }
  .file {
    display: block;
    margin-top: 8px;
  }
  .warn,
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--warn);
  }
  .bad {
    color: var(--err);
  }
  .result p {
    margin: 4px 0;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
  @media (max-width: 760px) {
    textarea {
      min-height: 40vh;
      font-size: 14px;
    }
  }
</style>

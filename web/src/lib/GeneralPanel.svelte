<script lang="ts">
  import { untrack } from 'svelte';
  import { putAppSettings } from './api';
  import { lintSettings, sameHomepage, type Homepage } from './appsettings';
  import { Check, Copy, TriangleAlert } from './icons';
  import { resolveKeymap } from './keymap';
  import { store } from './store.svelte';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  // The choice is one value in the list: `report:next`, `board:week`, or nothing for the default (`next`).
  const toValue = (h: Homepage | null) => (!h ? '' : 'report' in h ? `report:${h.report}` : `board:${h.board}`);
  const fromValue = (v: string): Homepage | null =>
    v.startsWith('report:') ? { report: v.slice(7) } : v.startsWith('board:') ? { board: v.slice(6) } : null;

  const saved = $derived(store.homepage);
  let choice = $state(untrack(() => toValue(saved)));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const edited = $derived(!sameHomepage(fromValue(choice), saved));

  // Saved somewhere else (another tab, the file): follow it, unless there are edits here.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = sameHomepage(fromValue(choice), before);
    const changed = !sameHomepage(now, before);
    before = now;
    if (changed && untouched) choice = toValue(now);
  });

  // The saved choice may name a report or board that is not there (any more): say so rather than show a blank list.
  const missing = $derived(
    saved && 'report' in saved
      ? !store.reports.some((r) => r.name === saved.report)
      : saved
        ? !store.boards.some((b) => b.id === saved.board)
        : false,
  );

  // The whole of settings.json as it is saved, to read, edit or paste over, and to copy to the terminal app or the phone.
  const file = $derived(JSON.stringify(store.appSettings?.raw ?? {}, null, 2) + '\n');
  let text = $state(untrack(() => file));
  const textEdited = $derived(text !== file);
  // Saved somewhere else: follow it, unless the text has edits.
  let fileBefore = untrack(() => file);
  $effect(() => {
    const now = file;
    const untouched = untrack(() => text) === fileBefore;
    fileBefore = now;
    if (untouched) text = now;
  });
  const lint = $derived(lintSettings(text, (raw) => resolveKeymap(raw, store.keymapDefaults).problems));
  let fileBusy = $state(false);
  let fileError = $state<string | null>(null);
  let fileDone = $state(false);

  $effect(() => {
    ondirty?.(edited || textEdited);
  });

  async function applyFile() {
    fileBusy = true;
    fileError = null;
    fileDone = false;
    try {
      // Saved as written: what is pasted is the file.
      await putAppSettings(text);
      await store.loadAppSettings();
      text = file;
      fileDone = true;
    } catch (e) {
      fileError = e instanceof Error ? e.message : String(e);
    } finally {
      fileBusy = false;
    }
  }

  let copied = $state(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      store.notify('Could not copy: select the text and copy it by hand.', 'err');
    }
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      await store.saveHomepage(fromValue(choice));
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label="General settings">
  <h2>General</h2>
  <label class="field">
    <span>Home page</span>
    <select bind:value={choice} onchange={() => (done = false)}>
      <option value="">next (the default)</option>
      <optgroup label="Reports">
        {#each store.offeredReports as r (r.name)}
          {#if r.name !== 'next'}<option value="report:{r.name}">{r.name}</option>{/if}
        {/each}
      </optgroup>
      {#if store.boards.length}
        <optgroup label="Boards">
          {#each store.boards as b (b.id)}<option value="board:{b.id}">{b.name}</option>{/each}
        </optgroup>
      {/if}
      {#if choice && !store.offeredReports.some((r) => choice === `report:${r.name}`) && !store.boards.some((b) => choice === `board:${b.id}`)}
        <option value={choice}>{choice.slice(choice.indexOf(':') + 1)} (not found)</option>
      {/if}
    </select>
  </label>
  <p class="dim">
    The page the app opens on: any report, including your own, or any of your boards. Left alone it is
    <code>next</code>. A link to a task still opens that task. It is kept in this app's own <code>settings.json</code>,
    not in your taskrc.
  </p>
  {#if missing}
    <p class="err" role="alert">
      The saved home page is not there any more, so the app opens on <code>next</code>.
    </p>
  {/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="row">
    <button class="primary" onclick={apply} disabled={busy || !edited}>Apply</button>
    {#if done}<span class="ok"><Check size={14} /> Saved</span>{/if}
  </div>

  <details class="json" open={textEdited || undefined}>
    <summary>settings.json</summary>
    <p class="dim">
      This app's own settings: the home page, appearance, task commands, boards and keys, and the terminal app's and the
      phone's own. Edit it here or paste a whole file over it; what this app doesn't use is kept as it is. The terminal
      app reads the same file from <code>~/.config/meghnaad</code>.
    </p>
    <div class="file">
      <button type="button" class="ghost copy" onclick={copy} aria-label="Copy settings.json">
        {#if copied}<Check size={13} /> Copied{:else}<Copy size={13} /> Copy{/if}
      </button>
      <textarea
        class="mono"
        bind:value={text}
        oninput={() => (fileDone = false)}
        aria-label="settings.json"
        spellcheck="false"
        autocomplete="off"
        autocapitalize="off"
        rows="18"></textarea>
    </div>
    {#if lint.errors.length || lint.warnings.length}
      <ul class="lint" aria-label="Problems in settings.json">
        {#each lint.errors as e, i (i)}<li class="err"><TriangleAlert size={13} /> {e}</li>{/each}
        {#each lint.warnings as w, i (i)}<li class="warn"><TriangleAlert size={13} /> {w} (kept)</li>{/each}
      </ul>
    {/if}
    {#if fileError}<p class="err" role="alert">{fileError}</p>{/if}
    <div class="row">
      <button
        class="primary"
        onclick={applyFile}
        disabled={fileBusy || !textEdited || lint.errors.length > 0}
        title={lint.errors.length ? 'Put the errors right first' : undefined}>Apply</button
      >
      <button type="button" class="ghost" onclick={() => (text = file)} disabled={fileBusy || !textEdited}>Reset</button
      >
      {#if fileDone}<span class="ok"><Check size={14} /> Saved</span>{/if}
    </div>
  </details>
</section>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 360px;
    margin-bottom: 8px;
    font-size: 12px;
    color: var(--dim);
  }
  .field select {
    font-size: 14px;
    color: var(--text);
  }
  .json {
    margin-top: 20px;
  }
  .json summary {
    cursor: pointer;
    font-family: var(--mono);
    font-size: 13px;
  }
  .file {
    position: relative;
  }
  .file textarea {
    display: block;
    box-sizing: border-box;
    width: 100%;
    margin: 6px 0 0;
    padding: 10px 12px;
    background: var(--panel-2);
    border-radius: var(--radius);
    font-size: 12.5px;
    resize: vertical;
    max-height: 70vh;
  }
  .lint {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    font-size: 12.5px;
  }
  .lint li {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .lint .warn {
    color: var(--dim);
  }
  .copy {
    position: absolute;
    top: 6px;
    right: 6px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    background: var(--panel);
  }
  h2 {
    font-size: 16px;
    margin: 0 0 10px;
  }
</style>

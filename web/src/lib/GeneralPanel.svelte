<script lang="ts">
  import { untrack } from 'svelte';
  import { sameHomepage, type Homepage } from './appsettings';
  import { Check } from './icons';
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
  $effect(() => {
    ondirty?.(edited);
  });

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
  h2 {
    font-size: 16px;
    margin: 0 0 10px;
  }
</style>

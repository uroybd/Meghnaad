<script lang="ts">
  import { untrack } from 'svelte';
  import { runCli } from './api';
  import { Check, TriangleAlert } from './icons';
  import { store } from './store.svelte';
  import {
    configArgs,
    REVIEW_FILTER,
    REVIEW_SORT,
    setupLines,
    taskshStatus,
    type Part,
    type ReviewWanted,
  } from './tasksh';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  const status = $derived(taskshStatus(store.config));
  const saved = $derived<ReviewWanted>({ filter: status.filter ?? REVIEW_FILTER, sort: status.sort ?? REVIEW_SORT });

  // Start from what is saved; `saved` is followed again below when it changes.
  let filter = $state(untrack(() => saved.filter));
  let sort = $state(untrack(() => saved.sort));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const want = $derived<ReviewWanted>({ filter: filter.trim(), sort: sort.trim() });
  const valid = $derived(!!want.filter && !!want.sort && !/[\n\r]/.test(filter + sort));
  /** What Apply would write: nothing when the taskrc already is as wanted. */
  const lines = $derived(setupLines(store.config, want));
  const edited = $derived(want.filter !== saved.filter || want.sort !== saved.sort);
  const blocked = $derived(!!store.config?.config_error);
  $effect(() => {
    ondirty?.(edited);
  });

  // The saved report changed somewhere else (the taskrc tab, the console): follow it, unless there are edits here.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = want.filter === before.filter && want.sort === before.sort;
    const changed = now.filter !== before.filter || now.sort !== before.sort;
    before = now;
    if (changed && untouched) {
      filter = now.filter;
      sort = now.sort;
    }
  });

  // One `config` line at a time: each reads the saved settings and writes them back.
  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      for (const line of lines) {
        const r = (await runCli({ args: configArgs(line) })).result;
        if (r.kind === 'error') throw new Error(r.message);
      }
      await store.settingsChanged();
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      await store.settingsChanged();
    } finally {
      busy = false;
    }
  }

  function defaults() {
    filter = REVIEW_FILTER;
    sort = REVIEW_SORT;
    done = false;
  }

  const label: Record<Part, string> = { ok: 'defined', missing: 'not defined', different: 'defined differently' };
</script>

<section aria-label="tasksh settings">
  <h2>tasksh</h2>
  <p class="dim">
    tasksh's <code>review</code> walks the tasks you have not looked at lately and marks each as reviewed. It needs two
    things in your taskrc: a <code>reviewed</code> date attribute (UDA) that records when, and a
    <code>_reviewed</code> report that lists the tasks due. Apply writes whichever is missing, with the same lines tasksh
    writes the first time it runs, and any change you make to the filter or order below.
  </p>

  {#if blocked}
    <p class="bad" role="alert">
      <TriangleAlert size={15} /> The saved settings couldn't be read, so nothing can be applied until that is fixed in the
      taskrc tab.
    </p>
  {/if}

  <ul class="parts" aria-label="Status">
    <li class={status.uda}>
      <span class="mark" aria-hidden="true"
        >{#if status.uda === 'ok'}<Check size={14} />{:else}<TriangleAlert size={14} />{/if}</span
      >
      <span>
        <code>reviewed</code> attribute, type <code>date</code>: <strong>{label[status.uda]}</strong>
        {#if status.uda === 'different'}
          <span class="dim hint"
            >It is a <code>{status.udaType}</code> now. Applying changes its type to <code>date</code>.</span
          >
        {/if}
      </span>
    </li>
    <li class={status.report}>
      <span class="mark" aria-hidden="true"
        >{#if status.report === 'ok'}<Check size={14} />{:else}<TriangleAlert size={14} />{/if}</span
      >
      <span>
        <code>_reviewed</code> report with one <code>uuid</code> column: <strong>{label[status.report]}</strong>
        {#if status.report === 'different'}
          <span class="dim hint"
            >It has the columns <code>{status.columns.join(', ')}</code> now. Applying replaces them with
            <code>uuid</code>.</span
          >
        {/if}
      </span>
    </li>
  </ul>

  <div class="field">
    <label for="ts-filter">Tasks due for review <span class="dim">(<code>report._reviewed.filter</code>)</span></label>
    <input
      id="ts-filter"
      class="mono"
      bind:value={filter}
      oninput={() => (done = false)}
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
    />
    <span class="dim hint"
      >By default: never reviewed, or not for 6 days, and still pending or waiting. For a month, use
      <code>reviewed.before:now-30days</code>.</span
    >
  </div>
  <div class="field">
    <label for="ts-sort">Order <span class="dim">(<code>report._reviewed.sort</code>)</span></label>
    <input
      id="ts-sort"
      class="mono"
      bind:value={sort}
      oninput={() => (done = false)}
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
    />
    <span class="dim hint">By default the longest-ago first, then the least recently changed.</span>
  </div>

  {#if lines.length}
    <div class="plan">
      <strong>Apply will write {lines.length} setting{lines.length === 1 ? '' : 's'}:</strong>
      <ul class="mono">
        {#each lines as [name, value] (name)}<li>{name}={value}</li>{/each}
      </ul>
    </div>
  {:else if status.enabled}
    <p class="dim">The taskrc already has all of this.</p>
  {/if}

  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if done && !edited}<p class="ok" role="status">Applied. <code>review</code> can use it now.</p>{/if}

  <footer class="row">
    {#if edited}<button class="ghost" onclick={() => ((filter = saved.filter), (sort = saved.sort))}
        >Revert edits</button
      >{/if}
    <button class="ghost" onclick={defaults} disabled={filter === REVIEW_FILTER && sort === REVIEW_SORT}
      >Use tasksh's defaults</button
    >
    <span class="grow"></span>
    <button class="primary" onclick={apply} disabled={busy || blocked || !valid || lines.length === 0}>Apply</button>
  </footer>
</section>

<style>
  h2 {
    margin: 0 0 4px;
    font-size: 18px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--err);
  }
  .parts {
    list-style: none;
    margin: 12px 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .parts li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 9px 12px;
  }
  .parts li + li {
    border-top: 1px solid var(--line);
  }
  .mark {
    flex: none;
    line-height: 0;
    margin-top: 3px;
  }
  .ok .mark,
  li.ok .mark {
    color: var(--ok);
  }
  li.missing .mark,
  li.different .mark {
    color: var(--warn);
  }
  .hint {
    display: block;
    font-size: 12.5px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 14px 0;
  }
  .field input {
    width: 100%;
  }
  .plan {
    margin: 12px 0;
    padding: 10px 12px;
    background: var(--panel-2);
    border-radius: var(--radius);
    font-size: 13px;
  }
  .plan ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  .plan li {
    overflow-wrap: anywhere;
  }
  .ok {
    color: var(--ok);
  }
  footer {
    margin-top: 16px;
    gap: 8px;
  }
</style>

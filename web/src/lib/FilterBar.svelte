<script lang="ts">
  import { reportArgs, shellQuote, splitWords } from './cmdline';
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import DateTimeInput from './DateTimeInput.svelte';
  import { ArrowDown, ArrowUp, SlidersHorizontal, X } from './icons';
  import { store } from './store.svelte';

  let hintsOpen = $state(false);
  let dueBefore = $state('');
  const completer = new Completer();

  const meta = $derived(store.reports.find((r) => r.name === store.report));
  const args = $derived(reportArgs(store.filter, store.report));
  const words = $derived(splitWords(store.filter));
  const cfg = $derived(store.config?.config);
  const context = $derived(
    cfg?.active_context ? { name: cfg.active_context, read: cfg.contexts[cfg.active_context]?.read ?? null } : null,
  );

  // Run the report whenever the report or filter changes. Only typing is debounced: opening the
  // page or picking another report should not wait for a keystroke that isn't coming. The console
  // may already have loaded exactly this (it hands its result over), so skip duplicates.
  let ranFilter: string | null = null;
  $effect(() => {
    const a = args;
    const filter = store.filter;
    if (JSON.stringify(a) === store.liveKey) {
      ranFilter = filter;
      return;
    }
    const typing = ranFilter !== null && filter !== ranFilter;
    const t = setTimeout(() => {
      ranFilter = filter;
      void store.runLive({ args: a });
    }, typing ? 250 : 0);
    return () => clearTimeout(t);
  });

  function addToken(tok: string) {
    if (words.includes(tok)) return;
    store.filter = [...words, tok].map(shellQuote).join(' ');
  }

  function removeWord(i: number) {
    store.filter = words.filter((_, j) => j !== i).map(shellQuote).join(' ');
  }

  // `rc.report.<name>.sort:due-` is how a table-header sort travels on the command line.
  const sortToken = /^rc\.report\.[^.]+\.sort[:=]/;
  function chipLabel(w: string): string {
    return sortToken.test(w) ? `sort ${w.slice(w.search(/[:=]/) + 1)}` : w;
  }

  function onkeydown(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLInputElement;
    const open = !!completer.menu;
    const apply = (r: { line: string; caret: number } | null) => {
      if (!r) return false;
      store.filter = r.line;
      queueMicrotask(() => el.setSelectionRange(r.caret, r.caret));
      return true;
    };
    if (e.key === 'Tab') {
      if (apply(completer.tab(store.filter, el.selectionStart ?? store.filter.length, store.vocab, e.shiftKey))) e.preventDefault();
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && open) {
      e.preventDefault();
      apply(completer.move(e.key === 'ArrowDown' ? 1 : -1));
    } else if (e.key === 'Enter' && open) {
      e.preventDefault();
      completer.close();
    } else if (e.key === 'Escape') {
      completer.close();
    }
  }

  function pickReport(name: string) {
    // Sort overrides belong to one report; drop the ones that no longer apply.
    store.filter = words
      .filter((w) => !/^rc\.report\./.test(w) || w.startsWith(`rc.report.${name}.`))
      .map(shellQuote)
      .join(' ');
    store.report = name;
  }
</script>

<section class="bar" aria-label="Report and filters">
  <div class="row top">
    <label class="sr-only" for="fb-report">Report</label>
    <select id="fb-report" value={store.report} onchange={(e) => pickReport(e.currentTarget.value)} title={meta?.description ?? ''}>
      {#each store.reports as r (r.name)}
        <option value={r.name}>{r.name}{r.description ? ` — ${r.description}` : ''}</option>
      {/each}
      {#if store.reports.length === 0}<option value={store.report}>{store.report}</option>{/if}
    </select>
    <div class="grow filter">
      <label class="sr-only" for="fb-filter">Filter</label>
      <input
        id="fb-filter"
        bind:value={store.filter}
        {onkeydown}
        oninput={() => completer.close()}
        onblur={() => completer.close()}
        placeholder="Add filters: project:Home +work due.before:eow  (Tab completes)"
        autocomplete="off"
        spellcheck="false"
        class="mono"
      />
      <CompletionMenu
        {completer}
        placement="below"
        onpick={(i) => {
          const r = completer.pick(i);
          if (r) store.filter = r.line;
        }}
      />
    </div>
  </div>

  {#if words.length}
    <div class="chips" aria-label="Active filters">
      {#each words as w, i (i + w)}
        <span class="chip mono" class:sort={sortToken.test(w)} title={w}>
          {chipLabel(w)}<button class="ghost x" aria-label="Remove {chipLabel(w)}" onclick={() => removeWord(i)}><X size={11} /></button>
        </span>
      {/each}
      <button class="ghost" onclick={() => (store.filter = '')}>clear all</button>
    </div>
  {/if}

  <details class="helpers" bind:open={hintsOpen}>
    <summary><SlidersHorizontal size={13} /> Filter helpers</summary>
    <div class="row wrap">
      <label>Project
        <select onchange={(e) => { if (e.currentTarget.value) addToken(`project:${e.currentTarget.value}`); e.currentTarget.value = ''; }}>
          <option value="">add…</option>
          {#each store.projects as p}<option value={p}>{p}</option>{/each}
        </select>
      </label>
      <label>Tag
        <select onchange={(e) => { if (e.currentTarget.value) addToken(e.currentTarget.value); e.currentTarget.value = ''; }}>
          <option value="">add…</option>
          {#each store.tags as t}<option value={`+${t}`}>+{t}</option><option value={`-${t}`}>-{t}</option>{/each}
        </select>
      </label>
      <label>State
        <select onchange={(e) => { if (e.currentTarget.value) addToken(e.currentTarget.value); e.currentTarget.value = ''; }}>
          <option value="">add…</option>
          <option value="status:pending">pending</option>
          <option value="status:completed">completed</option>
          <option value="status:deleted">deleted</option>
          <option value="+OVERDUE">overdue</option>
          <option value="+DUETODAY">due today</option>
          <option value="+WEEK">due this week</option>
          <option value="+ACTIVE">active</option>
          <option value="+BLOCKED">blocked</option>
          <option value="+READY">ready</option>
        </select>
      </label>
      <label>Priority
        <select onchange={(e) => { if (e.currentTarget.value) addToken(e.currentTarget.value); e.currentTarget.value = ''; }}>
          <option value="">add…</option>
          <option value="priority:H">H</option><option value="priority:M">M</option>
          <option value="priority:L">L</option><option value="priority.none:">none</option>
        </select>
      </label>
      <label>Due before
        <DateTimeInput label="Due before" bind:value={dueBefore} />
        <button disabled={!dueBefore} onclick={() => { addToken(`due.before:${dueBefore}`); dueBefore = ''; }}>add</button>
      </label>
    </div>
  </details>

  {#if meta?.filter || meta?.sort || context}
    <p class="implicit dim">
      {#if meta?.filter}report filter <code>{meta.filter}</code>{/if}
      {#if meta?.sort} · default sort <code>{meta.sort}</code>{/if}
      {#if context} · context <code>{context.name}{context.read ? ` (${context.read})` : ''}</code>{/if}
      <span class="tail">— applied automatically; your filters are added to these. Click a column header to sort.</span>
    </p>
  {/if}
</section>

<style>
  .bar { display: grid; gap: 10px; margin-bottom: 18px; }
  .top select { max-width: 17em; }
  @media (max-width: 760px) {
    .top { flex-wrap: wrap; }
    .top select { max-width: 100%; width: 100%; }
    .top .filter { flex-basis: 100%; }
    .helpers .row { gap: 10px 14px; }
    .helpers label { font-size: 13px; }
    .chip { line-height: 26px; }
    .x { padding: 0 6px; min-width: 28px; }
    .implicit .tail { display: none; }
  }
  .filter { position: relative; }
  .filter input { width: 100%; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .chip.sort { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
  .x { padding: 0 3px; font-size: 10px; margin-left: 3px; color: inherit; }
  .helpers summary { cursor: pointer; color: var(--dim); font-size: 13px; display: inline-flex; align-items: center; gap: 5px; }
  .chip { display: inline-flex; align-items: center; }
  .helpers .row { margin-top: 10px; gap: 12px 18px; }
  .wrap { flex-wrap: wrap; }
  .helpers label { display: inline-flex; gap: 6px; align-items: center; color: var(--dim); font-size: 13px; }
  .implicit { margin: 0; font-size: 13px; }
  .implicit code { background: var(--panel-2); border-radius: 4px; padding: 1px 5px; }
</style>

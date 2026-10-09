<script lang="ts">
  import { shellQuote, splitWords } from './cmdline';
  import DateTimeInput from './DateTimeInput.svelte';
  import { SlidersHorizontal, X } from './icons';
  import { store } from './store.svelte';

  /** The words of a filter as removable chips, and the "Filter helpers" for adding some without typing. */
  let { value = $bindable('') }: { value: string } = $props();

  let hintsOpen = $state(false);
  let dueBefore = $state('');
  const words = $derived(splitWords(value));

  function addToken(tok: string) {
    if (words.includes(tok)) return;
    value = [...words, tok].map(shellQuote).join(' ');
  }

  function removeWord(i: number) {
    value = words.filter((_, j) => j !== i).map(shellQuote).join(' ');
  }

  // `rc.report.<name>.sort:due-` is how a table-header sort travels on the command line.
  const sortToken = /^rc\.report\.[^.]+\.sort[:=]/;
  function chipLabel(w: string): string {
    return sortToken.test(w) ? `sort ${w.slice(w.search(/[:=]/) + 1)}` : w;
  }

  /** A `<select>` of things to add: picking one adds it and resets the box. */
  const pick = (e: Event, make: (v: string) => string = (v) => v) => {
    const el = e.currentTarget as HTMLSelectElement;
    if (el.value) addToken(make(el.value));
    el.value = '';
  };
</script>

{#if words.length}
  <div class="chips" aria-label="Active filters">
    {#each words as w, i (i + w)}
      <span class="chip mono" class:sort={sortToken.test(w)} title={w}>
        {chipLabel(w)}<button class="ghost x" aria-label="Remove {chipLabel(w)}" onclick={() => removeWord(i)}><X size={11} /></button>
      </span>
    {/each}
    <button class="ghost" onclick={() => (value = '')}>clear all</button>
  </div>
{/if}

<details class="helpers" bind:open={hintsOpen}>
  <summary><SlidersHorizontal size={13} /> Filter helpers</summary>
  <div class="row wrap">
    <label>Project
      <select onchange={(e) => pick(e, (v) => `project:${v}`)}>
        <option value="">add…</option>
        {#each store.projects as p}<option value={p}>{p}</option>{/each}
      </select>
    </label>
    <label>Tag
      <select onchange={(e) => pick(e)}>
        <option value="">add…</option>
        {#each store.tags as t}<option value={`+${t}`}>+{t}</option><option value={`-${t}`}>-{t}</option>{/each}
      </select>
    </label>
    <label>State
      <select onchange={(e) => pick(e)}>
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
      <select onchange={(e) => pick(e)}>
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

<style>
  .chips { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .chip { display: inline-flex; align-items: center; }
  .chip.sort { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
  .x { padding: 0 3px; font-size: 10px; margin-left: 3px; color: inherit; }
  .helpers summary { cursor: pointer; color: var(--dim); font-size: 13px; display: inline-flex; align-items: center; gap: 5px; }
  .helpers .row { margin-top: 10px; gap: 12px 18px; }
  .wrap { flex-wrap: wrap; }
  .helpers label { display: inline-flex; gap: 6px; align-items: center; color: var(--dim); font-size: 13px; }
  @media (max-width: 760px) {
    .helpers .row { gap: 10px 14px; }
    .helpers label { font-size: 13px; }
    .chip { line-height: 26px; }
    .x { padding: 0 6px; min-width: 28px; }
  }
</style>

<script lang="ts">
  import { describe, fieldsFor, modifyWords, problem, type Entry } from './bulk';
  import { Plus, X } from './icons';
  import { store } from './store.svelte';
  import SuggestInput from './SuggestInput.svelte';

  let {
    count,
    onapply,
    onclose,
  }: {
    /** How many tasks will change. */
    count: number;
    /** Run `modify` with these words; true when it went through (the dialog then closes). */
    onapply: (words: string[]) => Promise<boolean>;
    onclose: () => void;
  } = $props();

  let dialog: HTMLDialogElement;
  const fields = $derived(fieldsFor(store.config?.config.udas ?? {}));
  const field = (key: string) => fields.find((f) => f.key === key);

  // Each entry has its own key, so removing one in the middle keeps what the others hold.
  let nextKey = 1;
  type Row = Entry & { rk: number };
  let entries = $state<Row[]>([{ key: 'project', value: '', rk: nextKey++ }]);
  let busy = $state(false);

  $effect(() => {
    dialog.showModal();
  });

  const add = () => {
    // The next field nobody has used yet, so a second entry is not a duplicate by default.
    const used = new Set(entries.map((e) => e.key));
    const next = fields.find((f) => !used.has(f.key) || f.key === '+' || f.key === '-');
    entries.push({ key: next?.key ?? 'project', value: '', rk: nextKey++ });
  };
  const remove = (rk: number) => (entries = entries.filter((e) => e.rk !== rk));

  // What is wrong, if anything: an unusable entry, or the same field twice (tags excepted).
  const issue = $derived.by(() => {
    if (!entries.length) return 'Add at least one change.';
    for (const e of entries) {
      const p = problem(e, fields);
      if (p) return p;
    }
    const seen = new Set<string>();
    for (const e of entries) {
      if (e.key === '+' || e.key === '-') continue;
      if (seen.has(e.key)) return `${field(e.key)?.label ?? e.key} is set twice.`;
      seen.add(e.key);
    }
    return null;
  });

  const summary = $derived(entries.length && !issue ? describe(entries, fields) : '');

  async function apply() {
    if (issue || busy) return;
    busy = true;
    try {
      if (await onapply(modifyWords(entries))) onclose();
    } finally {
      busy = false;
    }
  }
</script>

<dialog bind:this={dialog} {onclose} aria-label="Modify {count} tasks">
  <h3>Modify {count} tasks</h3>
  <p class="dim">
    Each line sets one field on every selected task. Leave the value empty to clear a field. A line for tags adds or
    removes the tags you name.
  </p>

  <div class="entries">
    {#each entries as e, i (e.rk)}
      {@const f = field(e.key)}
      <div class="entry" data-testid="bulk-entry">
        <select bind:value={e.key} aria-label="Field {i + 1}">
          {#each fields as o (o.key)}<option value={o.key}>{o.label}</option>{/each}
        </select>
        {#if f?.kind === 'project'}
          <SuggestInput id="bm-{e.rk}" bind:value={e.value} names={store.projects} noun="project" placeholder="none" />
        {:else if f?.kind === 'tags'}
          <SuggestInput id="bm-{e.rk}" bind:value={e.value} names={store.tags} noun="tag" list placeholder="tags" />
        {:else if f?.values}
          <select bind:value={e.value} aria-label="Value of {f.label}">
            <option value="">clear</option>
            {#each f.values as v (v)}<option value={v}>{v}</option>{/each}
          </select>
        {:else}
          <input bind:value={e.value} aria-label="Value of {f?.label ?? 'the field'}" placeholder={f?.hint ?? ''} />
        {/if}
        <button class="ghost" aria-label="Remove this change" onclick={() => remove(e.rk)}><X size={16} /></button>
      </div>
    {/each}
  </div>

  <button class="ghost add" onclick={add}><Plus size={15} /> Add a change</button>

  {#if issue}<p class="err" role="alert">{issue}</p>{:else if summary}<p class="sum" data-testid="bulk-summary">
      On {count} tasks: {summary}.
    </p>{/if}

  <footer class="row">
    <span class="grow"></span>
    <button onclick={onclose}>Cancel</button>
    <button class="primary" disabled={!!issue || busy} onclick={apply}>Modify {count} tasks</button>
  </footer>
</dialog>

<style>
  dialog {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    padding: 22px 26px;
    width: min(620px, 96vw);
    max-height: 94vh;
    overflow: auto;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  h3 {
    margin: 0 0 4px;
  }
  .entries {
    display: grid;
    gap: 8px;
    margin: 12px 0 6px;
  }
  .entry {
    display: grid;
    grid-template-columns: 10em minmax(0, 1fr) auto;
    gap: 8px;
    align-items: start;
  }
  .sum {
    margin: 10px 0 0;
    overflow-wrap: anywhere;
  }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  @media (max-width: 760px) {
    .entry {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .entry select:first-child {
      grid-column: 1 / -1;
    }
  }
</style>

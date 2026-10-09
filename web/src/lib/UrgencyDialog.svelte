<script lang="ts">
  import { putUrgency } from './api';
  import { Plus, RotateCcw, TriangleAlert, X } from './icons';
  import { store } from './store.svelte';
  import {
    ADDABLE,
    buildKey,
    describe,
    effective,
    sameOverrides,
    sortKeys,
    validName,
    validUda,
    withValue,
  } from './urgency';

  let dialog: HTMLDialogElement;
  $effect(() => {
    dialog.showModal();
  });

  const defaults = $derived(store.config?.urgency_defaults ?? {});
  /** What is saved right now, to detect edits and to revert to. */
  const savedOverrides = $derived({ ...(store.config?.config.urgency ?? {}) });
  const savedInherit = $derived(store.config?.urgency_inherit ?? false);

  // Start from what is saved; the dialog is only mounted when it opens.
  let overrides = $state<Record<string, number>>({ ...(store.config?.config.urgency ?? {}) });
  let inherit = $state(store.config?.urgency_inherit ?? false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let savedNote = $state(false);

  const keys = $derived(sortKeys(new Set([...Object.keys(defaults), ...Object.keys(overrides)])));
  const dirty = $derived(!sameOverrides(overrides, savedOverrides) || inherit !== savedInherit);
  const blocked = $derived(!!store.config?.config_error);

  function set(key: string, raw: string) {
    const v = Number(raw);
    if (raw.trim() === '' || !Number.isFinite(v)) return;
    overrides = withValue(overrides, defaults, key, v);
    savedNote = false;
  }

  function reset(key: string) {
    const { [key]: _drop, ...rest } = overrides;
    overrides = rest;
    savedNote = false;
  }

  // Adding a setting.
  let addIndex = $state(0);
  let addName = $state('');
  // A number input binds to a number, or null while it is empty.
  let addValue = $state<number | null>(1);
  const adding = $derived(ADDABLE[addIndex]);
  const addKey = $derived(
    adding.kind === 'uda'
      ? validUda(addName)
        ? buildKey('uda', addName)
        : null
      : validName(addName)
        ? buildKey(adding.kind, addName)
        : null,
  );
  const canAdd = $derived(!!addKey && typeof addValue === 'number' && Number.isFinite(addValue));
  // UDA = value only makes sense with a value; UDA (any value) only without one.
  const addShapeOk = $derived(
    adding.label === 'UDA = value'
      ? addName.includes('.')
      : adding.label === 'UDA (any value)'
        ? !addName.includes('.')
        : true,
  );

  function add() {
    if (!addKey || !canAdd || !addShapeOk) return;
    overrides = withValue(overrides, defaults, addKey, addValue as number);
    addName = '';
    savedNote = false;
  }

  async function save() {
    busy = true;
    error = null;
    try {
      await putUrgency(overrides, inherit);
      await store.loadConfig();
      // Scores, and so the order of tasks, may have changed.
      if (store.live) await store.refresh(store.live);
      savedNote = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function revert() {
    overrides = { ...savedOverrides };
    inherit = savedInherit;
    savedNote = false;
  }

  const close = () => (store.urgencyOpen = false);
  const isCustom = (key: string) => !(key in defaults);
  const fmt = (n: number | undefined) => (n === undefined ? '' : String(n));
</script>

<dialog bind:this={dialog} onclose={close} aria-label="Urgency settings">
  <h3>Urgency</h3>
  <p class="dim">
    A task's urgency is the sum of these terms, as in Taskwarrior's <code>urgency.*</code> settings. Changes are saved
    with your other imported settings, so they appear in the taskrc dialog and <em>Restore previous</em> covers them.
  </p>
  <ul class="rules dim">
    <li>
      Coefficients <strong>add up</strong>: a task in <code>Home.Kitchen</code> gets the <code>Home</code> and the
      <code>Home.Kitchen</code> coefficient.
    </li>
    <li>
      A project coefficient applies to that project <strong>and its sub-projects</strong>, never to a different project
      that merely starts with the same letters (<code>Work</code> does not reach <code>Workshop</code>).
    </li>
    <li>A tag coefficient also works for virtual tags such as <code>OVERDUE</code>.</li>
  </ul>

  {#if blocked}
    <p class="bad" role="alert">
      <TriangleAlert size={15} /> The saved settings couldn't be read, so urgency can't be saved until that is fixed in the
      taskrc dialog.
    </p>
  {/if}

  <label class="inherit">
    <input type="checkbox" bind:checked={inherit} onchange={() => (savedNote = false)} />
    <span>
      <strong>Blocking tasks inherit urgency</strong> <code>urgency.inherit</code>
      <span class="dim hint"
        >A task that blocks others is at least as urgent as the most urgent of them, and a little more, so it sorts
        first. Off in Taskwarrior unless you turn it on.</span
      >
    </span>
  </label>

  <div class="table" role="table" aria-label="Urgency coefficients">
    <div class="head" role="row">
      <span role="columnheader">Setting</span>
      <span role="columnheader" class="num">Default</span>
      <span role="columnheader" class="num">Value</span>
      <span role="columnheader"></span>
    </div>
    {#each keys as key (key)}
      {@const d = describe(key)}
      {@const overridden = key in overrides}
      <div class="row" class:changed={overridden} role="row">
        <span class="what" role="cell">
          <span>{d.label}</span>
          <span class="dim hint">{d.hint}</span>
          <span class="dim mono key">{key}</span>
        </span>
        <span class="num dim" role="cell">{fmt(defaults[key])}</span>
        <span class="num" role="cell">
          <input
            type="number"
            step="any"
            aria-label="{d.label} value"
            value={effective(key, overrides, defaults)}
            onchange={(e) => set(key, e.currentTarget.value)}
          />
        </span>
        <span class="act" role="cell">
          {#if isCustom(key)}
            <button class="ghost" aria-label="Remove {d.label}" title="Remove" onclick={() => reset(key)}
              ><X size={14} /></button
            >
          {:else if overridden}
            <button
              class="ghost"
              aria-label="Back to default for {d.label}"
              title="Back to the default"
              onclick={() => reset(key)}><RotateCcw size={14} /></button
            >
          {/if}
        </span>
      </div>
    {/each}
  </div>

  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      add();
    }}
  >
    <label>
      <span class="sr-only">Kind</span>
      <select bind:value={addIndex} aria-label="Kind of the new setting">
        {#each ADDABLE as a, i (i)}<option value={i}>{a.label}</option>{/each}
      </select>
    </label>
    <input
      class="grow"
      bind:value={addName}
      placeholder={adding.placeholder}
      aria-label="Name of the new setting"
      spellcheck="false"
      autocapitalize="off"
    />
    <input class="coef" type="number" step="any" bind:value={addValue} aria-label="Coefficient of the new setting" />
    <button type="submit" class="btn" disabled={!canAdd || !addShapeOk}><Plus size={14} /> Add</button>
  </form>
  {#if addName && (!addKey || !addShapeOk)}
    <p class="warn small">
      {#if adding.label === 'UDA = value'}Write the UDA, a dot and its value: <code>estimate.huge</code>.
      {:else if adding.label === 'UDA (any value)'}Write just the UDA name: <code>estimate</code>.
      {:else}Names can't contain spaces, <code>=</code> or <code>#</code>.{/if}
    </p>
  {/if}

  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if savedNote && !dirty}<p class="ok" role="status">Saved. Task scores are updated.</p>{/if}

  <footer class="row-actions">
    {#if dirty}<button class="ghost" onclick={revert}>Revert edits</button>{/if}
    <span class="grow"></span>
    <button onclick={close}>Close</button>
    <button class="primary" onclick={save} disabled={busy || blocked || !dirty}>Save</button>
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
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
  .rules {
    margin: 6px 0 12px;
    padding-left: 18px;
  }
  .rules li {
    margin: 2px 0;
  }
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--err);
  }
  .warn {
    color: var(--warn);
  }
  .small {
    font-size: 13px;
    margin: 4px 0;
  }
  .ok {
    color: var(--ok);
  }
  .inherit {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    margin: 8px 0 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel-2);
  }
  .inherit input {
    margin-top: 2px;
    flex: none;
    width: 16px;
    height: 16px;
  }
  .hint {
    display: block;
    font-size: 12.5px;
  }
  .table {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: 1fr 72px 110px 36px;
    gap: 8px;
    align-items: center;
    padding: 6px 10px;
  }
  .head {
    background: var(--panel-2);
    font-size: 12px;
    color: var(--dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row {
    border-top: 1px solid var(--line);
  }
  .row.changed {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .key {
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
  .num {
    text-align: right;
  }
  .num input {
    width: 100%;
    text-align: right;
  }
  .act {
    display: flex;
    justify-content: center;
  }
  .add {
    display: flex;
    gap: 8px;
    margin-top: 12px;
    flex-wrap: wrap;
  }
  .add .grow {
    flex: 1;
    min-width: 140px;
  }
  .add .coef {
    width: 90px;
    text-align: right;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .row-actions {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 16px;
  }
  .row-actions .grow {
    flex: 1;
  }
  @media (max-width: 760px) {
    dialog {
      padding: 18px 14px;
    }
    .head,
    .row {
      grid-template-columns: 1fr 60px 90px 32px;
      gap: 6px;
      padding: 6px;
    }
  }
</style>

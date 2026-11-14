<script lang="ts">
  import {
    CONTROL_TYPES,
    controlLabel,
    newControl,
    usage,
    type Control,
    type ControlType,
    type ControlValue,
  } from './boards';
  import ControlField from './ControlField.svelte';
  import { Plus, Trash2 } from './icons';
  import { store } from './store.svelte';

  /**
   * The controls a board offers, as it is edited: each a name (what a filter writes after `$`), an optional label (what is
   * shown; any text), its settings by kind, and the value it starts with. What the engine thinks of them (a name used twice,
   * a date it cannot read) is said with the board's other problems.
   */
  let { controls = $bindable() }: { controls: Control[] } = $props();

  /** What a selection can pick from: projects, tags, and each UDA with values. */
  const sources = $derived([
    { id: 'project', label: 'Projects' },
    { id: 'tags', label: 'Tags' },
    ...store.udas.filter((u) => u.values.length).map((u) => ({ id: u.name, label: u.label || u.name })),
  ]);

  function add(type: ControlType) {
    const base = type === 'daterange' ? 'daterange' : type;
    const taken = new Set(controls.map((c) => c.name));
    let n = 1;
    while (taken.has(n === 1 ? base : `${base}${n}`)) n++;
    controls = [...controls, newControl(type, n === 1 ? base : `${base}${n}`)];
  }
  const set = (i: number, patch: Partial<Control> | ControlValue) =>
    (controls = controls.map((c, j) => (j === i ? ({ ...c, ...patch } as Control) : c)));
  const called = (c: Control, i: number) => c.name || `control ${i + 1}`;
  const kindOf = (t: ControlType) => CONTROL_TYPES.find((k) => k.id === t)?.label ?? t;
</script>

<fieldset class="controls">
  <legend>Controls</legend>
  {#each controls as c, i (i)}
    <div class="control" role="group" aria-label="{kindOf(c.type)} {controlLabel(c)}">
      <div class="head">
        <span class="kind">{kindOf(c.type)}</span>
        <label>
          Name
          <input
            class="mono"
            value={c.name}
            oninput={(e) => set(i, { name: e.currentTarget.value.trim().toLowerCase() })}
            spellcheck="false"
            autocomplete="off"
            aria-label="Name of control {i + 1}"
          />
        </label>
        <label>
          Label
          <input
            value={c.label ?? ''}
            placeholder={controlLabel({ name: c.name })}
            oninput={(e) => set(i, { label: e.currentTarget.value || undefined })}
            aria-label="Label of control {i + 1}"
          />
        </label>
        <button
          type="button"
          class="ghost"
          aria-label="Remove {called(c, i)}"
          title="Remove"
          onclick={() => (controls = controls.filter((_, j) => j !== i))}><Trash2 size={15} /></button
        >
      </div>
      {#if c.type === 'select'}
        <div class="settings">
          <label>
            From
            <select
              value={c.source}
              aria-label="What {called(c, i)} picks from"
              onchange={(e) => set(i, { source: e.currentTarget.value, values: undefined })}
            >
              {#each sources as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
              {#if !sources.some((s) => s.id === c.source)}<option value={c.source}>{c.source}</option>{/if}
            </select>
          </label>
          <label class="check">
            <input
              type="checkbox"
              checked={!!c.multiple}
              onchange={(e) =>
                set(i, {
                  multiple: e.currentTarget.checked || undefined,
                  values: e.currentTarget.checked ? c.values : c.values?.slice(0, 1),
                })}
            />
            Several at once
          </label>
          {#if c.multiple}
            <label>
              Join with
              <input
                class="mono join"
                value={c.join ?? ''}
                placeholder="or"
                oninput={(e) => set(i, { join: e.currentTarget.value || undefined })}
                spellcheck="false"
                autocomplete="off"
                aria-label="What joins the values of {called(c, i)}"
              />
            </label>
          {/if}
          <label class="check">
            <input
              type="checkbox"
              checked={!!c.completed}
              onchange={(e) => set(i, { completed: e.currentTarget.checked || undefined })}
            />
            Offer values of completed tasks
          </label>
          <label class="check">
            <input
              type="checkbox"
              checked={!!c.typed}
              onchange={(e) => set(i, { typed: e.currentTarget.checked || undefined })}
            />
            Typed in at first
          </label>
        </div>
      {:else if c.type === 'boolean'}
        <div class="settings">
          <label>
            When on
            <input
              class="mono"
              value={c.on ?? ''}
              placeholder="+next"
              oninput={(e) => set(i, { on: e.currentTarget.value })}
              aria-label="Filter of {called(c, i)} when on"
            />
          </label>
          <label>
            When off
            <input
              class="mono"
              value={c.off ?? ''}
              placeholder="nothing"
              oninput={(e) => set(i, { off: e.currentTarget.value })}
              aria-label="Filter of {called(c, i)} when off"
            />
          </label>
        </div>
      {/if}
      <div class="value">
        <span>Starts as</span>
        <ControlField control={c} onchange={(v) => set(i, v)} />
      </div>
      <span class="dim small">Use it in a filter as <code>{usage(c)}</code>.</span>
    </div>
  {/each}
  <div class="foot">
    <span class="dim">Add</span>
    {#each CONTROL_TYPES as t (t.id)}
      <button type="button" aria-label="Add a control: {t.label}" onclick={() => add(t.id)}
        ><Plus size={14} /> {t.label}</button
      >
    {/each}
  </div>
  <span class="dim small">
    Any filter on the board can use a control: <code>due.after:$daterange.start</code>,
    <code>urgency.above:$number</code>,
    <code>project:$select</code> (what is chosen, the term written for each value and joined:
    <code>( project:A or project:B )</code>; an <code>or</code> is put in parentheses, any other join such as
    <code>|</code> joins the values in the one term), <code>$boolean</code> (its text for on or off). A selection can
    also be typed as an expression, <code>a and (b or c)</code>, and <code>$select.asNative</code> is short for the term
    its source writes (<code>project:$select</code>, <code>+$tags</code>). A selection with nothing chosen, or an empty
    text, picks every task.
  </span>
</fieldset>

<style>
  .controls {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px 10px;
    margin: 0;
    display: grid;
    gap: 8px;
  }
  legend {
    font-size: 12px;
    color: var(--dim);
    padding: 0 4px;
  }
  .control {
    display: grid;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .head,
  .settings,
  .value {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: end;
  }
  .value {
    align-items: center;
    font-size: 12px;
    color: var(--dim);
  }
  .join {
    width: 8ch;
  }
  .kind {
    font-size: 12px;
    font-weight: 600;
    align-self: center;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--dim);
  }
  label.check {
    flex-direction: row;
    align-items: center;
  }
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .small {
    font-size: 12px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    padding: 1px 5px;
    border-radius: 4px;
  }
</style>

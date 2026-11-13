<script lang="ts">
  import { choicesOf, controlLabel, valueOf, type Control, type ControlValue } from './boards';
  import DateField from './DateField.svelte';
  import { store } from './store.svelte';

  /**
   * One control of a board, as it is set above it: a date range's two dates, a date, a number, a least and a most, a selection
   * of the values its source has (one, or several), or a switch. What is set goes to `onchange`; the control itself is not
   * changed here. A selection asks for its choices when it is drawn and when a task changes.
   */
  let {
    control,
    onchange,
    invalid = false,
  }: { control: Control; onchange: (value: ControlValue) => void; invalid?: boolean } = $props();

  const label = $derived(controlLabel(control));
  const set = (patch: ControlValue) => onchange({ ...valueOf(control), ...patch });

  let choices = $state<string[]>([]);
  // The source and what counts as in use decide the choices; a change to a task can add one.
  const asked = $derived(
    control.type === 'select' ? JSON.stringify([control.source, !!control.completed, control.values ?? []]) : '',
  );
  $effect(() => {
    void asked;
    void store.rev;
    const c = control;
    if (c.type !== 'select') return;
    let live = true;
    choicesOf(c, store.config?.config.udas?.[c.source]?.values)
      .then((got) => live && (choices = got))
      .catch(() => live && (choices = c.values ?? []));
    return () => {
      live = false;
    };
  });

  function toggle(value: string, on: boolean) {
    if (control.type !== 'select') return;
    const was = control.values ?? [];
    set({ values: on ? [...was.filter((v) => v !== value), value] : was.filter((v) => v !== value) });
  }
</script>

{#if control.type === 'daterange'}
  <DateField bind:value={() => control.start, (v) => set({ start: v })} label="Start of {label}" {invalid} />
  <span class="dim">–</span>
  <DateField bind:value={() => control.end, (v) => set({ end: v })} label="End of {label}" {invalid} />
{:else if control.type === 'date'}
  <DateField bind:value={() => control.value, (v) => set({ value: v })} {label} {invalid} />
{:else if control.type === 'number'}
  <input
    class="num mono"
    class:bad={invalid}
    inputmode="decimal"
    value={control.value}
    aria-label={label}
    oninput={(e) => set({ value: e.currentTarget.value })}
  />
{:else if control.type === 'numrange'}
  <input
    class="num mono"
    class:bad={invalid}
    inputmode="decimal"
    value={control.min}
    aria-label="Least of {label}"
    oninput={(e) => set({ min: e.currentTarget.value })}
  />
  <span class="dim">–</span>
  <input
    class="num mono"
    class:bad={invalid}
    inputmode="decimal"
    value={control.max}
    aria-label="Most of {label}"
    oninput={(e) => set({ max: e.currentTarget.value })}
  />
{:else if control.type === 'select' && !control.multiple}
  <select
    value={control.values?.[0] ?? ''}
    aria-label={label}
    onchange={(e) => set({ values: e.currentTarget.value ? [e.currentTarget.value] : [] })}
  >
    <option value="">Any</option>
    {#each choices as c (c)}<option value={c}>{c}</option>{/each}
  </select>
{:else if control.type === 'select'}
  {@const picked = control.values ?? []}
  <details class="multi">
    <summary aria-label="{label}: {picked.length ? picked.join(', ') : 'any'}">
      {picked.length ? picked.join(', ') : 'Any'}
    </summary>
    <div class="choices" role="group" aria-label={label}>
      {#each choices as c (c)}
        <label
          ><input type="checkbox" checked={picked.includes(c)} onchange={(e) => toggle(c, e.currentTarget.checked)} />
          {c}</label
        >
      {:else}
        <span class="dim">Nothing to choose yet.</span>
      {/each}
    </div>
  </details>
{:else if control.type === 'boolean'}
  <input
    type="checkbox"
    class="switch"
    checked={!!control.value}
    aria-label={label}
    onchange={(e) => set({ value: e.currentTarget.checked })}
  />
{/if}

<style>
  .num {
    width: 7ch;
  }
  .bad {
    border-color: var(--err);
  }
  .multi {
    position: relative;
  }
  .multi summary {
    cursor: pointer;
    max-width: 24ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-size: 13px;
  }
  .choices {
    position: absolute;
    z-index: 5;
    margin-top: 4px;
    min-width: 16ch;
    max-height: 18em;
    overflow: auto;
    display: grid;
    gap: 4px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-size: 13px;
  }
  .choices label {
    display: flex;
    gap: 6px;
    align-items: center;
    white-space: nowrap;
  }
</style>

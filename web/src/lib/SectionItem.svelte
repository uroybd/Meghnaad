<script lang="ts">
  import type { Snippet } from 'svelte';
  import ColorPicker from './ColorPicker.svelte';
  import FilterInput from './FilterInput.svelte';
  import { ArrowDown, ArrowUp, Trash2 } from './icons';

  /** What a board's written group and a chart's series have in common: a label, the tasks of a filter, a colour. */
  interface Item {
    label: string;
    filter?: string;
    color?: string;
    ignoreSectionFilter?: boolean;
    ignoreBoardFilter?: boolean;
  }

  let {
    what,
    i,
    count,
    item,
    filterId,
    placeholder,
    earlier,
    later,
    removable = true,
    onpatch,
    onmove,
    onremove,
    children,
  }: {
    /** `group` or `series`, for the names of its fields and buttons. */
    what: string;
    /** Its place in the list, from 0, of `count`. */
    i: number;
    count: number;
    item: Item;
    filterId: string;
    placeholder: string;
    /** What moving it earlier and later does. */
    earlier: string;
    later: string;
    removable?: boolean;
    onpatch: (patch: Partial<Item>) => void;
    /** Move it by one place: -1 earlier, 1 later. */
    onmove: (by: -1 | 1) => void;
    onremove: () => void;
    /** More of its settings, under the label and the filter. */
    children?: Snippet;
  } = $props();
</script>

<div class="series-item">
  <div class="row-series">
    <input
      class="label"
      aria-label="Label of {what} {i + 1}"
      placeholder="Label"
      value={item.label}
      oninput={(e) => onpatch({ label: e.currentTarget.value })}
    />
    <div class="grow">
      <FilterInput
        bind:value={() => item.filter ?? '', (v) => onpatch({ filter: v || undefined })}
        id={filterId}
        {placeholder}
      />
    </div>
  </div>
  {@render children?.()}
  <ColorPicker
    value={item.color}
    position={i}
    label="Colour of {what} {i + 1}"
    onchange={(color) => onpatch({ color })}
  />
  <div class="row-series">
    <label class="check">
      <input
        type="checkbox"
        checked={!!item.ignoreSectionFilter}
        onchange={(e) => onpatch({ ignoreSectionFilter: e.currentTarget.checked || undefined })}
      />
      Without the chart's filter
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={!!item.ignoreBoardFilter}
        onchange={(e) => onpatch({ ignoreBoardFilter: e.currentTarget.checked || undefined })}
      />
      Without the board's filter
    </label>
  </div>
  <div class="row-series">
    <button aria-label="Move {what} {i + 1} up" title={earlier} disabled={i === 0} onclick={() => onmove(-1)}
      ><ArrowUp size={14} /></button
    >
    <button aria-label="Move {what} {i + 1} down" title={later} disabled={i === count - 1} onclick={() => onmove(1)}
      ><ArrowDown size={14} /></button
    >
    <button class="danger" aria-label="Remove {what} {i + 1}" disabled={!removable} onclick={onremove}
      ><Trash2 size={14} /> Remove {what}</button
    >
  </div>
</div>

<style>
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--dim);
  }
  label input {
    font-size: 14px;
    color: var(--text);
  }
  .row-series {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row-series .label {
    width: 130px;
    flex: none;
    font-size: 14px;
    color: var(--text);
  }
  .series-item {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--line);
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
</style>

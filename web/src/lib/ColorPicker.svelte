<script lang="ts">
  import { CHART_COLOR_NAMES, chartColor, defaultSeriesColor, defaultSeriesName, namedColorHex } from './colors';
  import { scheme } from './scheme.svelte';

  /**
   * A colour for a chart series: Taskwarrior's basic colours by name, drawn in the page's palette (so a choice follows a light
   * and a dark page), the colour the series has when it names none, and any other colour from the system's picker.
   */
  let {
    value,
    position,
    label,
    onchange,
  }: {
    /** A colour name, `#rrggbb`, or nothing for the default. */
    value: string | undefined;
    /** The series' place, which decides its default colour. */
    position: number;
    label: string;
    onchange: (value: string | undefined) => void;
  } = $props();

  const dark = $derived(scheme.dark);
  const norm = (v: string | undefined) => v?.trim().toLowerCase();
  const shown = $derived(chartColor(value, dark) ?? defaultSeriesColor(position, dark));
  const custom = $derived(!!value && value.startsWith('#'));
</script>

<div class="picker" role="group" aria-label={label}>
  <button
    type="button"
    class="sw default"
    class:on={!value}
    aria-pressed={!value}
    aria-label="Default colour ({defaultSeriesName(position)})"
    title="Default: {defaultSeriesName(position)}"
    style:background={defaultSeriesColor(position, dark)}
    onclick={() => onchange(undefined)}
  ></button>
  {#each CHART_COLOR_NAMES as name (name)}
    <button
      type="button"
      class="sw"
      class:on={norm(value) === name}
      aria-pressed={norm(value) === name}
      aria-label={name}
      title={name}
      style:background={namedColorHex(name, dark)}
      onclick={() => onchange(name)}
    ></button>
  {/each}
  <input
    type="color"
    class="any"
    class:on={custom}
    aria-label="Any other colour"
    title="Any other colour"
    value={shown.length === 7 ? shown : '#808080'}
    oninput={(e) => onchange(e.currentTarget.value)}
  />
</div>

<style>
  .picker {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .sw {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 4px;
  }
  /* The default is a swatch with a dashed edge: it follows the series' place, not a colour of its own. */
  .default {
    border-style: dashed;
    border-color: var(--dim);
  }
  .sw.on {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .any {
    width: 26px;
    height: 22px;
    padding: 0;
    border-radius: 4px;
  }
  .any.on {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>

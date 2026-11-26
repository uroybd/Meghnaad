<script lang="ts">
  import { choiceColor } from './colors';
  import { scheme } from './scheme.svelte';
  import { store } from './store.svelte';

  /**
   * A UDA's value that is one of its choices (spec 0034): a pill in the value's own colour, the same wherever it is
   * shown; grey for a value not among the choices.
   */
  let { name, value, choice }: { name: string; value: string; choice: number } = $props();
  const hue = $derived(choiceColor(name.split('.')[0], value, choice, store.config?.colors, scheme.dark));
</script>

<span class="choice" class:other={hue == null} style={hue ? `--hue:${hue}` : undefined}>{value}</span>

<style>
  .choice {
    display: inline-block;
    padding: 0 8px;
    border: 1px solid color-mix(in srgb, var(--hue) 45%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--hue) 16%, transparent);
    font-size: 13px;
    line-height: 22px;
    white-space: nowrap;
  }
  .other {
    border-color: var(--line);
    background: var(--panel-2);
    color: var(--dim);
  }
</style>

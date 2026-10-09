<script lang="ts">
  import { css, look } from './colors';
  import { scheme } from './scheme.svelte';
  import type { StyledResult } from './types';

  /** Coloured text: the `colors` palette, a sample or the legend, and the history graph. */
  let { result }: { result: StyledResult } = $props();
</script>

<pre class="styled mono">{#each result.lines as line, i (i)}<div>{#each line as span, j (j)}<span
          style={css(look(span.style, scheme.dark, undefined, !!result.swatch))}>{span.text}</span
        >{/each}{#if line.length === 0}&nbsp;{/if}</div>{/each}</pre>

<style>
  .styled {
    margin: 4px 0;
    white-space: pre;
    overflow-x: auto;
    line-height: 1.3;
    font-size: 13px;
  }
  .styled div {
    min-height: 1.3em;
  }
</style>

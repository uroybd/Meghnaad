<script lang="ts">
  import type { Shape } from './lucide';

  /** Draws an icon from its shapes (see `lucide.ts`: only a short list of shapes is ever here). */
  let { shapes, size = 15 }: { shapes: Shape[]; size?: number } = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each shapes as [tag, attrs], i (i)}
    <svelte:element this={tag} {...attrs} />
  {/each}
</svg>

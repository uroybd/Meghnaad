<script lang="ts">
  /**
   * Grey bars where content is on its way, so the page keeps its shape while it loads. Plain CSS; still for people
   * who ask for less motion.
   */
  let { rows = 6, label = 'Loading…' }: { rows?: number; label?: string } = $props();
  // Bars of a few lengths, so they read as lines of text.
  const widths = [92, 78, 85, 64, 88, 72, 80, 58];
</script>

<div class="skeleton" role="status" aria-label={label} aria-busy="true">
  {#each { length: rows }, i (i)}
    <span style:width="{widths[i % widths.length]}%"></span>
  {/each}
</div>

<style>
  .skeleton {
    display: grid;
    gap: 10px;
    padding: 8px 0;
  }
  span {
    display: block;
    height: 12px;
    border-radius: 4px;
    background: var(--line);
    animation: pulse 1.2s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.45;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    span {
      animation: none;
    }
  }
</style>

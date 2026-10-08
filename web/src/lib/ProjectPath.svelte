<script lang="ts">
  /** A project path with each part in its own colour and the dots between them muted. */
  let { segments }: { segments: { text: string; hue: number }[] } = $props();
</script>

<span class="path">{#each segments as seg, i}{#if i > 0}<span class="dot">.</span>{/if}<span class="seg" style="--h: {seg.hue}">{seg.text}</span>{/each}</span>

<style>
  .seg { color: hsl(var(--h) var(--seg-s) var(--seg-l)); font-weight: 500; }
  .dot { color: var(--dim); opacity: 0.55; padding: 0 1px; }
</style>

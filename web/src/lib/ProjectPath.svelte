<script lang="ts">
  /** A project path, `Home.Kitchen`, with the dots between its parts muted. Colour comes from `color.project.*`. */
  let { segments }: { segments: { text: string }[] } = $props();
</script>

<span class="path"
  >{#each segments as seg, i (i)}{#if i > 0}<span class="dot">.</span>{/if}<span>{seg.text}</span>{/each}</span
>

<style>
  .dot {
    color: var(--dim);
    opacity: 0.55;
    padding: 0 1px;
  }
</style>

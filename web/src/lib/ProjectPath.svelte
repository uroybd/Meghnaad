<script lang="ts">
  /**
   * A project path, `Home.Kitchen`, with the dots between its parts muted. Colour comes from `color.project.*`.
   * With `onpick` each part is a button that names the path up to and including it (`Home`, then `Home.Kitchen`).
   */
  let { segments, onpick }: { segments: { text: string }[]; onpick?: (path: string) => void } = $props();

  const upTo = (i: number) =>
    segments
      .slice(0, i + 1)
      .map((s) => s.text)
      .join('.');
</script>

<span class="path"
  >{#each segments as seg, i (i)}{#if i > 0}<span class="dot">.</span>{/if}{#if onpick}<button
        type="button"
        class="ghost part"
        title="Show {upTo(i)} on the Projects page"
        onclick={() => onpick(upTo(i))}>{seg.text}</button
      >{:else}<span>{seg.text}</span>{/if}{/each}</span
>

<style>
  .dot {
    color: var(--dim);
    opacity: 0.55;
    padding: 0 1px;
  }
  .part {
    padding: 0;
    font: inherit;
    color: inherit;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
  }
</style>

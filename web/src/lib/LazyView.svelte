<script lang="ts">
  // Draws a component fetched on demand (`lazy`): nothing until its code is here, with the page's waiting line meanwhile
  // (spec 0023, step 4), and a way out when it can't be fetched.
  import type { Lazy } from './lazy.svelte';
  import { waitFor } from './waiting.svelte';

  let { of, props = {} }: { of: Lazy; props?: Record<string, unknown> } = $props();

  $effect(() => {
    if (!of.component && !of.failed) void waitFor(of.load());
  });
</script>

{#if of.component}
  {@const Shown = of.component}
  <Shown {...props} />
{:else if of.failed}
  <p class="failed" role="alert">
    This part of the app could not be loaded: you may be offline, or the app was updated.
    <button onclick={() => location.reload()}>Reload</button>
    <button class="ghost" onclick={() => void waitFor(of.load())}>Try again</button>
  </p>
{/if}

<style>
  .failed {
    padding: 24px 16px;
    color: var(--muted, inherit);
  }
</style>

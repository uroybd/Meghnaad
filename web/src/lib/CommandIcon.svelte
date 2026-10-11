<script lang="ts">
  import { Terminal } from './icons';
  import { bundledIcons } from './lucide';
  import { lucide } from './lucideLib.svelte';
  import NodeIcon from './NodeIcon.svelte';

  /** A task command's icon: the Lucide icon of that name, or a terminal while it loads or when there is none. */
  let { name, size = 15 }: { name?: string; size?: number } = $props();

  const bundled = $derived(name ? bundledIcons.get(name) : undefined);
  const shapes = $derived(name ? lucide.shapes[name] : undefined);

  $effect(() => {
    if (name && !bundled) lucide.request(name);
  });
</script>

{#if bundled}
  {@const Icon = bundled}
  <Icon {size} />
{:else if shapes}
  <NodeIcon {shapes} {size} />
{:else}
  <Terminal {size} />
{/if}

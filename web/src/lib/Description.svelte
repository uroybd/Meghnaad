<script lang="ts">
  // A task's description as a title: as it was typed, unless one of the user's transformations changed it
  // (`appearance.transformations`), when it is drawn as the Markdown that made. A one-paragraph result stays inline.
  import MarkdownNodes from './MarkdownNodes.svelte';
  import { trees } from './markdown';
  import { store } from './store.svelte';

  // `uuid`: the task's, for a transformation with a condition.
  let { text, uuid }: { text: string; uuid?: string } = $props();

  const tree = $derived(trees([text], ['description'], store.rules, [uuid ?? null])[0]);
  const nodes = $derived(tree?.length === 1 && tree[0].t === 'p' ? (tree[0].c ?? []) : tree);
</script>

{#if nodes}<span class="md-desc"><MarkdownNodes {nodes} /></span>{:else}{text}{/if}

<style>
  .md-desc :global(p) {
    margin: 0;
  }
</style>

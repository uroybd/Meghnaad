<script lang="ts">
  import { store } from './store.svelte';

  /**
   * A tag as a chip you can click. With `onclick` (the live table in the Tasks view) it toggles the tag in that
   * report's filter, and `active` says it is in it. Without, it opens the tag's entry on the Tags page.
   */
  let { tag, active = false, onclick }: { tag: string; active?: boolean; onclick?: (tag: string) => void } = $props();

  const title = $derived(
    onclick ? (active ? `Remove +${tag} from the filter` : `Filter by +${tag}`) : `Show +${tag} on the Tags page`,
  );
  function go(e: MouseEvent) {
    e.stopPropagation();
    if (onclick) onclick(tag);
    else store.showTag(tag);
  }
</script>

<button type="button" class="tagpill" class:on={active} aria-pressed={onclick ? active : undefined} {title} onclick={go}
  >{tag}</button
>

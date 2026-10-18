<script lang="ts">
  import { NO_CONTEXT, type Scope } from './pageContext';
  import { store } from './store.svelte';

  /**
   * The context one page shows its tasks in. The active context (`context work` in the console) is the console's; a page
   * follows it unless it is told otherwise here, and then keeps its own choice, so a page never changes under another's.
   */
  let { scope }: { scope: Scope } = $props();

  const contexts = $derived(store.config?.config.contexts ?? {});
  const names = $derived(Object.keys(contexts).sort());
  const choice = $derived(store.contextChoice(scope) ?? '');
  const inForce = $derived(store.contextIn(scope));
  const read = $derived(inForce ? contexts[inForce]?.read : null);
  const id = $derived(`context-${scope.replace(/[^a-z0-9]/gi, '-')}`);
</script>

{#if names.length || choice}
  <label class="sr-only" for={id}>Context of this page</label>
  <select
    {id}
    class="context"
    class:own={!!choice}
    value={choice}
    title={inForce ? `Context ${inForce}${read ? `: ${read}` : ''}` : 'No context: every task'}
    onchange={(e) => store.setContextChoice(scope, e.currentTarget.value || undefined)}
  >
    <option value="">Context: {store.activeContext ?? 'none'} (same as console)</option>
    <option value={NO_CONTEXT}>Context: none</option>
    {#each names as n (n)}<option value={n}>Context: {n}</option>{/each}
  </select>
{/if}

<style>
  .context {
    max-width: 17em;
  }
  .own {
    border-color: var(--accent);
  }
  @media (max-width: 760px) {
    .context {
      max-width: 100%;
      width: 100%;
    }
  }
</style>

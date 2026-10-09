<script lang="ts">
  import { store, type Entry } from './store.svelte';
  import type { ConfirmResult } from './types';

  /**
   * Taskwarrior's questions, answered on the page. One question is a yes/no. Several (a change to
   * `bulk` tasks or more, follow-ups about a dependency chain or a recurring series) are a table
   * with a tick per task: ticking all is its "all", ticking none is its "quit".
   */
  let { entry, result }: { entry: Entry; result: ConfirmResult } = $props();

  // You typed the command, so yes is the usual answer: everything starts ticked, untick the exceptions.
  // (The page re-creates this for every new question, so reading the prop once is what is wanted.)
  // svelte-ignore state_referenced_locally
  let ticked = $state(new Set(result.items.map((i) => i.key)));
  const all = $derived(ticked.size === result.items.length);

  function toggle(key: string) {
    const next = new Set(ticked);
    if (!next.delete(key)) next.add(key);
    ticked = next;
  }
  const setAll = (on: boolean) => (ticked = new Set(on ? result.items.map((i) => i.key) : []));
  const apply = () => store.answerItems(entry, result.items.map((i) => i.key).filter((k) => ticked.has(k)));
  const showQuestion = $derived(result.ask === 'extras');
</script>

{#if result.ask === 'plain' || result.items.length === 1}
  <div class="confirm" role="alert">
    <span>{result.message}</span>
    {#if result.ask === 'plain'}
      <button class="primary" onclick={() => store.confirm(entry, true)}>Yes</button>
      <button onclick={() => store.confirm(entry, false)}>No</button>
    {:else}
      <button class="primary" onclick={() => store.answerItems(entry, [result.items[0].key])}>Yes</button>
      <button onclick={() => store.answerItems(entry, [])}>No</button>
    {/if}
  </div>
{:else}
  <div class="ask" role="alert">
    <p class="head">{result.message}</p>
    <div class="scroll">
      <table>
        <thead>
          <tr>
            <th class="tick">
              <input type="checkbox" checked={all} indeterminate={!all && ticked.size > 0} aria-label="Select all or none" onchange={(e) => setAll(e.currentTarget.checked)} />
            </th>
            <th>ID</th>
            <th>Task</th>
            {#if showQuestion}<th>Question</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each result.items as i (i.key)}
            <tr class:off={!ticked.has(i.key)}>
              <td class="tick"><input type="checkbox" checked={ticked.has(i.key)} aria-label="Yes: {i.question}" onchange={() => toggle(i.key)} /></td>
              <td class="id">{i.id ?? i.uuid.slice(0, 8)}</td>
              <td>{i.description}</td>
              {#if showQuestion}<td class="dim">{i.question}</td>{/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="row">
      <button class="ghost" onclick={() => setAll(true)}>Select all</button>
      <button class="ghost" onclick={() => setAll(false)}>Select none</button>
      <span class="grow"></span>
      <button class="ghost" onclick={() => store.confirm(entry, false)}>Cancel</button>
      <button class="primary" onclick={apply}>Go ahead with {ticked.size} of {result.items.length}</button>
    </div>
  </div>
{/if}

<style>
  .confirm { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; padding: 6px 0; }
  .ask { padding: 6px 0; max-width: 760px; }
  .head { margin: 0 0 6px; }
  .scroll { overflow-x: auto; }
  table { border-collapse: collapse; width: 100%; }
  th, td { text-align: left; padding: 4px 12px 4px 0; vertical-align: top; }
  th { color: var(--dim); font-weight: 500; font-size: 12px; border-bottom: 1px solid var(--line); }
  tbody td { border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent); }
  .tick { width: 1%; padding-left: 4px; }
  .id { font-weight: 700; font-variant-numeric: tabular-nums; }
  tr.off td { color: var(--dim); text-decoration: line-through; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 8px; }
  .grow { flex: 1; }
</style>

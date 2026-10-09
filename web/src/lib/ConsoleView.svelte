<script lang="ts">
  import { tick } from 'svelte';
  import ConsoleInput from './ConsoleInput.svelte';
  import { Eraser } from './icons';
  import ResultView from './ResultView.svelte';
  import { store } from './store.svelte';

  let end: HTMLElement;

  // Keep the newest output in view.
  $effect(() => {
    store.entries.length;
    store.entries.at(-1)?.at;
    tick().then(() => end?.scrollIntoView({ block: 'end' }));
  });
</script>

<div class="console">
<div class="out">
{#if store.entries.length === 0}
  <div class="empty dim">
    <p>Type a command below, like <code>task next</code> or <code>task add Buy milk due:tomorrow</code>.</p>
    <p>Press <kbd>/</kbd> to focus the prompt, <kbd>Tab</kbd> to complete, <kbd>↑</kbd> for history, <code>help</code> for commands.</p>
  </div>
{:else}
  <div class="toolbar"><button class="ghost btn" onclick={() => store.clear()} title="Clear the console (Ctrl+L)"><Eraser size={14} /> clear</button></div>
  {#each store.entries as e (e.id)}
    <section class="entry">
      <div class="cmd mono"><span class="dim">$</span> {e.title.replace(/^task /, 'task ')}</div>
      <ResultView entry={e} onedit={(row, from) => (store.editing = { row, from })} />
    </section>
  {/each}
{/if}
<div bind:this={end}></div>
</div>
<ConsoleInput autofocus />
</div>

<style>
  .console { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .out { flex: 1; min-height: 0; overflow: auto; padding: 24px 36px 16px; }
  @media (max-width: 760px) {
    .out { padding: 8px max(10px, env(safe-area-inset-right)) 72px max(10px, env(safe-area-inset-left)); }
  }
  .entry { padding: 12px 0 16px; border-bottom: 1px dashed var(--line); }
  .cmd { font-weight: 600; margin-bottom: 2px; overflow-wrap: anywhere; }
  .toolbar { text-align: right; }
  .btn { display: inline-flex; align-items: center; gap: 5px; }
  .empty { padding: 24px 4px; }
  code, kbd { font-family: var(--mono); background: var(--panel-2); border-radius: 4px; padding: 0 5px; }
</style>

<script lang="ts">
  import { formatSeconds } from './dates';
  import { Square, Timer } from './icons';
  import { notifier } from './notifier.svelte';
  import { store } from './store.svelte';

  let now = $state(Math.floor(Date.now() / 1000));

  $effect(() => {
    if (notifier.active.length === 0) return;
    const t = setInterval(() => (now = Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  });

  const first = $derived(notifier.active[0]);
  // With journal.time the server has the tracked total; otherwise it's the current stretch.
  const elapsed = $derived(
    first
      ? first.active_seconds != null
        ? first.active_seconds + Math.max(0, now - notifier.polledAt)
        : Math.max(0, now - (first.start ?? now))
      : 0,
  );

  // The tab title shows it too, so it's visible from another tab.
  $effect(() => {
    document.title = first ? `● ${formatSeconds(elapsed)} · ${first.description}` : 'Meghnaad';
  });

  async function stop() {
    if (!first) return;
    await store.act(null, [first.uuid, 'stop']);
    await notifier.poll();
  }
</script>

{#if first}
  <span class="timer" role="timer" aria-label="Task in progress">
    <button class="ghost body" onclick={() => store.openDetail(first.uuid)} title="Open {first.description}">
      <span class="dot" aria-hidden="true"><Timer size={15} /></span>
      <span class="what">{first.description}</span>
      <span class="mono t" data-testid="timer">{formatSeconds(elapsed)}</span>
      {#if notifier.active.length > 1}<span class="chip">+{notifier.active.length - 1}</span>{/if}
    </button>
    <button class="ghost" aria-label="Stop {first.description}" title="Stop" onclick={stop}><Square size={13} fill="currentColor" /></button>
  </span>
{/if}

<style>
  .timer button { line-height: 0; }
  .timer .body { line-height: 1.3; }
  /* It gives way first when the header is tight; the task name inside is cut with an ellipsis. */
  .timer { display: inline-flex; align-items: center; border: 1px solid var(--ok); border-radius: 999px; padding: 0 4px; max-width: 26em; min-width: 0; flex: 0 1 auto; }
  .body { display: inline-flex; gap: 6px; align-items: center; min-width: 0; padding: 0 4px; }
  .what { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 14em; }
  .dot { display: inline-flex; color: var(--ok); animation: pulse 2s ease-in-out infinite; }
  .t { font-variant-numeric: tabular-nums; }
  @keyframes pulse { 50% { opacity: 0.35; } }
  @media (prefers-reduced-motion: reduce) { .dot { animation: none; } }
</style>

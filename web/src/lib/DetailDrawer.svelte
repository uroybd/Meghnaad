<script lang="ts">
  import { runCli } from './api';
  import { formatSeconds } from './dates';
  import { Check, Pencil, Play, RotateCcw, Square, Timer, Trash2, X } from './icons';
  import TaskInfo from './TaskInfo.svelte';
  import { store } from './store.svelte';
  import type { Row } from './types';

  let { uuid }: { uuid: string } = $props();

  let task = $state<Row | null>(null);
  let message = $state<string | null>(null);
  let note = $state('');
  let confirmDelete = $state(false);
  let fetchedAt = $state(Math.floor(Date.now() / 1000));
  let tick = $state(Math.floor(Date.now() / 1000));
  let el: HTMLElement;

  async function load() {
    try {
      const res = await runCli({ args: [uuid, 'info'] });
      const r = res.result;
      if (r.kind === 'info') {
        task = r.tasks[0] ?? null;
        message = null;
        fetchedAt = Math.floor(Date.now() / 1000);
      } else if (r.kind === 'error') {
        message = r.message;
      } else {
        message = 'This task is no longer here.';
        task = null;
      }
    } catch (e) {
      message = e instanceof Error ? e.message : String(e);
    }
  }

  // Load on open, and again after any write (this drawer's own, the table's, or the console's).
  $effect(() => {
    uuid;
    store.rev;
    load();
  });

  // While a task is running, keep its tracked time ticking between fetches.
  $effect(() => {
    if (task?.start == null) return;
    const t = setInterval(() => (tick = Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  });

  const live = $derived(
    task && task.active_seconds != null && task.start != null
      ? {
          ...task,
          active_seconds: task.active_seconds + Math.max(0, tick - fetchedAt),
          // Only the running session (no end) grows between fetches.
          sessions: task.sessions.map((s) => (s.end == null ? { ...s, seconds: s.seconds + Math.max(0, tick - fetchedAt) } : s)),
        }
      : task,
  );

  const act = (...args: string[]) => store.act(store.detail?.from ?? null, [uuid, ...args]);

  async function annotate(e: Event) {
    e.preventDefault();
    if (!note.trim()) return;
    const res = await act('annotate', note.trim());
    if (res && res.result.kind !== 'error') note = '';
  }

  async function del() {
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 3000);
      return;
    }
    const res = await act('delete');
    if (res && res.result.kind !== 'error') store.detail = null;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !store.editing) store.detail = null;
  }

  $effect(() => {
    el.focus({ preventScroll: true });
  });
</script>

<svelte:window {onkeydown} />

<aside class="drawer" bind:this={el} tabindex="-1" aria-label="Task details" data-testid="drawer">
  <header class="row">
    <strong class="grow title">{task?.description ?? 'Task'}</strong>
    <button class="ghost" aria-label="Close details" onclick={() => (store.detail = null)}><X size={18} /></button>
  </header>

  {#if message}<p class="err" role="alert">{message}</p>{/if}

  {#if live}
    <div class="actions row">
      {#if live.status === 'pending'}
        <button class="primary btn" onclick={() => act('done')}><Check size={15} /> Done</button>
        {#if live.start != null}
          <button class="btn" onclick={() => act('stop')}><Square size={13} fill="currentColor" /> Stop</button>
        {:else}
          <button class="btn" onclick={() => act('start')}><Play size={13} /> Start</button>
        {/if}
      {:else if live.status !== 'deleted'}
        <button class="btn" onclick={() => act('start')} title="Start it again (reopens the task)"><RotateCcw size={14} /> Reopen &amp; start</button>
      {/if}
      <button class="btn" onclick={() => (store.editing = { row: live, from: store.detail?.from ?? null })}><Pencil size={14} /> Edit</button>
      {#if live.status !== 'deleted'}
        <button class="danger btn" onclick={del}>{#if confirmDelete}sure?{:else}<Trash2 size={14} /> Delete{/if}</button>
      {/if}
    </div>

    {#if live.start != null && live.active_seconds != null}
      <p class="running" role="timer"><Timer size={15} /> running · {formatSeconds(live.active_seconds)} tracked</p>
    {/if}

    <TaskInfo task={live} embedded onopen={(u) => store.openDetail(u, store.detail?.from ?? null)} />

    <form class="note row" onsubmit={annotate}>
      <input class="grow" bind:value={note} placeholder="Add an annotation…" aria-label="New annotation" />
      <button disabled={!note.trim()}>Add</button>
    </form>
  {:else if !message}
    <p class="dim">Loading…</p>
  {/if}
</aside>

<style>
  .drawer {
    position: fixed; z-index: 15; top: 0; right: 0; bottom: 0; width: min(440px, 100vw);
    background: var(--panel); border-left: 1px solid var(--line); padding: 14px 16px;
    overflow: auto; box-shadow: -8px 0 24px rgb(0 0 0 / 0.12); outline: none;
  }
  .title { font-size: 16px; overflow-wrap: anywhere; }
  .actions { flex-wrap: wrap; margin: 10px 0; }
  .running { display: flex; align-items: center; gap: 6px; color: var(--ok); font-weight: 600; margin: 4px 0; }
  .btn { display: inline-flex; align-items: center; gap: 5px; }
  .note { margin-top: 12px; }
</style>

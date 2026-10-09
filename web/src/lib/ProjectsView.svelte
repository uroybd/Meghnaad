<script lang="ts">
  import { onMount } from 'svelte';
  import { runCli } from './api';
  import { formatFor } from './dateformat';
  import { formatMoment } from './dates';
  import { Check, ChevronDown, ChevronRight, Folder } from './icons';
  import { buildTree, expandable, type ProjectNode } from './projects';
  import { store } from './store.svelte';
  import type { Row } from './types';

  let rows = $state<Row[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let open = $state<Set<string>>(new Set());
  let seeded = false;

  const tree = $derived(buildTree(rows, store.now));
  const settings = $derived(store.config?.config.settings);
  const fmt = $derived(formatFor('report', settings));

  async function load() {
    try {
      const res = await runCli({ args: ['status:pending', 'export'] });
      const r = res.result;
      if (r.kind === 'json') rows = r.value as Row[];
      else if (r.kind === 'error') throw new Error(r.message);
      error = null;
      // First load: open the main projects so their sub-projects show grouped underneath.
      if (!seeded) {
        seeded = true;
        open = new Set(tree.filter((n) => n.children.length).map((n) => n.name));
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // Reload after any change made elsewhere (done, edit, add, console...).
  let lastRev = store.rev;
  $effect(() => {
    if (store.rev !== lastRev) {
      lastRev = store.rev;
      void load();
    }
  });

  function toggle(name: string) {
    const next = new Set(open);
    if (!next.delete(name)) next.add(name);
    open = next;
  }

  const expandAll = () => (open = new Set(expandable(tree)));
  const collapseAll = () => (open = new Set());

  const due = (r: Row) => formatMoment(r.due, undefined, fmt);
  const late = (r: Row) => r.due != null && r.due < store.now;

  function done(r: Row) {
    void store.act(null, [r.uuid, 'done']);
  }

  /** Show this project in the Tasks view, with the full report and filter bar. */
  function inTasks(n: ProjectNode) {
    store.filter = n.name ? `project:${n.name}` : 'project:';
    store.view = 'tasks';
  }
</script>

{#snippet branch(n: ProjectNode, depth: number)}
  {@const isOpen = open.has(n.name)}
  <li>
    <div class="head" style="--depth: {depth}">
      <button
        class="ghost toggle"
        aria-expanded={isOpen}
        aria-label="{isOpen ? 'Collapse' : 'Expand'} {n.label}"
        onclick={() => toggle(n.name)}
      >
        {#if isOpen}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
        <Folder size={15} />
        <span class="label">{n.label}</span>
      </button>
      <span class="count" title="Pending tasks, including sub-projects">{n.total}</span>
      {#if n.children.length}<span class="count"
          >· {n.children.length} sub-project{n.children.length === 1 ? '' : 's'}</span
        >{/if}
      {#if n.overdue}<span class="late" title="Overdue">{n.overdue} overdue</span>{/if}
      <span class="grow"></span>
      <button class="ghost link" onclick={() => inTasks(n)} title="Open {n.name || 'these tasks'} in the Tasks view"
        >Open in Tasks</button
      >
    </div>
    {#if isOpen}
      <ul class="tree">
        {#each n.children as c (c.name)}{@render branch(c, depth + 1)}{/each}
        {#each n.own as r (r.uuid)}
          <li class="task" style="--depth: {depth + 1}">
            <button class="ghost tick" aria-label="Mark done: {r.description}" title="Mark done" onclick={() => done(r)}
              ><Check size={15} /></button
            >
            <button class="ghost desc" onclick={() => store.openDetail(r.uuid)}>
              {#if r.priority}<span class="pri pri-{r.priority}">{r.priority}</span>{/if}
              <span class="text">{r.description}</span>
            </button>
            {#each r.tags as t, _i (_i)}<span class="tag">+{t}</span>{/each}
            {#if r.due != null}<span class="due" class:late={late(r)}>{due(r)}</span>{/if}
          </li>
        {/each}
        {#if n.children.length === 0 && n.own.length === 0}<li class="dim empty" style="--depth: {depth + 1}">
            Nothing here.
          </li>{/if}
      </ul>
    {/if}
  </li>
{/snippet}

<section aria-label="Projects">
  <div class="bar">
    <h2>Projects</h2>
    <span class="grow"></span>
    <button class="ghost" onclick={expandAll} disabled={tree.length === 0}>Expand all</button>
    <button class="ghost" onclick={collapseAll} disabled={open.size === 0}>Collapse all</button>
  </div>
  {#if error}
    <p class="err" role="alert">{error}</p>
  {:else if loading}
    <p class="dim">Loading…</p>
  {:else if tree.length === 0}
    <p class="dim">No pending tasks.</p>
  {:else}
    <ul class="tree root">
      {#each tree as n (n.name)}{@render branch(n, 0)}{/each}
    </ul>
  {/if}
</section>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 10px;
  }
  h2 {
    font-size: 15px;
    margin: 0;
  }
  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .head,
  .task {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0 3px calc(var(--depth) * 20px);
    min-width: 0;
  }
  .head {
    border-bottom: 1px solid var(--line);
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    text-align: left;
    min-width: 0;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    color: var(--dim);
    font-size: 13px;
  }
  .late,
  .due.late {
    color: var(--err);
    font-size: 13px;
  }
  .link {
    font-size: 13px;
    color: var(--accent);
    white-space: nowrap;
  }
  .task {
    padding-left: calc(var(--depth) * 20px + 4px);
  }
  .tick {
    line-height: 0;
    color: var(--dim);
  }
  .tick:hover {
    color: var(--ok);
  }
  .desc {
    display: flex;
    align-items: baseline;
    gap: 6px;
    text-align: left;
    min-width: 0;
    flex: 0 1 auto;
  }
  .text {
    overflow-wrap: anywhere;
  }
  .pri {
    font-size: 11px;
    font-weight: 700;
  }
  .pri-H {
    color: var(--pri-h);
  }
  .pri-M {
    color: var(--pri-m);
  }
  .pri-L {
    color: var(--pri-l);
  }
  .tag,
  .due {
    color: var(--dim);
    font-size: 13px;
    white-space: nowrap;
  }
  .empty {
    padding-left: calc(var(--depth) * 20px + 4px);
  }
  @media (max-width: 760px) {
    .task {
      flex-wrap: wrap;
    }
    .link {
      display: none;
    }
  }
</style>

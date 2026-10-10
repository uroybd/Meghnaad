<script lang="ts">
  import { tick, untrack } from 'svelte';
  import ExportFilter from './ExportFilter.svelte';
  import { ChevronDown, ChevronRight, Folder } from './icons';
  import { buildTree, expandable, type ProjectNode } from './projects';
  import { store } from './store.svelte';
  import TaskLine from './TaskLine.svelte';
  import type { Row } from './types';

  // What the filter selected so far: nothing yet (loading), or tasks, or an error with the last good tasks kept.
  let res = $state<{ rows: Row[]; error: string | null } | null>(null);
  const rows = $derived(res?.rows ?? []);
  const loading = $derived(res === null);
  const error = $derived(res?.error ?? null);
  let open = $state<Set<string>>(new Set());
  let seeded = false;
  /** The project whose entry was just asked for, highlighted for a moment. */
  let flash = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;
  let root: HTMLElement;

  // A project asked for from a chip can have no task the filter matches: it still gets its entry.
  const tree = $derived(buildTree(rows, store.now, store.projectFocus?.project));

  // First load: open the main projects so their sub-projects show grouped underneath.
  $effect(() => {
    if (loading || seeded) return;
    seeded = true;
    untrack(() => (open = new Set(tree.filter((n) => n.children.length).map((n) => n.name))));
  });

  // A project clicked somewhere else: open its entry (and the projects above it), bring it into view and flash it.
  $effect(() => {
    const focus = store.projectFocus;
    if (!focus || loading) return;
    untrack(() => {
      const parts = focus.project.split('.');
      open = new Set([...open, ...parts.map((_, i) => parts.slice(0, i + 1).join('.'))]);
      flash = focus.project;
      clearTimeout(flashTimer);
      flashTimer = setTimeout(() => (flash = null), 1800);
      store.projectFocus = null; // handled: coming back to this page later should not repeat it
    });
    void tick().then(() => {
      const el = [...root.querySelectorAll<HTMLElement>('[data-project]')].find(
        (e) => e.dataset.project === focus.project,
      );
      el?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    });
  });

  function toggle(name: string) {
    const next = new Set(open);
    if (!next.delete(name)) next.add(name);
    open = next;
  }

  const expandAll = () => (open = new Set(expandable(tree)));
  const collapseAll = () => (open = new Set());

  /** Show this project in the Tasks view, with the full report and filter bar. */
  function inTasks(n: ProjectNode) {
    store.filter = n.name ? `project:${n.name}` : 'project:';
    store.view = 'tasks';
  }
</script>

{#snippet branch(n: ProjectNode, depth: number)}
  {@const isOpen = open.has(n.name)}
  <li data-project={n.name} class:flash={flash === n.name}>
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
      <span class="count" title="Tasks matching the filter, including sub-projects">{n.total}</span>
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
        {#each n.own as r (r.uuid)}<TaskLine row={r} depth={depth + 1} />{/each}
        {#if n.children.length === 0 && n.own.length === 0}<li class="dim empty" style="--depth: {depth + 1}">
            Nothing here.
          </li>{/if}
      </ul>
    {/if}
  </li>
{/snippet}

<ExportFilter
  bind:filter={store.projectsFilter}
  onresult={(r) => (res = { rows: r.rows ?? res?.rows ?? [], error: r.error })}
  id="projects-filter"
  label="Projects filters"
  placeholder="Add filters: project:Home +work due.before:eow  (Tab completes)"
/>
<section aria-label="Projects" bind:this={root}>
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
    <p class="dim">No tasks match this filter.</p>
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
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0 3px calc(var(--depth) * 20px);
    min-width: 0;
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
  .late {
    color: var(--err);
    font-size: 13px;
  }
  .link {
    font-size: 13px;
    color: var(--accent);
    white-space: nowrap;
  }
  .empty {
    padding-left: calc(var(--depth) * 20px + 4px);
  }
  /* The entry a project chip asked for. A fading background, and a plain outline when motion is reduced. */
  li.flash > .head {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border-radius: 6px;
    animation: flash 1.8s ease-out forwards;
  }
  @keyframes flash {
    to {
      background: transparent;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    li.flash > .head {
      animation: none;
      outline: 2px solid var(--accent);
    }
  }
  @media (max-width: 760px) {
    .link {
      display: none;
    }
  }
</style>

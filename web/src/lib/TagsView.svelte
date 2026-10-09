<script lang="ts">
  import { tick, untrack } from 'svelte';
  import ExportFilter from './ExportFilter.svelte';
  import { shellQuote } from './cmdline';
  import { ChevronDown, ChevronRight, Tag } from './icons';
  import { store } from './store.svelte';
  import TaskLine from './TaskLine.svelte';
  import { buildTags, type TagEntry } from './tags';
  import type { Row } from './types';

  // What the filter selected so far: nothing yet (loading), or tasks, or an error with the last good tasks kept.
  let res = $state<{ rows: Row[]; error: string | null } | null>(null);
  const rows = $derived(res?.rows ?? []);
  const loading = $derived(res === null);
  const error = $derived(res?.error ?? null);
  let open = $state<Set<string>>(new Set());
  /** The tag whose entry was just asked for, highlighted for a moment. */
  let flash = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;
  let root: HTMLElement;

  // A tag asked for from a chip can have no pending task (a chip from a report of finished tasks): it still gets its entry.
  const tags = $derived(buildTags(rows, store.now, store.tagFocus?.tag));

  // A tag chip clicked somewhere else: open that tag's entry, bring it into view and flash it.
  $effect(() => {
    const focus = store.tagFocus;
    if (!focus || loading) return;
    untrack(() => {
      open = new Set([...open, focus.tag]);
      flash = focus.tag;
      clearTimeout(flashTimer);
      flashTimer = setTimeout(() => (flash = null), 1800);
      store.tagFocus = null; // handled: coming back to this page later should not repeat it
    });
    void tick().then(() => {
      const el = [...root.querySelectorAll<HTMLElement>('[data-tag]')].find((e) => e.dataset.tag === focus.tag);
      el?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    });
  });

  function toggle(name: string) {
    const next = new Set(open);
    if (!next.delete(name)) next.add(name);
    open = next;
  }

  const expandAll = () => (open = new Set(tags.map((t) => t.name)));
  const collapseAll = () => (open = new Set());

  /** Show this tag's tasks in the Tasks view, with the full report and filter bar. */
  function inTasks(t: TagEntry) {
    store.filter = `+${shellQuote(t.name)}`;
    store.view = 'tasks';
  }
</script>

<ExportFilter
  bind:filter={store.tagsFilter}
  onresult={(r) => (res = { rows: r.rows ?? res?.rows ?? [], error: r.error })}
  id="tags-filter"
  label="Tags filters"
  placeholder="Add filters: project:Home +work due.before:eow  (Tab completes)"
/>
<section aria-label="Tags" bind:this={root}>
  <div class="bar">
    <h2>Tags</h2>
    <span class="grow"></span>
    <button class="ghost" onclick={expandAll} disabled={tags.length === 0}>Expand all</button>
    <button class="ghost" onclick={collapseAll} disabled={open.size === 0}>Collapse all</button>
  </div>
  {#if error}
    <p class="err" role="alert">{error}</p>
  {:else if loading}
    <p class="dim">Loading…</p>
  {:else if tags.length === 0}
    <p class="dim">No task with a tag matches this filter.</p>
  {:else}
    <ul class="tree">
      {#each tags as t (t.name)}
        {@const isOpen = open.has(t.name)}
        <li data-tag={t.name} class:flash={flash === t.name}>
          <div class="head">
            <button
              class="ghost toggle"
              aria-expanded={isOpen}
              aria-label="{isOpen ? 'Collapse' : 'Expand'} +{t.name}"
              onclick={() => toggle(t.name)}
            >
              {#if isOpen}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
              <Tag size={15} />
              <span class="label">+{t.name}</span>
            </button>
            <span class="count" title="Tasks with this tag that match the filter">{t.tasks.length}</span>
            {#if t.overdue}<span class="late" title="Overdue">{t.overdue} overdue</span>{/if}
            <span class="grow"></span>
            <button class="ghost link" onclick={() => inTasks(t)} title="Open +{t.name} in the Tasks view"
              >Open in Tasks</button
            >
          </div>
          {#if isOpen}
            <ul class="tree">
              {#each t.tasks as r (r.uuid)}<TaskLine row={r} depth={1} />{/each}
              {#if t.tasks.length === 0}<li class="dim empty">No task matches the filter for this tag.</li>{/if}
            </ul>
          {/if}
        </li>
      {/each}
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
    padding: 3px 0;
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
    padding: 3px 0 3px 24px;
  }
  /* The entry a tag chip asked for. A fading background, and a plain outline when motion is reduced. */
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

<script lang="ts">
  import { formatFor } from './dateformat';
  import { formatMoment, fromParts, toParts } from './dates';
  import { clashOf } from './holidayClash';
  import { udaLabel } from './format';
  import { Calendar, Folder, Link, Repeat, TriangleAlert } from './icons';
  import { describeRecur } from './recurrence';
  import { store } from './store.svelte';
  import type { AddPreview } from './addPreview';

  /** The task a line would add, drawn as a card; with no preview (nothing typed yet) it says what will show here. */
  let { preview }: { preview: AddPreview | null } = $props();

  const fmt = $derived(formatFor('info', store.config?.config.settings));
  const defs = $derived(store.config?.config.udas ?? {});
  // A due or scheduled date on a holiday says so, beside the date.
  const dates = $derived(
    Object.entries(preview?.dates ?? {}).map(([name, ts]) => ({
      name,
      ts,
      warn: name === 'due' || name === 'scheduled' ? clashOf(name, fromParts(toParts(ts)))?.message : undefined,
    })),
  );
  const udas = $derived(
    Object.entries(preview?.udas ?? {}).map(([name, value]) => ({
      label: defs[name] ? udaLabel(defs[name]) : name,
      value: defs[name]?.type === 'date' ? formatMoment(Number(value), undefined, fmt) : value,
    })),
  );
</script>

<div class="card" class:empty={!preview} aria-label="Preview of the task">
  {#if !preview}
    <span class="dim">The task will show here as you type.</span>
  {:else}
    <div class="head">
      <span class="title" class:dim={!preview.description}>{preview.description || 'No description yet'}</span>
      {#if preview.priority}<span class="pri pri-{preview.priority.toLowerCase()}" title="Priority"
          >{preview.priority}</span
        >{/if}
    </div>
    {#if preview.project || preview.tags.length || dates.length || preview.recur || udas.length || preview.depends.length}
      <div class="facts">
        {#if preview.project}<span class="fact"><Folder size={13} /> {preview.project}</span>{/if}
        {#each dates as d (d.name)}
          <span class="fact" class:warn={!!d.warn} title={d.warn}
            >{#if d.warn}<TriangleAlert size={13} />{:else}<Calendar size={13} />{/if}
            <span class="dim">{d.name}</span>
            {formatMoment(d.ts, undefined, fmt)}{#if d.warn}<span class="sr-only"> {d.warn}</span>{/if}</span
          >
        {/each}
        {#if preview.recur}<span class="fact"><Repeat size={13} /> {describeRecur(preview.recur)}</span>{/if}
        {#each udas as u (u.label)}<span class="fact"><span class="dim">{u.label}</span> {u.value}</span>{/each}
        {#if preview.depends.length}<span class="fact"><Link size={13} /> depends on {preview.depends.join(', ')}</span
          >{/if}
        {#each preview.tags as tag (tag)}<span class="tagpill">{tag}</span>{/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .card {
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    min-height: 3.2em;
    align-content: center;
  }
  .empty {
    border-style: dashed;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 12px;
    font-size: 13px;
  }
  .fact {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .fact.warn {
    color: var(--warn);
    border-bottom: 1px dotted var(--warn);
  }
  .tagpill {
    margin: 0;
  }
  .pri {
    font-size: 12px;
    font-weight: 700;
    padding: 0 7px;
    border-radius: 4px;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  .pri-h {
    --c: var(--pri-h);
  }
  .pri-m {
    --c: var(--pri-m);
  }
  .pri-l {
    --c: var(--pri-l);
  }
</style>

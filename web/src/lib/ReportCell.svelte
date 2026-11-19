<script lang="ts">
  import DepLink from './DepLink.svelte';
  import { trimNumber, type Cell } from './format';
  import { Repeat } from './icons';
  import MarkdownNodes from './MarkdownNodes.svelte';
  import ProgressBar from './ProgressBar.svelte';
  import ProjectPath from './ProjectPath.svelte';
  import { describeRecur } from './recurrence';
  import { store, type Entry } from './store.svelte';
  import TagChip from './TagChip.svelte';
  import type { Column, Row } from './types';
  import UuidTip from './UuidTip.svelte';

  let {
    col,
    c,
    row,
    entry,
    activeTags,
    ontag,
  }: {
    col: Column;
    /** What the cell shows, as the engine wrote it. */
    c: Cell;
    row: Row;
    entry: Entry | null;
    activeTags: string[];
    ontag?: (tag: string) => void;
  } = $props();

  /** A numeric UDA's column, whose numbers are drawn without the zeros they end in. */
  const numeric = (name: string) => store.config?.config.udas?.[name.split('.')[0]]?.type === 'numeric';
  /** A cell of a UDA shown as a progress bar: its number and range, or null (some other column, or no number in it). */
  function progressOf(name: string, text: string) {
    const range = store.appearance.progress[name];
    const value = Number(text);
    return range && text.trim() !== '' && Number.isFinite(value) ? { value, range } : null;
  }
</script>

{#if c.uuid}
  <UuidTip text={c.text} uuid={c.uuid} />
{:else if c.links}
  {#each c.links as u (u)}<DepLink uuid={u} />{/each}
{:else if c.segments}
  <ProjectPath segments={c.segments} />
{:else if c.chips}
  <span class="cap chips"
    >{#each c.chips as t (t)}<TagChip tag={t} active={activeTags.includes(t)} onclick={ontag} />{/each}</span
  >
{:else if c.blocks}
  <div class="md">
    <MarkdownNodes nodes={c.blocks} ontoggle={(line) => store.tickBox(entry, row.uuid, col.name, c.text, line)} />
  </div>
{:else if progressOf(col.name, c.text)}
  {@const p = progressOf(col.name, c.text)}
  {#if p}<ProgressBar value={p.value} text={c.text} range={p.range} label={col.label} />{/if}
{:else if col.name === 'description'}
  {c.text}
{:else}
  <span class="cap">{numeric(col.name) ? trimNumber(c.text) : c.text}</span>
{/if}
{#if col.name === 'description' && row.recur}
  <span
    class="repeat"
    title={row.status === 'recurring'
      ? `Recurring task: repeats ${describeRecur(row.recur)}`
      : `Repeats ${describeRecur(row.recur)}`}
  >
    <Repeat size={12} /><span class="sr-only"
      >{row.status === 'recurring' ? 'recurring' : 'repeats'} {describeRecur(row.recur)}</span
    >
  </span>
{/if}
{#if col.name === 'description' && row.orphans.length}
  <span class="chip orphan" title="Has properties your taskrc doesn't define: {row.orphans.join(', ')} (read-only)">
    +{row.orphans.length}
  </span>
{/if}
{#each c.notes ?? [] as note (note.entry)}
  <div class="note dim">
    {note.at}
    {#if note.blocks}
      <div class="md">
        <MarkdownNodes nodes={note.blocks} />
      </div>
    {:else}
      {note.text}
    {/if}
  </div>
{/each}

<style>
  /*
   * A cell keeps to one line up to a width, then wraps (between tags, or between words), so one long value does not
   * widen its column without end. The description wraps already.
   */
  .cap {
    display: inline-block;
    max-width: 22em;
    white-space: normal;
    overflow-wrap: break-word;
  }
  .note {
    font-size: 13px;
    margin-left: 1em;
    white-space: pre-wrap;
  }
  /* Markdown is laid out by its own elements: the line breaks of the text are already in them. */
  .md {
    white-space: normal;
  }
  .orphan {
    margin-left: 6px;
    color: var(--dim);
  }
  .repeat {
    display: inline-flex;
    margin-left: 6px;
    color: var(--dim);
    vertical-align: -1px;
  }
  /* Phones: a card lays out its marks itself, so the cap is not a box there. */
  @media (max-width: 760px) {
    .cap {
      display: contents;
    }
    .note {
      margin-left: 0;
      flex-basis: 100%;
    }
    .md,
    .note {
      font-weight: 400;
    }
  }
</style>

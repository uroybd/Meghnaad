<script lang="ts">
  // Draws a tree from `tc_core::markdown`. Nothing is put into the page as HTML: every piece is an element made here, so
  // what a note says cannot add markup, and a link is only ever one the engine allowed (http, https, mailto).
  import MarkdownNodes from './MarkdownNodes.svelte';
  import type { MdNode } from './markdown';

  let {
    nodes,
    ontoggle,
  }: {
    nodes: MdNode[];
    /** Called with the line of a checklist item when its box is clicked. Without it (a note) the boxes are only shown. */
    ontoggle?: (line: number) => void;
  } = $props();
</script>

{#each nodes as n, i (i)}
  {#if n.t === 'text'}{n.s}
  {:else if n.t === 'br'}<br />
  {:else if n.t === 'b'}<strong><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></strong>
  {:else if n.t === 'i'}<em><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></em>
  {:else if n.t === 'del'}<del><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></del>
  {:else if n.t === 'code'}<code>{n.s}</code>
  {:else if n.t === 'a'}<a href={n.href} target="_blank" rel="noopener noreferrer nofollow"
      ><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></a
    >
  {:else if n.t === 'p'}<p><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></p>
  {:else if n.t === 'h'}
    <!-- Below the page's own headings, whatever level was typed. -->
    <div class="md-h" role="heading" aria-level={Math.min(6, (n.n ?? 1) + 2)}>
      <MarkdownNodes nodes={n.c ?? []} {ontoggle} />
    </div>
  {:else if n.t === 'ul'}<ul><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></ul>
  {:else if n.t === 'ol'}<ol start={n.n}><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></ol>
  {:else if n.t === 'li'}
    <li class:task={n.done !== undefined}>
      {#if n.done !== undefined}
        <input
          type="checkbox"
          checked={n.done}
          disabled={!ontoggle}
          aria-label={n.done ? 'Done: untick' : 'Not done: tick'}
          onclick={(e) => {
            // The text decides what is ticked, not the box: it is drawn again from the text once that has changed.
            e.preventDefault();
            e.stopPropagation();
            if (n.line !== undefined) ontoggle?.(n.line);
          }}
        />
      {/if}
      <div class="li-body" class:done={n.done}><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></div>
    </li>
  {:else if n.t === 'q'}<blockquote><MarkdownNodes nodes={n.c ?? []} {ontoggle} /></blockquote>
  {:else if n.t === 'pre'}<pre><code>{n.s}</code></pre>
  {:else if n.t === 'hr'}<hr />
  {/if}
{/each}

<style>
  p {
    margin: 0 0 0.4em;
  }
  p:last-child {
    margin-bottom: 0;
  }
  .md-h {
    margin: 0.5em 0 0.25em;
    font-size: 1em;
    font-weight: 650;
  }
  ul,
  ol {
    margin: 0.2em 0;
    padding-left: 1.4em;
  }
  li.task {
    list-style: none;
    display: flex;
    gap: 0.45em;
    align-items: flex-start;
    margin-left: -1.4em;
  }
  /* Smaller than the boxes that select a task (and the page's touch size for them), so a checklist reads as part of the text. */
  li.task input {
    width: 12px;
    height: 12px;
    min-height: 0;
    margin: 0.3em 0 0;
    flex: none;
    cursor: pointer;
  }
  @media (max-width: 760px) {
    li.task input {
      width: 16px;
      height: 16px;
      margin-top: 0.2em;
    }
  }
  li.task input:disabled {
    cursor: default;
  }
  .li-body {
    min-width: 0;
  }
  .li-body.done {
    text-decoration: line-through;
    opacity: 0.65;
  }
  .li-body :global(p) {
    margin: 0;
  }
  blockquote {
    margin: 0.3em 0;
    padding-left: 0.7em;
    border-left: 3px solid var(--line, currentColor);
    opacity: 0.85;
  }
  code {
    font-family: var(--mono);
    font-size: 0.92em;
    padding: 0.05em 0.3em;
    border-radius: 4px;
    background: color-mix(in srgb, currentColor 10%, transparent);
  }
  pre {
    margin: 0.3em 0;
    padding: 0.5em 0.7em;
    overflow-x: auto;
    border-radius: 6px;
    background: color-mix(in srgb, currentColor 8%, transparent);
  }
  pre code {
    padding: 0;
    background: none;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--line, currentColor);
    opacity: 0.4;
    margin: 0.5em 0;
  }
  a {
    color: var(--accent, inherit);
    text-decoration: underline;
    overflow-wrap: anywhere;
  }
</style>

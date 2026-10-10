<script lang="ts">
  import { tick } from 'svelte';
  import { shortUuid } from './format';
  import { fetchTask } from './lookup';
  import TaskCard from './TaskCard.svelte';
  import { taskPath } from './route';
  import { store } from './store.svelte';
  import type { Row } from './types';

  /**
   * Another task named by its uuid (a dependency, a recurring task's parent): its short id, which on hover, focus or
   * a tap shows a small card of the task with a button that opens it in the drawer. On a device that can hover, a
   * click opens the drawer directly (the card has already shown); on a touch screen the tap shows the card first.
   */
  let { uuid }: { uuid: string } = $props();

  let shown = $state(false);
  let card = $state<Row | null>(null);
  let missing = $state(false);
  let tip: HTMLElement | undefined = $state();

  // Until the card arrives, what the page already has of the task.
  const known = $derived(store.tasks.find((t) => t.uuid === uuid));

  async function show(e: Event) {
    shown = true;
    const anchor = e.currentTarget as HTMLElement;
    void place(anchor);
    const row = await fetchTask(uuid);
    card = row;
    missing = !row;
    void place(anchor); // the card has its content now, so its height is known
  }

  async function place(anchor: HTMLElement) {
    // Fixed under the id so a scrolling table can't clip it; moved back inside the window (above the id if it
    // would run off the bottom) once it is drawn.
    await tick();
    if (!tip) return;
    const r = anchor.getBoundingClientRect();
    const width = tip.offsetWidth || 280;
    tip.style.left = `${Math.max(8, Math.min(r.left, window.innerWidth - width - 8))}px`;
    const below = r.bottom + 4;
    tip.style.top = `${below + tip.offsetHeight > window.innerHeight ? Math.max(8, r.top - tip.offsetHeight - 4) : below}px`;
  }

  const hide = () => (shown = false);

  function onclick(e: MouseEvent) {
    e.stopPropagation(); // not also a click on the row it sits in
    if (window.matchMedia?.('(hover: hover)').matches) open();
    else if (shown) hide();
    else void show(e);
  }

  function open() {
    hide();
    store.openDetail(uuid, null);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -- the group only passes the pointer and Escape on to its button and card -->
<span
  class="deplink"
  role="group"
  aria-label="Task {shortUuid(uuid)}"
  onpointerenter={(e) => e.pointerType !== 'touch' && void show(e)}
  onpointerleave={(e) => e.pointerType !== 'touch' && hide()}
  onfocusout={(e) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) hide();
  }}
  onkeydown={(e) => e.key === 'Escape' && shown && (hide(), e.stopPropagation())}
>
  <button
    type="button"
    class="ghost id mono"
    aria-expanded={shown}
    {onclick}
    onfocus={(e) => e.currentTarget.matches(':focus-visible') && void show(e)}>{shortUuid(uuid)}</button
  >
  {#if shown}
    <span class="tip" bind:this={tip} role="tooltip" data-testid="deplink-card">
      {#if card}
        <TaskCard row={card} />
      {:else if known && !missing}
        <strong class="desc">{known.description}</strong>
      {:else if missing}
        <span class="dim">This task is not here any more.</span>
      {:else}
        <span class="dim">Loading…</span>
      {/if}
      <span class="foot">
        <button type="button" class="open" onclick={open}>Open task</button>
        <a class="dim mono" href={taskPath(uuid)} onclick={(e) => (e.preventDefault(), open())}>{taskPath(uuid)}</a>
      </span>
    </span>
  {/if}
</span>

<style>
  .deplink {
    display: inline-block;
    margin-right: 6px;
  }
  .id {
    padding: 0 4px;
    font-size: inherit;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
  }
  .tip {
    position: fixed;
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: min(300px, calc(100vw - 16px));
    padding: 10px 12px;
    font-weight: 400;
    white-space: normal;
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.18);
  }
  .desc {
    overflow-wrap: anywhere;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 4px;
    font-size: 12px;
  }
  .foot a {
    color: inherit;
  }
</style>

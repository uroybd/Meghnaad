<script lang="ts">
  import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Info, Move, Pencil, Trash2 } from './icons';
  import MoreMenu, { type MenuAction } from './MoreMenu.svelte';
  import BoardSection from './BoardSection.svelte';
  import SectionInfo from './SectionInfo.svelte';
  import {
    GAP_PX,
    ROW_PX,
    MIN_WIDTH,
    columnsFor,
    flowed,
    loadBoard,
    moveBy,
    place,
    reorder,
    resizeBy,
    resizeFlow,
    stacked,
    zoneNow,
    type Board,
    type Rect,
    type Section,
    type SectionData,
  } from './boards';
  import { runCli } from './api';
  import { withContext } from './pageContext';
  import { store } from './store.svelte';

  /**
   * A board: its sections on a 12-column grid, or on 6 columns (4 on a small phone) in reading order on a narrow screen, each
   * as wide as its share of the row, so several numbers sit side by side. Editing the layout drags a section by its handle,
   * resizes it by the corner (on a phone too, in the screen's columns), or with the arrow keys on either; the places are the
   * engine's (`tc_core::board::layout`), which pushes what a section lands on down and floats everything up. This draws the
   * board it is given and reports the new sections; it keeps nothing.
   */
  let {
    board,
    editing = false,
    selected = null,
    onchange,
    onselect,
    onedit,
    onremove,
  }: {
    board: Board;
    editing?: boolean;
    selected?: string | null;
    onchange?: (sections: Section[]) => void;
    onselect?: (id: string) => void;
    onedit?: (id: string) => void;
    onremove?: (id: string) => void;
  } = $props();

  // ---- The data ----------------------------------------------------------------------------------------------------
  let data = $state<Record<string, SectionData>>({});
  // Ask again when what a section asks for changes (not when it only moves), and when a task changes anywhere.
  const scope = $derived(`board:${board.id}` as const);
  const dataKey = $derived(
    JSON.stringify([
      store.contextChoice(scope),
      board.filter,
      board.sections.map((s) => [s.id, s.type, s.report, s.filter, s.ignoreBoardFilter, s.measure, s.by, s.series]),
    ]),
  );
  let asked = 0;
  $effect(() => {
    void dataKey;
    void store.rev;
    const mine = ++asked;
    const zone = zoneNow(store.config?.config.settings);
    const t = setTimeout(() => {
      const choice = store.contextChoice(scope);
      const run = (input: { args: string[] }) => runCli(withContext(input, choice));
      void loadBoard($state.snapshot(board) as Board, zone, run).then((d) => {
        if (mine === asked) data = d;
      });
    }, 150);
    return () => clearTimeout(t);
  });

  // ---- The info panel -----------------------------------------------------------------------------------------------
  let info = $state<string | null>(null);
  const infoSection = $derived(board.sections.find((s) => s.id === info) ?? null);
  function closeInfo() {
    const id = info;
    info = null;
    // Back to the button that opened it.
    if (id) queueMicrotask(() => document.getElementById(`board-info-${id}`)?.focus());
  }

  // ---- Wide or narrow ----------------------------------------------------------------------------------------------
  let narrow = $state(false);
  let small = $state(false);
  $effect(() => {
    if (typeof matchMedia !== 'function') return;
    const wide = matchMedia('(max-width: 720px)');
    const tiny = matchMedia('(max-width: 480px)');
    narrow = wide.matches;
    small = tiny.matches;
    const onWide = (e: MediaQueryListEvent) => (narrow = e.matches);
    const onTiny = (e: MediaQueryListEvent) => (small = e.matches);
    wide.addEventListener('change', onWide);
    tiny.addEventListener('change', onTiny);
    return () => {
      wide.removeEventListener('change', onWide);
      tiny.removeEventListener('change', onTiny);
    };
  });
  const cols = $derived(columnsFor(narrow, small));
  const gap = $derived(narrow ? 8 : GAP_PX);

  // Reading order is also the order a screen reader and the Tab key meet them in. A narrow screen has the sections' places on
  // its own, fewer columns.
  const ordered = $derived(narrow ? flowed(board.sections, cols) : stacked(board.sections));

  // ---- A card too narrow for its buttons ----------------------------------------------------------------------------
  // Its tools fold into one ⋯ menu. A width of 0 (not measured yet, or no layout at all) is not narrow.
  const COMPACT_PX = 280;
  let widths = $state<Record<string, number>>({});
  function measure(el: HTMLElement, id: string) {
    if (typeof ResizeObserver === 'undefined') return;
    const read = () => (widths[id] = el.clientWidth);
    const watch = new ResizeObserver(read);
    watch.observe(el);
    read();
    return { destroy: () => watch.disconnect() };
  }
  const compact = (s: Section) => (widths[s.id] ?? 0) > 0 && widths[s.id] < COMPACT_PX;

  function cellActions(s: Section, i: number): MenuAction[] {
    const go = (dx: number, dy: number) => () => {
      const next = moveBy(board.sections, s.id, dx, dy);
      onchange?.(next);
      const now = next.find((x) => x.id === s.id);
      if (now) announce = describe(now);
    };
    const moves: MenuAction[] = narrow
      ? [
          { label: 'Move up', icon: ArrowUp, run: () => shift(s, -1), disabled: i === 0 },
          { label: 'Move down', icon: ArrowDown, run: () => shift(s, 1), disabled: i === ordered.length - 1 },
        ]
      : [
          { label: 'Move up', icon: ArrowUp, run: go(0, -1) },
          { label: 'Move down', icon: ArrowDown, run: go(0, 1) },
          { label: 'Move left', icon: ArrowLeft, run: go(-1, 0) },
          { label: 'Move right', icon: ArrowRight, run: go(1, 0) },
        ];
    return [
      { label: 'Information', icon: Info, run: () => (info = s.id), disabled: !data[s.id] },
      ...moves,
      {
        label: 'Settings',
        icon: Pencil,
        run: () => {
          onselect?.(s.id);
          onedit?.(s.id);
        },
      },
      { label: 'Remove', icon: Trash2, danger: true, run: () => onremove?.(s.id) },
    ];
  }

  // ---- Moving and resizing -----------------------------------------------------------------------------------------
  let grid: HTMLDivElement | undefined = $state();
  let announce = $state('');
  let drag: {
    id: string;
    mode: 'move' | 'resize';
    x: number;
    y: number;
    start: Rect;
    base: Section[];
    last: string;
  } | null = null;

  const describe = (s: Section) => {
    const w = narrow ? (flowed([s], cols)[0]?.w ?? s.w) : s.w;
    return `${s.title}: column ${s.x + 1}, row ${s.y + 1}, ${w} ${w === 1 ? 'column' : 'columns'} wide, ${s.h} ${s.h === 1 ? 'row' : 'rows'} high`;
  };

  function begin(e: PointerEvent, s: Section, mode: 'move' | 'resize') {
    if (!editing) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = {
      id: s.id,
      mode,
      x: e.clientX,
      y: e.clientY,
      start: { x: s.x, y: s.y, w: s.w, h: s.h },
      base: board.sections,
      last: '',
    };
    onselect?.(s.id);
  }

  function track(e: PointerEvent) {
    if (!drag || !grid) return;
    // One grid step is a column (or row) and its gap. Each step is worked out from where the drag began, on the layout
    // as it was then, so dragging back puts what was pushed aside back where it was.
    const dx = Math.round((e.clientX - drag.x) / ((grid.clientWidth + gap) / cols));
    const dy = Math.round((e.clientY - drag.y) / (ROW_PX + gap));
    const s = drag.start;
    if (narrow) {
      // A phone resizes in its own columns; the engine keeps the width in the board's 12.
      const key = `${dx},${dy}`;
      if (drag.mode !== 'resize' || key === drag.last) return;
      drag.last = key;
      onchange?.(resizeFlow(drag.base, drag.id, cols, dx, dy));
      return;
    }
    const r: Rect =
      drag.mode === 'move'
        ? { ...s, x: Math.max(0, s.x + dx), y: Math.max(0, s.y + dy) }
        : { ...s, w: Math.max(MIN_WIDTH, s.w + dx), h: Math.max(1, s.h + dy) };
    const key = `${r.x},${r.y},${r.w},${r.h}`;
    if (key === drag.last) return;
    drag.last = key;
    onchange?.(place(drag.base, drag.id, r));
  }

  function end() {
    if (!drag) return;
    const s = board.sections.find((x) => x.id === drag?.id);
    drag = null;
    if (s) announce = describe(s);
  }

  const ARROWS: Record<string, [number, number]> = {
    ArrowLeft: [-1, 0],
    ArrowRight: [1, 0],
    ArrowUp: [0, -1],
    ArrowDown: [0, 1],
  };

  function key(e: KeyboardEvent, s: Section, mode: 'move' | 'resize') {
    const d = ARROWS[e.key];
    if (!d) return;
    e.preventDefault();
    const next =
      mode === 'move'
        ? moveBy(board.sections, s.id, d[0], d[1])
        : narrow
          ? resizeFlow(board.sections, s.id, cols, d[0], d[1])
          : resizeBy(board.sections, s.id, d[0], d[1]);
    onchange?.(next);
    const now = next.find((x) => x.id === s.id);
    if (now) announce = describe(now);
  }

  function shift(s: Section, delta: -1 | 1) {
    const next = reorder(board.sections, s.id, delta);
    onchange?.(next);
    announce = `${s.title}: now number ${stacked(next).findIndex((x) => x.id === s.id) + 1} of ${next.length}`;
  }
</script>

{#if !board.sections.length}
  <p class="dim empty">
    {editing ? 'Nothing here yet. Add a section.' : 'This board has no sections yet. Edit it to add some.'}
  </p>
{/if}
<div class="grid" class:editing bind:this={grid} style:--row="{ROW_PX}px" style:--gap="{gap}px" style:--cols={cols}>
  {#each ordered as s, i (s.id)}
    <div
      class="cell"
      class:selected={editing && selected === s.id}
      style:--x={s.x}
      style:--y={s.y}
      style:--w={s.w}
      style:--h={s.h}
      use:measure={s.id}
    >
      <BoardSection section={s} data={data[s.id]} {editing} compact={compact(s)} oninfo={() => (info = s.id)} />
      {#if editing}
        <div class="tools">
          {#if compact(s)}
            <MoreMenu label="Actions of {s.title}" actions={cellActions(s, i)} />
          {:else if narrow}
            <button
              class="ghost"
              aria-label="Move {s.title} up"
              title="Move up"
              disabled={i === 0}
              onclick={() => shift(s, -1)}><ArrowUp size={15} /></button
            >
            <button
              class="ghost"
              aria-label="Move {s.title} down"
              title="Move down"
              disabled={i === ordered.length - 1}
              onclick={() => shift(s, 1)}><ArrowDown size={15} /></button
            >
          {:else}
            <button
              class="ghost handle"
              aria-label="Move {s.title}. Drag, or use the arrow keys."
              title="Drag to move, or use the arrow keys"
              onpointerdown={(e) => begin(e, s, 'move')}
              onpointermove={track}
              onpointerup={end}
              onpointercancel={end}
              onkeydown={(e) => key(e, s, 'move')}><Move size={15} /></button
            >
          {/if}
          {#if !compact(s)}
            <button
              class="ghost"
              aria-label="Settings of {s.title}"
              title="Settings"
              onclick={() => {
                onselect?.(s.id);
                onedit?.(s.id);
              }}><Pencil size={15} /></button
            >
            <button class="ghost danger" aria-label="Remove {s.title}" title="Remove" onclick={() => onremove?.(s.id)}
              ><Trash2 size={15} /></button
            >
          {/if}
        </div>
        <button
          class="resize"
          aria-label="Resize {s.title}. Drag, or use the arrow keys."
          title="Drag to resize, or use the arrow keys"
          onpointerdown={(e) => begin(e, s, 'resize')}
          onpointermove={track}
          onpointerup={end}
          onpointercancel={end}
          onkeydown={(e) => key(e, s, 'resize')}
        ></button>
      {/if}
    </div>
  {/each}
</div>
<p class="sr-only" aria-live="polite">{announce}</p>
{#if infoSection && data[infoSection.id]}
  <SectionInfo
    section={infoSection}
    data={data[infoSection.id]}
    onclose={closeInfo}
    context={store.contextChoice(scope)}
  />
{/if}

<style>
  .empty {
    margin: 8px 0 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    grid-auto-rows: var(--row);
    gap: var(--gap);
  }
  .cell {
    position: relative;
    min-width: 0;
    grid-column: calc(var(--x) + 1) / span var(--w);
    grid-row: calc(var(--y) + 1) / span var(--h);
  }
  .editing .cell {
    outline: 1px dashed var(--line);
    outline-offset: 2px;
    border-radius: var(--radius);
  }
  .editing .cell.selected {
    outline: 2px solid var(--accent);
  }
  .tools {
    position: absolute;
    top: 4px;
    right: 4px;
    display: flex;
    gap: 2px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 1px;
  }
  .tools button {
    padding: 4px 6px;
  }
  .handle {
    cursor: grab;
    touch-action: none;
  }
  .resize {
    position: absolute;
    right: 2px;
    bottom: 2px;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 0 0 6px 0;
    background: linear-gradient(135deg, transparent 50%, var(--accent) 50%);
    cursor: nwse-resize;
    touch-action: none;
  }
</style>
